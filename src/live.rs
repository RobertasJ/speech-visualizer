use freya::prelude::*;

use crate::scene::{Names, Scene};
use crate::stt::{Event, Status, use_stt};

/// Transcribes the microphone for as long as the component is mounted, and turns what
/// is said into changes to `scene`.
pub fn use_live(scene: State<Scene>, names: State<Names>) -> UseLive {
    let live = UseLive {
        scene,
        names,
        status: use_state(|| Status::Loading),
        transcript: use_state(String::new),
        command_log: use_state(Vec::new),
    };
    let mut on_event = live;
    use_stt(move |event| on_event.on_event(event));
    live
}

/// Returned by [`use_live`]. Turns transcription events into scene changes, on the UI
/// thread.
#[derive(Clone, Copy, PartialEq)]
pub struct UseLive {
    scene: State<Scene>,
    names: State<Names>,
    status: State<Status>,
    transcript: State<String>,
    command_log: State<Vec<String>>,
}

impl UseLive {
    /// Whether the models are loading, listening, or failed.
    pub fn status(&self) -> State<Status> {
        self.status
    }

    /// Called for every transcription event, in order: work out what the display should
    /// show and send commands for it.
    fn on_event(&mut self, event: Event) {
        match event {
            Event::Started(device) => self.status.set(Status::Listening(device)),
            Event::Failed(err) => self.status.set(Status::Failed(err)),
            // The open section's text so far; it may still change.
            Event::Live { .. } => {}
            // Text that won't change anymore: run it as a command, like a typed one.
            Event::Final { text, .. } => {
                self.transcript.write().push_str(&text);
            }
            Event::Error(_) => {}
        }
    }
}
