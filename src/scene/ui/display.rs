use std::rc::Rc;

use freya::prelude::*;
use freya::winit::window::Fullscreen;

use super::window::{WindowHandle, spawn_window};
use crate::scene::{self, ElementId, Scene};

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
        let root = self
            .scene
            .try_read()
            .map(|scene| view(&scene, ElementId::ROOT));

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
            .theme_background()
            .theme_color()
            .on_global_key_down(toggle_fullscreen)
            .children(root)
    }
}

/// The element `id` is, as Freya draws it, with what's in it: the scene keeps Freya's
/// own elements, so this only adds their children, and a paragraph's spans.
fn view(scene: &Scene, id: ElementId) -> Element {
    let key = DiffKey::from(&id.0);
    let children = scene.children(id).unwrap_or_default();
    match scene.get(id) {
        Some(scene::Element::Rect(rect)) => Element::Element {
            key,
            element: Rc::new(rect.clone()),
            elements: children.iter().map(|&child| view(scene, child)).collect(),
        },
        Some(scene::Element::Paragraph(paragraph)) => {
            let mut paragraph = paragraph.clone();
            paragraph.spans = children
                .iter()
                .filter_map(|&child| match scene.get(child) {
                    Some(scene::Element::Span(span)) => Some(span.clone()),
                    _ => None,
                })
                .collect();
            Element::Element {
                key,
                element: Rc::new(paragraph),
                elements: Vec::new(),
            }
        }
        // Spans are drawn by their paragraph, and the scene only links ids it has.
        Some(scene::Element::Span(_)) | None => rect().key(id.0).into(),
    }
}

#[cfg(test)]
mod tests {
    use freya::elements::paragraph::ParagraphElement;
    use freya::elements::rect::RectElement;

    use super::*;

    /// The rect `element` draws, and what's in it.
    fn rect_of(element: &Element) -> (RectElement, &[Element]) {
        let Element::Element {
            element, elements, ..
        } = element
        else {
            panic!("expected an element")
        };
        (
            Rect::try_downcast(element.as_ref()).expect("a rect"),
            elements,
        )
    }

    /// The paragraph `element` draws, and its inline elements.
    fn paragraph_of(element: &Element) -> (ParagraphElement, &[Element]) {
        let Element::Element {
            element, elements, ..
        } = element
        else {
            panic!("expected an element")
        };
        let paragraph = Paragraph::try_downcast(element.as_ref()).expect("a paragraph");
        (paragraph, elements)
    }

    #[test]
    fn draws_the_scenes_own_elements_with_their_children() {
        let mut scene = Scene::default();
        let card = scene.add_rect(ElementId::ROOT).unwrap();
        scene.set_property(card, "background", "red").unwrap();
        let paragraph = scene.add_paragraph(card).unwrap();
        scene.set_property(paragraph, "max_lines", "2").unwrap();
        let hello = scene.add_span(paragraph, "Hello ".into()).unwrap();
        scene.set_property(hello, "font_weight", "bold").unwrap();
        scene.add_span(paragraph, "world".into()).unwrap();

        let root = view(&scene, ElementId::ROOT);
        let (root_rect, cards) = rect_of(&root);
        assert_eq!(root_rect.layout.layout.width, Size::fill());

        let (card_rect, paragraphs) = rect_of(&cards[0]);
        assert_eq!(card_rect.style.background, Fill::Color(Color::RED));

        let (drawn, inline) = paragraph_of(&paragraphs[0]);
        assert!(inline.is_empty());
        assert_eq!(drawn.max_lines, Some(2));
        let texts: Vec<&str> = drawn.spans.iter().map(|span| span.text.as_ref()).collect();
        assert_eq!(texts, ["Hello ", "world"]);
        assert_eq!(
            drawn.spans[0].text_style_data.font_weight,
            Some(FontWeight::BOLD)
        );
    }
}
