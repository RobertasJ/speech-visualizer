mod live;
mod options;
mod scene;
mod stt;
// Not wired into the app yet.
#[allow(dead_code)]
mod typesafeai;
mod ui;

use freya::prelude::*;

use options::Options;
use ui::{Console, LiveSession, Selection, Session};

fn main() {
    dotenv::dotenv().ok();

    let mut options = Options::default();
    if let Some(dir) = std::env::args().nth(1) {
        options.models_dir = dir.into();
    }

    // Kept for the whole run, so going back to the selection shows the last choices.
    let options = State::create_global(options);

    launch(
        // Global contexts are in the root context of every window.
        LaunchConfig::new().with_global(options).with_window(
            WindowConfig::new_app(SpeechApp)
                .with_size(900., 600.)
                .with_title("Speech visualizer"),
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
struct SpeechApp;

impl App for SpeechApp {
    fn render(&self) -> impl IntoElement {
        // Follow the OS light/dark preference in this window.
        let mut theme = use_init_theme(|| Platform::get().preferred_theme.read().to_theme());
        use_side_effect(move || theme.set(Platform::get().preferred_theme.read().to_theme()));

        let screen = use_provide_context(|| State::create(Screen::Selection));

        let content = match screen() {
            Screen::Selection => Selection.into_element(),
            Screen::Transcript => Session.into_element(),
            Screen::Console => Console.into_element(),
            Screen::Live => LiveSession.into_element(),
        };

        rect()
            .expanded()
            .theme_background()
            .theme_color()
            .child(content)
    }
}
