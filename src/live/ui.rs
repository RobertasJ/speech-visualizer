//! The live screen's panels: the transcript with how each segment was checked, and what
//! the agent is doing.

mod activity;
mod transcript;

use std::time::Duration;

use freya::prelude::*;

use super::usage::Usage;

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

/// What requests used and cost, like `2 requests · 12,345 in · 678 out (300 thinking) ·
/// $0.0016`. Cache reads and writes are counted in, and noted if there are any.
fn usage_text(usage: &Usage) -> String {
    let plural = if usage.requests == 1 { "" } else { "s" };
    let mut parts = vec![format!("{} request{plural}", usage.requests)];
    let input = usage.input + usage.cache_read + usage.cache_write;
    let mut input_text = format!("{} in", thousands(input));
    if usage.cache_read + usage.cache_write > 0 {
        input_text.push_str(&format!(
            " ({} cached, {} cache written)",
            thousands(usage.cache_read),
            thousands(usage.cache_write)
        ));
    }
    parts.push(input_text);
    let mut output_text = format!("{} out", thousands(usage.output));
    if usage.thinking > 0 {
        output_text.push_str(&format!(" ({} thinking)", thousands(usage.thinking)));
    }
    parts.push(output_text);
    parts.push(match usage.dollars {
        Some(dollars) => format!("${dollars:.4}"),
        None => "price unknown".to_owned(),
    });
    parts.join(" · ")
}

/// `number` with commas between thousands.
fn thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
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
    fn usage_reads_as_requests_tokens_and_dollars() {
        let usage = Usage {
            requests: 2,
            input: 12_345,
            output: 678,
            thinking: 300,
            dollars: Some(0.00157),
            ..Usage::default()
        };
        assert_eq!(
            usage_text(&usage),
            "2 requests · 12,345 in · 678 out (300 thinking) · $0.0016"
        );
        let cached = Usage {
            requests: 1,
            input: 10,
            cache_read: 1_000,
            cache_write: 5,
            ..Usage::default()
        };
        assert_eq!(
            usage_text(&cached),
            "1 request · 1,015 in (1,000 cached, 5 cache written) · 0 out · price unknown"
        );
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(999), "999");
    }

    #[test]
    fn durations_read_as_milliseconds_or_seconds() {
        assert_eq!(duration_text(Duration::from_millis(640)), "640 ms");
        assert_eq!(duration_text(Duration::from_millis(999)), "999 ms");
        assert_eq!(duration_text(Duration::from_millis(1000)), "1.0 s");
        assert_eq!(duration_text(Duration::from_millis(4150)), "4.2 s");
    }
}
