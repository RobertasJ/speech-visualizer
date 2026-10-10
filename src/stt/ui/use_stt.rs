use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use freya::prelude::*;
use futures_channel::mpsc;
use futures_lite::StreamExt;

use crate::options::Options;
use crate::stt::{self, Config, Event};

/// Transcribes the microphone for as long as the component is mounted.
///
/// `on_event` runs on the UI thread, so it can write to `State`. The first event is
/// [`Event::Started`] once the models have loaded, or [`Event::Failed`] if they didn't;
/// nothing comes after `Failed`.
///
/// The model, language and pause length are read from [`Options`] on mount; changing
/// them later needs a remount. Only the first render's `on_event` is kept.
///
/// Unmounting blocks until the thread has stopped, which is instant while transcribing
/// but lasts until loading is done if the models are still loading.
pub fn use_stt(mut on_event: impl FnMut(Event) + 'static) {
    let (stop, thread) = use_hook(|| {
        let options = GlobalContexts::get().get_context::<State<Options>>();
        let config = config(&options.peek());

        // The thread is stopped by setting `stop` to true, which the thread checks in a loop.
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = mpsc::unbounded();

        let thread = std::thread::spawn({
            let stop = stop.clone();
            move || {
                let mut send = |event| {
                    let _ = tx.unbounded_send(event);
                };
                if let Err(err) = stt::run(&config, &stop, &mut send) {
                    send(Event::Failed(err));
                }
            }
        });

        // Cancelled when the component unmounts.
        spawn(async move {
            while let Some(event) = rx.next().await {
                on_event(event);
            }
        });

        (stop, Rc::new(Cell::new(Some(thread))))
    });

    use_drop(move || {
        stop.store(true, Ordering::Relaxed);
        if let Some(thread) = thread.take() {
            let _ = thread.join();
        }
    });
}

fn config(options: &Options) -> Config {
    Config {
        model_path: options.model.clone().unwrap_or_default(),
        vad_path: options.vad.clone().unwrap_or_default(),
        language: (options.language != "auto").then(|| options.language.to_owned()),
        pause_ms: options.pause_ms,
    }
}
