//! What's been said: the final text in segments, each with how its check went, the open
//! section's text, and which segments the agent has yet to finish acting on.

use std::ops::Range;
use std::time::Duration;

use super::check::Verdict;
use super::schedule::Decision;

/// Everything said so far.
#[derive(Debug, Default)]
pub struct Transcript {
    segments: Vec<Segment>,
    /// The open section's text so far; it may still change.
    live: String,
    /// The first segment the agent hasn't finished acting on.
    pending: usize,
}

/// A section of speech that won't change anymore. Its number is its place in the
/// transcript.
#[derive(Debug)]
pub struct Segment {
    pub text: String,
    /// How its check went; None while it's being checked.
    pub check: Option<Checked>,
}

/// How a segment's check went.
#[derive(Debug)]
pub struct Checked {
    /// What TypeSafe AI said, or why it couldn't be asked.
    pub verdict: Result<Verdict, String>,
    /// What was done about the segment.
    pub decision: Decision,
    /// How long TypeSafe AI took.
    pub took: Duration,
}

impl Transcript {
    /// Adds a segment that's still to be checked, and returns its number.
    pub fn push(&mut self, text: String) -> usize {
        self.segments.push(Segment { text, check: None });
        self.segments.len() - 1
    }

    /// Records how segment `number`'s check went.
    pub fn checked(&mut self, number: usize, checked: Checked) {
        self.segments[number].check = Some(checked);
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// How many segments there are, so also the next one's number.
    pub fn count(&self) -> usize {
        self.segments.len()
    }

    /// The text of the segments in `range`, with a space between them.
    pub fn text(&self, range: Range<usize>) -> String {
        let texts: Vec<&str> = self.segments[range]
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        texts.join(" ")
    }

    pub fn live(&self) -> &str {
        &self.live
    }

    pub fn set_live(&mut self, text: String) {
        self.live = text;
    }

    /// The first segment the agent hasn't finished acting on.
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// The agent has finished acting on every segment numbered below `end`.
    pub fn handled(&mut self, end: usize) {
        self.pending = self.pending.max(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript(texts: &[&str]) -> Transcript {
        let mut transcript = Transcript::default();
        for text in texts {
            transcript.push((*text).to_owned());
        }
        transcript
    }

    #[test]
    fn segments_are_numbered_in_order() {
        let mut transcript = transcript(&["add a box"]);
        assert_eq!(transcript.push("make it red".into()), 1);
        assert_eq!(transcript.count(), 2);
        assert!(
            transcript
                .segments()
                .iter()
                .all(|segment| segment.check.is_none())
        );
    }

    #[test]
    fn text_joins_a_range_of_segments() {
        let transcript = transcript(&["add a box", "um", "make it red"]);
        assert_eq!(transcript.text(0..3), "add a box um make it red");
        assert_eq!(transcript.text(1..3), "um make it red");
        assert_eq!(transcript.text(1..1), "");
    }

    #[test]
    fn handled_only_moves_forward() {
        let mut transcript = transcript(&["a", "b", "c"]);
        assert_eq!(transcript.pending(), 0);
        transcript.handled(2);
        assert_eq!(transcript.pending(), 2);
        transcript.handled(1);
        assert_eq!(transcript.pending(), 2);
        assert_eq!(transcript.text(transcript.pending()..3), "c");
    }

    #[test]
    fn checked_records_on_the_segment() {
        let mut transcript = transcript(&["a", "b"]);
        let checked = Checked {
            verdict: Err("offline".into()),
            decision: Decision::Started,
            took: Duration::from_millis(5),
        };
        transcript.checked(1, checked);
        assert!(transcript.segments()[0].check.is_none());
        let check = transcript.segments()[1].check.as_ref().unwrap();
        assert_eq!(check.decision, Decision::Started);
    }
}
