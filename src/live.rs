use freya::prelude::State;

use crate::scene::{Names, Scene};
use crate::stt::Event;

/// Turns transcription events into scene changes, on the UI thread.
pub struct Live {
    // Will be used once commands are run from the transcript.
    #[allow(dead_code)]
    scene: State<Scene>,
    #[allow(dead_code)]
    names: State<Names>,
    transcript: String,
}

impl Live {
    pub fn new(scene: State<Scene>, names: State<Names>) -> Self {
        Self {
            scene,
            names,
            transcript: String::new(),
        }
    }

    /// Called for every transcription event, in order: work out what the display should
    /// show and send commands for it.
    pub fn on_event(&mut self, event: Event) {
        match event {
            // The open section's text so far; it may still change.
            Event::Live { .. } => {}
            // Text that won't change anymore: run it as a command, like a typed one.
            Event::Final { text, .. } => {
                self.transcript.push_str(&text);
            }
            Event::Started(_) | Event::Error(_) | Event::Failed(_) => {}
        }
    }
}
