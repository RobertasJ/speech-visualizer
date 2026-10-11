//! Asking TypeSafe AI about a segment of speech: does it need the agent at all, and if
//! the agent is running, does it correct what that run is doing?

use serde_json::json;

use crate::typesafeai::{self, Noul, Request};

/// How likely a yes has to be to count.
const THRESHOLD: f32 = 0.5;

/// The question asking whether a segment asks for anything new.
const NEW: &str = "new";
const NEW_QUESTION: &str = "Someone is speaking to an assistant that changes what a \
    display shows. `earlier` is what they said before `new_text`, and `acting_on` is \
    what the assistant is carrying out right now, if anything. Does `new_text` ask for \
    anything to be done to the display that hasn't been asked for already?";
const NEW_YES: &str = "It asks for something to be added, changed, named, removed or \
    cleared, or corrects or takes back something asked for, even in passing.";
const NEW_NO: &str = "It's filler, thinking out loud, talk not meant for the assistant, \
    or only repeats what was already asked for.";

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
        self.new > THRESHOLD || self.corrects()
    }

    /// Whether it corrects what the running agent is doing.
    pub fn corrects(&self) -> bool {
        self.correction
            .is_some_and(|correction| correction > THRESHOLD)
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
    /// A check of `text`, given what was said `earlier` and what the running agent is
    /// `acting_on`, if one is running. `earlier` mustn't include `text`, or it reads as
    /// asked for already.
    pub fn new(earlier: &str, acting_on: Option<&str>, text: &str) -> Self {
        let state = json!({
            "earlier": earlier,
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
        assert!(!verdict(0.5, None).needs_agent());
        // A correction needs the agent, whatever the other answer said.
        assert!(verdict(0.2, Some(0.9)).needs_agent());
        assert!(verdict(0.2, Some(0.9)).corrects());
        assert!(!verdict(0.9, Some(0.1)).corrects());
        assert!(!verdict(0.9, None).corrects());
    }
}
