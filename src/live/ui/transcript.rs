use freya::prelude::*;

use super::{duration_text, follow_end, heading};
use crate::live::check::Verdict;
use crate::live::transcript::{Checked, Segment, Transcript};

/// What's been said, segment by segment, each with what TypeSafe AI said about it and
/// what was done; then what's being said right now.
#[derive(PartialEq)]
pub struct TranscriptPanel {
    pub transcript: State<Transcript>,
}

impl Component for TranscriptPanel {
    fn render(&self) -> impl IntoElement {
        let transcript = self.transcript;
        let mut scroll = use_scroll_controller(follow_end);
        use_side_effect(move || {
            transcript.read();
            scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
        });
        let colors = use_theme().read().colors.clone();

        let transcript = transcript.read();
        let segments = transcript
            .segments()
            .iter()
            .enumerate()
            .map(|(number, segment)| segment_view(number, segment, &colors));
        let mut body = rect()
            .width(Size::fill())
            .padding(12.)
            .spacing(10.)
            .children(segments);
        if !transcript.live().is_empty() {
            body = body.child(
                label()
                    .width(Size::fill())
                    .color(colors.text_secondary)
                    .font_slant(FontSlant::Italic)
                    .text(transcript.live().to_owned()),
            );
        } else if transcript.count() == 0 {
            body = body.child(label().color(colors.text_secondary).text("Say something."));
        }

        rect()
            .width(Size::flex(1.))
            .height(Size::fill())
            .content(Content::Flex)
            .child(heading("Transcript".to_owned(), &colors))
            .child(
                ScrollView::new_controlled(scroll)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
    }
}

/// A segment's text, with its check under it: the scores, what was decided, and how
/// long TypeSafe AI took.
fn segment_view(number: usize, segment: &Segment, colors: &ColorsSheet) -> Element {
    let (check, color) = match &segment.check {
        None => ("checking…".to_owned(), colors.text_secondary),
        Some(Checked {
            verdict,
            decision,
            took,
        }) => {
            let (scores, color) = match verdict {
                Ok(Verdict {
                    new,
                    correction: Some(correction),
                }) => (
                    format!("new {new:.2} · correction {correction:.2}"),
                    colors.text_secondary,
                ),
                Ok(Verdict {
                    new,
                    correction: None,
                }) => (format!("new {new:.2}"), colors.text_secondary),
                Err(error) => (format!("check failed: {error}"), colors.error),
            };
            let took = duration_text(*took);
            (format!("{scores} → {decision} · {took}"), color)
        }
    };
    rect()
        .key(number)
        .width(Size::fill())
        .spacing(2.)
        .child(label().width(Size::fill()).text(segment.text.clone()))
        .child(
            label()
                .width(Size::fill())
                .font_size(12.)
                .color(color)
                .text(check),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use freya_testing::TestingRunner;

    use super::*;
    use crate::live::schedule::Decision;
    use crate::live::ui::tests::labels;

    fn panel() -> impl IntoElement {
        use_init_theme(Theme::default);
        TranscriptPanel {
            transcript: use_consume(),
        }
    }

    #[test]
    fn shows_each_segment_with_its_check_then_the_live_text() {
        let (mut test, transcript) = TestingRunner::new(
            panel,
            (400., 600.).into(),
            |runner| runner.provide_root_context(|| State::create(Transcript::default())),
            1.,
        );
        test.sync_and_update();
        assert!(labels(&test).contains(&"Say something.".to_owned()));

        test.run_in(|| {
            let mut transcript = transcript;
            let mut transcript = transcript.write();
            transcript.push("add a box".into());
            let verdict = Verdict {
                new: 0.97,
                correction: None,
            };
            let checked = Checked {
                verdict: Ok(verdict),
                decision: Decision::Started,
                took: Duration::from_millis(640),
            };
            transcript.checked(0, checked);
            transcript.push("no, a circle".into());
            let checked = Checked {
                verdict: Err("TYPESAFE_API_KEY not set".into()),
                decision: Decision::Queued,
                took: Duration::from_millis(1500),
            };
            transcript.checked(1, checked);
            transcript.push("um".into());
            transcript.set_live("and make it".into());
        });
        test.sync_and_update();

        let labels = labels(&test);
        for expected in [
            "Transcript",
            "add a box",
            "new 0.97 → started · 640 ms",
            "no, a circle",
            "check failed: TYPESAFE_API_KEY not set → queued · 1.5 s",
            "um",
            "checking…",
            "and make it",
        ] {
            assert!(
                labels.iter().any(|label| label == expected),
                "{expected} in {labels:?}"
            );
        }
        assert!(!labels.contains(&"Say something.".to_owned()));
    }
}
