use freya::prelude::*;
use freya::winit::window::Fullscreen;

use super::{ElementId, Scene};
use crate::window::{WindowHandle, spawn_window};

/// Opens a window showing `scene`, which closes when the handle is dropped.
pub fn spawn(scene: State<Scene>) -> WindowHandle {
    spawn_window(
        WindowConfig::new_app(DisplayWindow { scene })
            .with_size(800., 600.)
            .with_title("Speech visualizer display"),
    )
}

struct DisplayWindow {
    scene: State<Scene>,
}

impl App for DisplayWindow {
    fn render(&self) -> impl IntoElement {
        // Follow the OS light/dark preference in this window.
        let mut theme = use_init_theme(|| Platform::get().preferred_theme.read().to_theme());
        use_side_effect(move || theme.set(Platform::get().preferred_theme.read().to_theme()));

        // The scene goes with the component that owns it, which can unmount a moment
        // before this window is closed: show nothing until then.
        let elements: Vec<Element> = match self.scene.try_read() {
            Some(scene) => scene.roots().iter().map(|&id| view(&scene, id)).collect(),
            None => Vec::new(),
        };

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
            .children(elements)
    }
}

fn view(scene: &Scene, id: ElementId) -> Element {
    match scene.get(id) {
        Some(super::Element::Rect { children }) => rect()
            .key(id)
            .padding(8.)
            .spacing(8.)
            .border(Border::new().fill((128, 128, 128)).width(1.))
            .children(children.iter().map(|&child| view(scene, child)))
            .into(),
        Some(super::Element::Text { text }) => label().key(id).text(text.clone()).into(),
        // The scene only links ids of elements it has.
        None => rect().key(id).into(),
    }
}
