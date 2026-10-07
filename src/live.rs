//! Live mode: turns what's being said into commands for the display window's scene.

use std::sync::mpsc;
use std::thread;

use crate::scene::{Command, SceneSender};
use crate::stt::Event;

/// Starts the live logic on its own thread, so it can take its time (and wait for
/// replies to its commands) without holding up transcription or the UI. Returns the
/// callback to hand the transcriber; the thread ends once that's dropped.
pub fn spawn(scene: SceneSender) -> impl FnMut(Event) + Send + 'static {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut live = Live::new(scene);
        for event in rx {
            live.on_event(event);
        }
    });
    move |event| {
        let _ = tx.send(event);
    }
}

/// What the live logic keeps between events.
struct Live {
    scene: SceneSender,
}

impl Live {
    fn new(scene: SceneSender) -> Self {
        // Start from an empty display.
        scene.send(Command::Clear);
        Self { scene }
    }

    /// Called for every transcription event, in order: work out what the display should
    /// show and send commands for it.
    fn on_event(&mut self, event: Event) {
        match event {
            // The open section's text so far; it may still change.
            Event::Live { .. } => {}
            // Text that won't change anymore: run it as a command, like a typed one.
            Event::Final { text, .. } => {
                let line = normalize(&text);
                if line.is_empty() {
                    return;
                }
                let command = match Command::parse(&line) {
                    Ok(command) => command,
                    Err(err) => {
                        eprintln!("[live] '{line}': {err}");
                        return;
                    }
                };
                if let Err(err) = self.scene.request(command).wait() {
                    eprintln!("[live] '{line}': {err}");
                }
            }
            Event::Error(_) => {}
        }
    }
}

/// Drops commas and periods and lowercases, so "Text, Hello." reads as "text hello".
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, ',' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_punctuation_and_case() {
        assert_eq!(
            normalize(" Text in 3, Hello World."),
            " text in 3 hello world"
        );
    }
}
