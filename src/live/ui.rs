//! The live screen's panels: the transcript with how each segment was checked, and what
//! the agent is doing.

mod activity;
mod transcript;

use std::time::Duration;

use freya::prelude::*;

pub use activity::ActivityPanel;
pub use transcript::TranscriptPanel;

/// A panel's title row, saying what it shows.
fn heading(text: String, colors: &ColorsSheet) -> impl IntoElement {
    rect()
        .width(Size::fill())
        .padding((10., 12.))
        .background(colors.surface_primary)
        .child(
            label()
                .width(Size::fill())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(13.)
                .color(colors.text_secondary)
                .text(text),
        )
}

/// `duration` the way people read it: milliseconds under a second, seconds above.
fn duration_text(duration: Duration) -> String {
    if duration < Duration::from_secs(1) {
        format!("{} ms", duration.as_millis())
    } else {
        format!("{:.1} s", duration.as_secs_f64())
    }
}

/// A scroll position that starts at the end, for content that grows at the end.
fn follow_end() -> ScrollConfig {
    ScrollConfig {
        default_vertical_position: ScrollPosition::End,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use freya::prelude::*;
    use freya_testing::TestingRunner;

    use super::*;

    /// The text of every label on screen, in order.
    pub fn labels(test: &TestingRunner) -> Vec<String> {
        test.find_many(|_, element| {
            Label::try_downcast(element).map(|label| label.text.into_owned())
        })
    }

    #[test]
    fn durations_read_as_milliseconds_or_seconds() {
        assert_eq!(duration_text(Duration::from_millis(640)), "640 ms");
        assert_eq!(duration_text(Duration::from_millis(999)), "999 ms");
        assert_eq!(duration_text(Duration::from_millis(1000)), "1.0 s");
        assert_eq!(duration_text(Duration::from_millis(4150)), "4.2 s");
    }
}
