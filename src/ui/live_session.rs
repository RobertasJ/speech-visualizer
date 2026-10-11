use freya::prelude::*;

use super::nav_back::NavBack;
use crate::live::{ActivityPanel, TranscriptPanel, use_live};
use crate::scene::use_scene;

/// Live mode, for debugging it: what's been said and how each part was checked on the
/// left, what the agent is doing on the right. The scene shows in its own window.
#[derive(PartialEq)]
pub struct LiveSession;

impl Component for LiveSession {
    fn render(&self) -> impl IntoElement {
        let scene = use_scene();
        let live = use_live(scene);
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

        let divider = rect()
            .width(Size::px(1.))
            .height(Size::fill())
            .background(colors.border);

        rect()
            .expanded()
            .content(Content::Flex)
            .child(top_bar)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .content(Content::Flex)
                    .child(TranscriptPanel {
                        transcript: live.transcript(),
                    })
                    .child(divider)
                    .child(ActivityPanel {
                        history: live.history(),
                        transcript: live.transcript(),
                        schedule: live.schedule(),
                    }),
            )
    }
}
