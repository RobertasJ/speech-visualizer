//! Asking TypeSafe AI about a segment of speech: does it need the agent at all, and if
//! the agent is running, does it correct what that run is doing?

use serde_json::json;

use crate::typesafeai::{self, Noul, Request};

/// How likely a yes has to be to count, for each question. Missing a request is worse
/// than running the agent for nothing, but a wrong correction cancels a run.
const NEW_THRESHOLD: f32 = 0.3;
const CORRECTION_THRESHOLD: f32 = 0.5;

/// How many sentences of what was said before a segment its check is given. More
/// makes requests read as asked for already.
const EARLIER_SENTENCES: usize = 3;

/// The question asking whether a segment asks for anything new.
const NEW: &str = "new";
const NEW_QUESTION: &str = "Someone is talking, and an assistant listens and changes \
    what a display shows for them. `new_text` is what they just said, `earlier` is the \
    few sentences before it, and `acting_on` is what the assistant is working on right \
    now, if anything. Does `new_text` want anything shown on the display or changed on \
    it, beyond what `acting_on` already does?";
const NEW_YES: &str = "It asks, however indirectly, politely or vaguely, for something \
    to be shown, added, changed, moved, removed or cleared, like \"show me…\", \"can \
    you…\", \"I need…\", \"help me with…\" or \"what about…\", or asks a question \
    the display could answer, or corrects or takes back something asked for. Asking \
    again for something said `earlier` counts, since it may not have been done. When \
    unsure, yes.";
const NEW_NO: &str = "It's only filler or a fragment with no request in it, talk \
    plainly not meant for the assistant, or only repeats what `acting_on` already does.";

/// The question asking whether a segment corrects what the agent is doing.
const CORRECTION: &str = "correction";
const CORRECTION_QUESTION: &str = "Someone is speaking commands that change what a \
    display shows, and an assistant is carrying out `acting_on`. `earlier` is what they \
    said before `new_text`. Does `new_text` correct it, rather than asking for \
    something more?";
const CORRECTION_YES: &str = "It changes, fixes or takes back what `acting_on` asks for, \
    like \"no, the other box\", \"I meant blue\" or \"wait, not that\", or fixes a word \
    in it that was misheard.";
const CORRECTION_NO: &str = "It asks for something new, adds to what `acting_on` asks \
    for, or isn't a request at all.";

/// What TypeSafe AI said about a segment: how likely each answer is a yes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Verdict {
    /// That it asks for anything not asked for already.
    pub new: f32,
    /// That it corrects what the running agent is doing; None if none was running.
    pub correction: Option<f32>,
}

impl Verdict {
    /// Whether it needs the agent: it asks for something new, or corrects it.
    pub fn needs_agent(&self) -> bool {
        self.new > NEW_THRESHOLD || self.corrects()
    }

    /// Whether it corrects what the running agent is doing.
    pub fn corrects(&self) -> bool {
        self.correction
            .is_some_and(|correction| correction > CORRECTION_THRESHOLD)
    }
}

/// Whether a segment needs the agent, going by its check. One whose check failed does:
/// better to run the agent for nothing than to miss a request.
pub fn needs_agent(verdict: &Result<Verdict, String>) -> bool {
    verdict.as_ref().map_or(true, Verdict::needs_agent)
}

/// A check of one segment, ready to send.
pub struct Check {
    request: Request,
    /// Whether it asks the correction question, which it does if the agent is running.
    asks_correction: bool,
}

impl Check {
    /// A check of `text`, given what was said `earlier` (only its last few sentences are
    /// sent) and what the running agent is `acting_on`, if one is running. `earlier`
    /// mustn't include `text`, or it reads as asked for already.
    pub fn new(earlier: &str, acting_on: Option<&str>, text: &str) -> Self {
        let state = json!({
            "earlier": last_sentences(earlier, EARLIER_SENTENCES),
            "acting_on": acting_on,
            "new_text": text,
        });
        let mut request = Request::new(state)
            .with_question(NEW, Noul::new(NEW_QUESTION).with_criteria(NEW_YES, NEW_NO));
        if acting_on.is_some() {
            request = request.with_question(
                CORRECTION,
                Noul::new(CORRECTION_QUESTION).with_criteria(CORRECTION_YES, CORRECTION_NO),
            );
        }
        Check {
            request,
            asks_correction: acting_on.is_some(),
        }
    }

    pub async fn send(self) -> Result<Verdict, typesafeai::Error> {
        let response = typesafeai::send(&self.request).await?;
        Ok(Verdict {
            new: response.get_noul(NEW),
            correction: self.asks_correction.then(|| response.get_noul(CORRECTION)),
        })
    }
}

/// The last `count` sentences of `text`, or all of it if it has no more. A sentence
/// ends at a `.`, `!` or `?` followed by a space.
fn last_sentences(text: &str, count: usize) -> &str {
    let text = text.trim();
    // Where each sentence but the first starts.
    let starts: Vec<usize> = text
        .char_indices()
        .zip(text.chars().skip(1))
        .filter(|&((_, mark), next)| matches!(mark, '.' | '!' | '?') && next.is_whitespace())
        .map(|((at, mark), _)| at + mark.len_utf8())
        .collect();
    match starts.len().checked_sub(count) {
        Some(skipped) if count > 0 => text[starts[skipped]..].trim_start(),
        Some(_) => "",
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn sent(check: &Check) -> Value {
        serde_json::to_value(&check.request).unwrap()
    }

    #[test]
    fn asks_about_corrections_only_while_the_agent_runs() {
        let idle = sent(&Check::new("add a box", None, "make it red"));
        assert_eq!(
            idle["state"],
            json!({"earlier": "add a box", "acting_on": null, "new_text": "make it red"})
        );
        assert!(idle["questions"].get(NEW).is_some());
        assert!(idle["questions"].get(CORRECTION).is_none());

        let running = sent(&Check::new("", Some("add a box"), "no, a circle"));
        assert_eq!(running["state"]["acting_on"], "add a box");
        assert!(running["questions"].get(CORRECTION).is_some());
    }

    #[test]
    fn a_failed_check_counts_as_needing_the_agent() {
        assert!(needs_agent(&Err("offline".into())));
        let verdict = Verdict {
            new: 0.04,
            correction: None,
        };
        assert!(!needs_agent(&Ok(verdict)));
    }

    #[test]
    fn verdicts_count_above_the_threshold() {
        let verdict = |new, correction| Verdict { new, correction };
        assert!(verdict(0.97, None).needs_agent());
        assert!(!verdict(0.04, None).needs_agent());
        assert!(verdict(0.35, None).needs_agent());
        assert!(!verdict(0.3, None).needs_agent());
        // A correction needs the agent, whatever the other answer said.
        assert!(verdict(0.2, Some(0.9)).needs_agent());
        assert!(verdict(0.2, Some(0.9)).corrects());
        assert!(!verdict(0.9, Some(0.1)).corrects());
        assert!(!verdict(0.9, Some(0.5)).corrects());
        assert!(!verdict(0.9, None).corrects());
    }

    #[test]
    fn checks_get_the_last_few_sentences_said_before() {
        let said = "First one. Second one! Third one? Fourth one... and fifth";
        assert_eq!(
            last_sentences(said, 3),
            "Third one? Fourth one... and fifth"
        );
        assert_eq!(last_sentences(said, 5), said);
        assert_eq!(last_sentences(said, 9), said);
        assert_eq!(last_sentences(" no marks at all ", 2), "no marks at all");
        assert_eq!(last_sentences("Ends here. ", 1), "Ends here.");
        assert_eq!(last_sentences("a. b. c.", 0), "");
        assert_eq!(last_sentences("", 3), "");

        let check = Check::new(said, None, "show it");
        assert_eq!(
            sent(&check)["state"]["earlier"],
            "Third one? Fourth one... and fifth"
        );
    }
}
