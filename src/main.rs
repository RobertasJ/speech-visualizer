mod command;
mod live;
mod names;
mod options;
mod scene;
mod stt;
mod ui;
mod window;

use freya::prelude::*;

use options::Options;
use stt::Transcriber;
use ui::{Console, LiveSession, Selection, Session};

fn main() {
    let mut options = Options::default();
    if let Some(dir) = std::env::args().nth(1) {
        options.models_dir = dir.into();
    }

    // Kept for the whole run, so going back to the selection shows the last choices.
    let options = State::create_global(options);
    let transcriber = State::create_global(None);

    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new_app(SpeechApp {
                options,
                transcriber,
            })
            .with_size(900., 600.)
            .with_title("Speech visualizer")
            // Closing the main window quits, even while the display window is open.
            .with_on_close(move |mut ctx, _| {
                // Stop the transcriber and wait for its thread here: if the process
                // exits while whisper is still using the GPU, ggml aborts.
                let mut transcriber = transcriber;
                drop(transcriber.take());
                ctx.exit();
                CloseDecision::Close
            }),
        ),
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Selection,
    Transcript,
    Console,
    Live,
}

#[derive(Clone, Copy)]
struct SpeechApp {
    options: State<Options>,
    transcriber: State<Option<Transcriber>>,
}

impl App for SpeechApp {
    fn render(&self) -> impl IntoElement {
        // Follow the OS light/dark preference in this window.
        let mut theme = use_init_theme(|| Platform::get().preferred_theme.read().to_theme());
        use_side_effect(move || theme.set(Platform::get().preferred_theme.read().to_theme()));

        let screen = use_state(|| Screen::Selection);

        let content = match screen() {
            Screen::Selection => Selection {
                options: self.options,
                screen,
            }
            .into_element(),
            Screen::Transcript => Session {
                options: self.options.read().clone(),
                screen,
                transcriber: self.transcriber,
            }
            .into_element(),
            Screen::Console => Console { screen }.into_element(),
            Screen::Live => LiveSession {
                options: self.options.read().clone(),
                screen,
                transcriber: self.transcriber,
            }
            .into_element(),
        };

        rect()
            .expanded()
            .theme_background()
            .theme_color()
            .child(content)
    }
}
