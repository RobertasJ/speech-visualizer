use freya::prelude::*;

use super::nav_back::NavBack;
use crate::scene::{Command, Names, use_scene};

#[derive(PartialEq)]
pub struct Console;

impl Component for Console {
    fn render(&self) -> impl IntoElement {
        let mut scene = use_scene();
        let mut names = use_state(Names::default);
        let mut input = use_state(String::new);
        // Typed lines and what came of them, oldest first.
        let mut history = use_state(Vec::<(String, Result<String, String>)>::new);
        let mut scroll = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..Default::default()
        });

        let on_submit = move |line: String| {
            input.set(String::new());
            scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
            let outcome = match line.parse::<Command>() {
                Ok(command) => match command.apply(&mut scene.write(), &mut names.write()) {
                    Ok(Some(id)) => Ok(format!("added {id}")),
                    Ok(None) => Ok("ok".into()),
                    Err(err) => Err(err.to_string()),
                },
                Err(err) => Err(err),
            };
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

        let help = label()
            .padding((8., 12.))
            .font_size(13.)
            .color(colors.text_secondary)
            .text(
                "rect(angle) [in <id>]  ·  text [in <id>] <text>  ·  set <id> <text>  ·  remove <id>  ·  name <id> <name>  ·  unname <name>  ·  unname all  ·  clear  —  a name works wherever an <id> does",
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
                            let (result, color) = match outcome {
                                Ok(message) => (message.as_str(), colors.text_secondary),
                                Err(message) => (message.as_str(), colors.error),
                            };
                            rect()
                                .key(i)
                                .child(label().text(format!("> {line}")).font_tabular())
                                .child(label().text(result.to_owned()).color(color))
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
                        .placeholder("Type a command and press Enter")
                        .auto_focus(true)
                        .on_submit(on_submit),
                ),
            )
    }
}
