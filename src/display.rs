//! The display window: renders the whole [`Scene`] after every change. F11 toggles
//! fullscreen.

use freya::prelude::*;
use freya::winit::window::{Fullscreen, WindowId};

use crate::scene::{self, ElementId, Scene};

/// Opens the display window, or focuses it if it's open already. `window` tracks it
/// and is reset when it closes.
pub fn show(scene: State<Scene>, mut window: State<Option<WindowId>>) {
    if let Some(id) = *window.peek() {
        Platform::get().focus_window(id);
        return;
    }
    // Not tied to the caller, so the id is recorded even if the caller unmounts first.
    spawn_forever(async move {
        let config = WindowConfig::new_app(DisplayWindow { scene })
            .with_size(800., 600.)
            .with_title("Speech visualizer display")
            .with_on_close(move |_, _| {
                window.set(None);
                CloseDecision::Close
            });
        let id = Platform::get().launch_window(config).await;
        window.set(Some(id));
    });
}

/// Closes the display window if it's open.
pub fn close(mut window: State<Option<WindowId>>) {
    if let Some(id) = window.take() {
        Platform::get().close_window(id);
    }
}

struct DisplayWindow {
    scene: State<Scene>,
}

impl App for DisplayWindow {
    fn render(&self) -> impl IntoElement {
        crate::use_os_theme();
        let scene = self.scene.read();

        let toggle_fullscreen = |e: Event<KeyboardEventData>| {
            if e.key == Key::Named(NamedKey::F11) {
                Platform::get().with_window(Platform::window_id(), |window| {
                    let fullscreen = window.fullscreen().is_none();
                    window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
                });
            }
        };

        rect()
            .expanded()
            .padding(16.)
            .spacing(8.)
            .theme_background()
            .theme_color()
            .on_global_key_down(toggle_fullscreen)
            .children(scene.roots().iter().map(|&id| view(&scene, id)))
    }
}

/// Builds the element `id` and everything in it.
fn view(scene: &Scene, id: ElementId) -> Element {
    match scene.get(id) {
        Some(scene::Element::Rect { children }) => rect()
            .key(id)
            .padding(8.)
            .spacing(8.)
            .border(Border::new().fill((128, 128, 128)).width(1.))
            .children(children.iter().map(|&child| view(scene, child)))
            .into(),
        Some(scene::Element::Text { text }) => label().key(id).text(text.clone()).into(),
        // The scene only links ids of elements it has.
        None => rect().key(id).into(),
    }
}
