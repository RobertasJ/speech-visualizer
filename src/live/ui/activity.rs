use freya::prelude::*;

use super::{duration_text, follow_end, heading};
use crate::live::history::{Entry, History, Run};
use crate::live::schedule::Schedule;
use crate::live::transcript::Transcript;
use crate::scene::Outcome;

/// What the agent is doing: whether it's running, how many segments are being checked
/// or queued, then each run with what it wrote, the scripts it ran and how they went.
#[derive(PartialEq)]
pub struct ActivityPanel {
    pub history: State<History>,
    /// For how many segments are still being checked.
    pub transcript: State<Transcript>,
    /// For how many segments are queued for the next run.
    pub schedule: State<Schedule>,
}

impl Component for ActivityPanel {
    fn render(&self) -> impl IntoElement {
        let history = self.history;
        let mut scroll = use_scroll_controller(follow_end);
        use_side_effect(move || {
            history.read();
            scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
        });
        let colors = use_theme().read().colors.clone();

        let history = history.read();
        let mut activity = vec!["Agent".to_owned()];
        if history.runs().last().is_some_and(Run::is_running) {
            activity.push("thinking…".to_owned());
        }
        let checking = self
            .transcript
            .read()
            .segments()
            .iter()
            .filter(|segment| segment.check.is_none())
            .count();
        if checking > 0 {
            activity.push(format!("checking {checking}…"));
        }
        let queued = self.schedule.read().queued();
        if queued > 0 {
            activity.push(format!("{queued} queued"));
        }

        let runs = history
            .runs()
            .iter()
            .enumerate()
            .map(|(index, run)| run_view(index, run, &colors));
        let mut body = rect()
            .width(Size::fill())
            .padding(12.)
            .spacing(20.)
            .children(runs);
        if history.runs().is_empty() {
            body = body.child(
                label()
                    .color(colors.text_secondary)
                    .text("Nothing yet: the agent runs when something said needs it."),
            );
        }

        rect()
            .width(Size::flex(1.))
            .height(Size::fill())
            .content(Content::Flex)
            .child(heading(activity.join(" · "), &colors))
            .child(
                ScrollView::new_controlled(scroll)
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
    }
}

/// A run: which one, its timings and the text it was for, then what it did.
fn run_view(index: usize, run: &Run, colors: &ColorsSheet) -> Element {
    let mut about = vec![format!("run {}", index + 1)];
    if let Some(first) = run.first_output {
        about.push(format!("first output {}", duration_text(first)));
    }
    about.push(match run.took {
        Some(took) => format!("took {}", duration_text(took)),
        None => "running…".to_owned(),
    });
    let entries = run
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| entry_view(i, entry, run.is_running(), colors));
    rect()
        .key(index)
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .width(Size::fill())
                .font_size(12.)
                .color(colors.text_secondary)
                .text(about.join(" · ")),
        )
        .child(
            label()
                .width(Size::fill())
                .max_lines(2)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .font_slant(FontSlant::Italic)
                .color(colors.text_secondary)
                .text(format!("for “{}”", run.new_text)),
        )
        .children(entries)
        .into()
}

/// Something the agent did, or None for text with nothing in it yet.
fn entry_view(index: usize, entry: &Entry, running: bool, colors: &ColorsSheet) -> Option<Element> {
    let view: Element = match entry {
        Entry::Text(text) if text.trim().is_empty() => return None,
        Entry::Text(text) => label()
            .width(Size::fill())
            .text(text.trim().to_owned())
            .into(),
        Entry::Script { code, outcome } => script_view(code, outcome.as_ref(), running, colors),
        Entry::Cancelled => label()
            .color(colors.warning)
            .text("cancelled: new speech corrected this")
            .into(),
        Entry::Failed(error) => label()
            .width(Size::fill())
            .color(colors.error)
            .text(format!("failed: {error}"))
            .into(),
    };
    Some(rect().key(index).width(Size::fill()).child(view).into())
}

/// A script's code in a box, then how it went: what it printed and its result, `ok`,
/// or its error in red.
fn script_view(
    code: &str,
    outcome: Option<&Outcome>,
    running: bool,
    colors: &ColorsSheet,
) -> Element {
    let lines = match outcome {
        Some(outcome) => outcome.lines(),
        // Still being written, or about to run.
        None if running => vec!["…".to_owned()],
        None => vec!["not run".to_owned()],
    };
    let failed = outcome.is_some_and(|outcome| outcome.error.is_some());
    let last = lines.len() - 1;
    let lines = lines.into_iter().enumerate().map(|(i, line)| {
        let color = if failed && i == last {
            colors.error
        } else {
            colors.text_secondary
        };
        label()
            .key(i)
            .width(Size::fill())
            .font_size(13.)
            .color(color)
            .text(format!("> {line}"))
            .into_element()
    });
    rect()
        .width(Size::fill())
        .spacing(2.)
        .child(
            rect()
                .width(Size::fill())
                .padding(8.)
                .corner_radius(6.)
                .background(colors.surface_secondary)
                .child(
                    label()
                        .width(Size::fill())
                        .font_family("monospace")
                        .font_size(13.)
                        .text(code.to_owned()),
                ),
        )
        .children(lines)
        .into()
}

#[cfg(test)]
mod tests {
    use freya_testing::TestingRunner;

    use super::*;
    use crate::live::ui::tests::labels;

    fn panel() -> impl IntoElement {
        use_init_theme(Theme::default);
        ActivityPanel {
            history: use_consume(),
            transcript: use_consume(),
            schedule: use_consume(),
        }
    }

    type States = (State<History>, State<Transcript>, State<Schedule>);

    fn runner() -> (TestingRunner, States) {
        TestingRunner::new(
            panel,
            (400., 800.).into(),
            |runner| {
                (
                    runner.provide_root_context(|| State::create(History::default())),
                    runner.provide_root_context(|| State::create(Transcript::default())),
                    runner.provide_root_context(|| State::create(Schedule::default())),
                )
            },
            1.,
        )
    }

    fn has(labels: &[String], expected: &str) -> bool {
        labels.iter().any(|label| label == expected)
    }

    #[test]
    fn says_when_nothing_has_run() {
        let (mut test, _) = runner();
        test.sync_and_update();
        let labels = labels(&test);
        assert!(has(&labels, "Agent"), "{labels:?}");
        assert!(has(
            &labels,
            "Nothing yet: the agent runs when something said needs it."
        ));
    }

    #[test]
    fn heading_says_what_the_agent_is_doing() {
        let (mut test, (history, transcript, schedule)) = runner();
        test.run_in(|| {
            let (mut history, mut transcript, mut schedule) = (history, transcript, schedule);
            history.write().begin("add a box".into());
            transcript.write().push("and a circle".into());
            schedule.write().start(1);
            schedule.write().decide(2, true, false, true);
        });
        test.sync_and_update();
        let labels = labels(&test);
        assert!(
            has(&labels, "Agent · thinking… · checking 1… · 1 queued"),
            "{labels:?}"
        );

        test.run_in(|| {
            let mut history = history;
            history.write().end();
        });
        test.sync_and_update();
        assert!(
            !labels_of(&mut test)
                .iter()
                .any(|label| label.contains("thinking"))
        );
    }

    fn labels_of(test: &mut TestingRunner) -> Vec<String> {
        test.sync_and_update();
        labels(test)
    }

    #[test]
    fn shows_a_run_with_its_text_scripts_and_outcomes() {
        let (mut test, (history, ..)) = runner();
        test.run_in(|| {
            let mut history = history;
            let mut history = history.write();
            history.begin("add a box".into());
            history.paragraph();
            history.text("A box.");
            history.script();
            history.ran("let b = rect();".into(), Outcome::default());
            history.script();
            let failed = Outcome {
                output: vec!["#1 rect".into()],
                error: Some("no element 9".into()),
            };
            history.ran("print(b); el(9)".into(), failed);
            history.script();
            history.code("b.desc".into());
        });
        let labels = labels_of(&mut test);
        for expected in [
            "for “add a box”",
            "A box.",
            "let b = rect();",
            "> ok",
            "print(b); el(9)",
            "> #1 rect",
            "> error: no element 9",
            "b.desc",
            "> …",
        ] {
            assert!(has(&labels, expected), "{expected} in {labels:?}");
        }
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("run 1 · ") && label.ends_with("running…"))
        );

        test.run_in(|| {
            let mut history = history;
            history.write().cancelled();
        });
        let labels = labels_of(&mut test);
        assert!(has(&labels, "> not run"), "{labels:?}");
        assert!(has(&labels, "cancelled: new speech corrected this"));
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("run 1 · ") && label.contains("took "))
        );
    }

    #[test]
    fn shows_a_failed_request() {
        let (mut test, (history, ..)) = runner();
        test.run_in(|| {
            let mut history = history;
            let mut history = history.write();
            history.begin("add a box".into());
            history.failed("ANTHROPIC_API_KEY not set".into());
            history.end();
        });
        let labels = labels_of(&mut test);
        assert!(
            has(&labels, "failed: ANTHROPIC_API_KEY not set"),
            "{labels:?}"
        );
    }
}
