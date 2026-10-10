use freya::prelude::*;

use super::nav_back::NavBack;
use crate::live::Live;
use crate::scene::{self, ElementId, Names, Scene, use_scene};
use crate::stt::{Event, Status, use_stt};

#[derive(PartialEq)]
pub struct LiveSession;

impl Component for LiveSession {
    fn render(&self) -> impl IntoElement {
        let scene = use_scene();
        let names = use_state(Names::default);
        let mut status = use_state(|| Status::Loading);
        let mut live = Live::new(scene, names);
        use_stt(move |event| match event {
            Event::Started(device) => status.set(Status::Listening(device)),
            Event::Failed(err) => status.set(Status::Failed(err)),
            event => live.on_event(event),
        });

        let colors = use_theme().read().colors.clone();

        let top_bar = rect()
            .horizontal()
            .width(Size::fill())
            .padding(10.)
            .spacing(12.)
            .cross_align(Alignment::center())
            .background(colors.surface_primary)
            .child(NavBack)
            .child(
                label()
                    .width(Size::flex(1.))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .text(status.read().to_string())
                    .color(colors.text_secondary),
            );

        let rows = outline(&scene.read(), &names.read());
        let body = if rows.is_empty() {
            rect().padding(16.).child(
                label()
                    .text("No elements yet. Try saying \"rectangle\", then \"text in one hello\".")
                    .color(colors.text_secondary),
            )
        } else {
            rect().padding(16.).spacing(4.).children(rows)
        };

        rect()
            .expanded()
            .content(Content::Flex)
            .child(top_bar)
            .child(
                ScrollView::new()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
    }
}

fn outline(scene: &Scene, names: &Names) -> Vec<Element> {
    let mut rows = Vec::new();
    let mut pending: Vec<(ElementId, usize)> =
        scene.roots().iter().rev().map(|&id| (id, 0)).collect();
    while let Some((id, depth)) = pending.pop() {
        let description = match scene.get(id) {
            Some(scene::Element::Rect { children }) => {
                pending.extend(children.iter().rev().map(|&child| (child, depth + 1)));
                "rect".to_owned()
            }
            Some(scene::Element::Text { text }) => format!("text \"{text}\""),
            None => continue,
        };
        let names = names.of(id);
        let heading = if names.is_empty() {
            id.to_string()
        } else {
            format!("{id} ({})", names.join(", "))
        };
        rows.push(
            label()
                .key(id)
                .padding((0., 0., 0., depth as f32 * 24.))
                .font_size(18.)
                .text(format!("{heading}  {description}"))
                .into(),
        );
    }
    rows
}
