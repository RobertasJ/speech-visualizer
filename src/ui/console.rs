use freya::prelude::*;

use super::nav_back::NavBack;
use crate::scene::{FUNCTIONS, Outcome, Script, use_scene};

#[derive(PartialEq)]
pub struct Console;

impl Component for Console {
    fn render(&self) -> impl IntoElement {
        let scene = use_scene();
        // Its variables last as long as the screen.
        let mut script = use_state(|| Script::new(scene));
        let mut input = use_state(String::new);
        // Typed scripts and what came of them, oldest first.
        let mut history = use_state(Vec::<(String, Outcome)>::new);
        let mut scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });

        let on_submit = move |line: String| {
            input.set(String::new());
            scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
            let outcome = script.write().run(&line);
            history.write().push((line, outcome));
        };

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
                    .text("F11 in the display window toggles fullscreen")
                    .color(colors.text_secondary),
            );

        // The script functions, one per row.
        let help = ScrollView::new()
            .width(Size::fill())
            .height(Size::px(110.))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((8., 12.))
                    .spacing(2.)
                    .children(FUNCTIONS.iter().enumerate().map(|(i, function)| {
                        rect()
                            .key(i)
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .spacing(12.)
                            .child(
                                label()
                                    .width(Size::px(230.))
                                    .font_size(13.)
                                    .font_family("monospace")
                                    .text(function.signature),
                            )
                            .child(
                                label()
                                    .width(Size::flex(1.))
                                    .font_size(13.)
                                    .color(colors.text_secondary)
                                    .text(function.description),
                            )
                            .into_element()
                    })),
            );

        let log =
            rect()
                .padding((0., 12.))
                .spacing(4.)
                .children(
                    history
                        .read()
                        .iter()
                        .enumerate()
                        .map(|(i, (line, outcome))| {
                            let output = outcome.output.iter().map(|line| {
                                label()
                                    .text(line.clone())
                                    .color(colors.text_secondary)
                                    .into_element()
                            });
                            let end = match &outcome.error {
                                Some(error) => {
                                    Some(label().text(error.clone()).color(colors.error))
                                }
                                None if outcome.output.is_empty() => {
                                    Some(label().text("ok").color(colors.text_secondary))
                                }
                                None => None,
                            };
                            rect()
                                .key(i)
                                .child(label().text(format!("> {line}")).font_tabular())
                                .children(output)
                                .children(end.map(IntoElement::into_element))
                                .into_element()
                        }),
                );

        rect()
            .expanded()
            .content(Content::Flex)
            .child(top_bar)
            .child(help)
            .child(
                ScrollView::new_controlled(scroll)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(log),
            )
            .child(
                rect().width(Size::fill()).padding(12.).child(
                    Input::new(input)
                        .width(Size::fill())
                        .placeholder("Type a script and press Enter")
                        .auto_focus(true)
                        .on_submit(on_submit),
                ),
            )
    }
}
