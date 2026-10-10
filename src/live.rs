use freya::prelude::State;
use futures_channel::mpsc;
use futures_lite::StreamExt;

use crate::scene::{Command, Names, Scene};
use crate::stt::Event;

/// Starts the live logic as a task of the calling component, on the UI thread where
/// the scene can be changed; it stops when the component unmounts. Returns the
/// callback to hand the transcriber, which may call it from any thread.
pub fn spawn(scene: State<Scene>, names: State<Names>) -> impl FnMut(Event) + Send + 'static {
    let (tx, mut rx) = mpsc::unbounded();
    freya::prelude::spawn(async move {
        let mut live = Live { scene, names };
        while let Some(event) = rx.next().await {
            live.on_event(event);
        }
    });
    move |event| {
        let _ = tx.unbounded_send(event);
    }
}

struct Live {
    scene: State<Scene>,
    names: State<Names>,
}

impl Live {
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
                let command = match line.parse::<Command>() {
                    Ok(command) => command,
                    Err(err) => {
                        eprintln!("[live] '{line}': {err}");
                        return;
                    }
                };
                if let Err(err) = command.apply(&mut self.scene.write(), &mut self.names.write()) {
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
