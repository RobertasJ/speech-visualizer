use freya::prelude::*;
use futures_channel::oneshot;

use crate::options::Options;
use crate::stt::{self, Config, DeviceInfo, Event, Transcriber};

pub enum Status {
    Loading,
    Listening(DeviceInfo),
    Stopped,
    Failed(String),
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Loading => write!(f, "Loading models…"),
            Status::Listening(device) => write!(
                f,
                "Recording from '{}' at {} Hz, {} channel(s)",
                device.name, device.sample_rate, device.channels
            ),
            Status::Stopped => write!(f, "Stopped"),
            Status::Failed(err) => write!(f, "Failed to start: {err}"),
        }
    }
}

/// Starts a transcriber owned by the calling component, with the options at the time,
/// and hands its events to the callback `on_event` makes. It's stopped when the
/// component unmounts, waiting for the pass in progress: if the process exits while
/// whisper is still using the GPU, ggml aborts.
pub fn use_transcriber<F>(on_event: impl FnOnce() -> F) -> State<Status>
where
    F: FnMut(Event) + Send + 'static,
{
    let mut status = use_state(|| Status::Loading);
    let mut running = use_state(|| None::<Transcriber>);

    use_drop(move || drop(running.take()));
    use_hook(|| {
        let options = GlobalContexts::get().get_context::<State<Options>>();
        // Dropped along with the callback, when the transcription thread ends.
        let (ended_tx, ended_rx) = oneshot::channel::<()>();
        let mut on_event = on_event();
        let setup = start_transcriber(&options.peek(), move |event| {
            let _ = &ended_tx;
            on_event(event);
        });

        // Cancelled when the component unmounts.
        spawn(async move {
            match setup.await {
                Ok(Ok(transcriber)) => {
                    status.set(Status::Listening(transcriber.device().clone()));
                    running.set(Some(transcriber));
                }
                Ok(Err(err)) => return status.set(Status::Failed(err.to_string())),
                Err(_) => return status.set(Status::Failed("the setup thread panicked".into())),
            }
            let _ = ended_rx.await;
            status.set(Status::Stopped);
        });
    });

    status
}

/// Starts a transcriber with `options`. start() blocks while the models load, so it
/// runs on its own thread; if the receiver is gone by the time it's done, the
/// transcriber is dropped right there.
fn start_transcriber(
    options: &Options,
    on_event: impl FnMut(Event) + Send + 'static,
) -> oneshot::Receiver<Result<Transcriber, stt::Error>> {
    let config = Config {
        model_path: options.model.clone().unwrap_or_default(),
        vad_path: options.vad.clone().unwrap_or_default(),
        language: (options.language != "auto").then(|| options.language.to_owned()),
        pause_ms: options.pause_ms,
    };
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Transcriber::start(config, on_event));
    });
    rx
}
