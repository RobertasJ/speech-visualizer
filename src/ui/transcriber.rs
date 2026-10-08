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

/// Starts a transcriber with `options`. start() blocks while the models load, so it
/// runs on its own thread; if the receiver is gone by the time it's done, the
/// transcriber is dropped right there.
pub fn start_transcriber(
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

pub async fn wait_for_transcriber(
    setup: oneshot::Receiver<Result<Transcriber, stt::Error>>,
    mut status: State<Status>,
    mut running: State<Option<Transcriber>>,
) -> bool {
    match setup.await {
        Ok(Ok(transcriber)) => {
            status.set(Status::Listening(transcriber.device().clone()));
            running.set(Some(transcriber));
            true
        }
        Ok(Err(err)) => {
            status.set(Status::Failed(err.to_string()));
            false
        }
        Err(_) => {
            status.set(Status::Failed("the setup thread panicked".into()));
            false
        }
    }
}

/// Stops the transcriber in `running` when the calling component unmounts, on a
/// separate thread: dropping it waits for the pass in progress, which would otherwise
/// freeze the window.
pub fn use_stop_in_background(mut running: State<Option<Transcriber>>) {
    use_drop(move || {
        if let Some(transcriber) = running.take() {
            std::thread::spawn(move || drop(transcriber));
        }
    });
}
