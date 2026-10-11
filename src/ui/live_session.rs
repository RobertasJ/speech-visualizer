use freya::prelude::*;

use super::nav_back::NavBack;
use crate::live::use_live;
use crate::scene::{self, ElementId, Names, Scene, use_scene};

#[derive(PartialEq)]
pub struct LiveSession;

impl Component for LiveSession {
    fn render(&self) -> impl IntoElement {
        let scene = use_scene();
        let names = use_state(Names::default);
        let live = use_live(scene, names);

        // Both stay scrolled to the end as text comes in.
        let mut transcript_scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });
        let mut agent_scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });
        use_side_effect(move || {
            live.transcript().read();
            live.live_text().read();
            transcript_scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
        });
        use_side_effect(move || {
            live.responses().read();
            live.writing().read();
            agent_scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
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
                    .text(live.status().read().to_string())
                    .color(colors.text_secondary),
            );

        let transcript = live.transcript().read().clone();
        let live_text = live.live_text().read().clone();
        let transcript = if transcript.is_empty() && live_text.is_empty() {
            paragraph().span(Span::new("Say something.").color(colors.text_secondary))
        } else {
            let sep = if transcript.is_empty() { "" } else { " " };
            paragraph()
                .width(Size::fill())
                .span(Span::new(transcript))
                .span(
                    Span::new(format!("{sep}{live_text}"))
                        .color(colors.text_secondary)
                        .font_slant(FontSlant::Italic),
                )
        };

        let mut activity = vec!["Agent".to_owned()];
        if *live.thinking().read() {
            activity.push("thinking…".into());
        }
        if *live.checking().read() > 0 {
            activity.push("checking what was said…".into());
        }
        let queued = *live.queued().read();
        if queued > 0 {
            activity.push(format!("{queued} more queued"));
        }
        let agent_heading = label()
            .padding((8., 16., 4., 16.))
            .font_size(13.)
            .color(colors.text_secondary)
            .text(activity.join(" · "));
        let agent_output = agent_output(&live.responses().read(), &live.writing().read(), &colors);

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
                ScrollView::new_controlled(transcript_scroll)
                    .width(Size::fill())
                    .height(Size::px(96.))
                    .child(rect().padding((8., 16.)).child(transcript)),
            )
            .child(agent_heading)
            .child(
                ScrollView::new_controlled(agent_scroll)
                    .width(Size::fill())
                    .height(Size::px(160.))
                    .child(
                        rect()
                            .padding((0., 16., 8., 16.))
                            .spacing(8.)
                            .children(agent_output),
                    ),
            )
            .child(
                ScrollView::new()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
    }
}

/// The agent's responses, one block each, with the line it's writing at the end of the
/// last. Commands are in the normal color, how they went is dimmed (red if they
/// failed), and its reasoning is dimmed and in italics.
fn agent_output(responses: &[String], writing: &str, colors: &ColorsSheet) -> Vec<Element> {
    let last = responses.len().saturating_sub(1);
    responses
        .iter()
        .enumerate()
        .filter(|&(i, response)| !response.is_empty() || (i == last && !writing.is_empty()))
        .map(|(i, response)| {
            let lines = response.lines().enumerate().map(|(j, line)| {
                let label = label().key(j).font_size(14.).text(line.to_owned());
                if line.starts_with("> failed") {
                    label.color(colors.error)
                } else if line.starts_with('>') {
                    label.color(colors.text_secondary)
                } else if line.trim_start().starts_with('#') {
                    label
                } else {
                    label
                        .color(colors.text_secondary)
                        .font_slant(FontSlant::Italic)
                }
                .into_element()
            });
            let writing = (i == last && !writing.is_empty()).then(|| {
                label()
                    .key("writing")
                    .font_size(14.)
                    .color(colors.text_secondary)
                    .text(writing.to_owned())
                    .into_element()
            });
            rect()
                .key(i)
                .spacing(2.)
                .children(lines.chain(writing))
                .into_element()
        })
        .collect()
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
