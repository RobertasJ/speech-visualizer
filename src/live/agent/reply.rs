//! Following one reply as it streams in: what the agent writes goes into the history as
//! it arrives, and each call comes out once it's complete, to be run.

use rig::completion::AssistantContent;
use rig::message::ToolCall;
use rig::streaming::{PartKind, StreamEvent};

use super::tools;
use crate::live::history::History;

/// A reply being read.
#[derive(Debug, Default)]
pub struct Reply {
    /// The arguments of the call being streamed, so far.
    arguments: String,
}

impl Reply {
    /// Takes the reply's next event, recording what it shows in `history`. Returns a
    /// call once it's complete.
    pub fn event(&mut self, event: StreamEvent, history: &mut History) -> Option<ToolCall> {
        match event {
            StreamEvent::Start {
                kind: PartKind::Text,
                ..
            } => history.paragraph(),
            StreamEvent::Text { text, .. } => history.text(&text),
            StreamEvent::Start {
                kind: PartKind::ToolCall,
                ..
            } => {
                self.arguments.clear();
                history.script();
            }
            StreamEvent::Arguments { json, .. } => {
                self.arguments.push_str(&json);
                if let Some(code) = tools::partial_code(&self.arguments) {
                    history.code(code);
                }
            }
            StreamEvent::End {
                content: AssistantContent::ToolCall(call),
                ..
            } => return Some(call),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use rig::message::{ToolFunction, ToolName};
    use rig::streaming::Part;
    use serde_json::json;

    use super::*;
    use crate::live::history::Entry;

    fn part(index: u32) -> Part {
        serde_json::from_value(json!(index)).unwrap()
    }

    fn start(index: u32, kind: PartKind) -> StreamEvent {
        StreamEvent::Start {
            part: part(index),
            kind,
            name: None,
        }
    }

    fn text(index: u32, text: &str) -> StreamEvent {
        StreamEvent::Text {
            part: part(index),
            text: text.to_owned(),
        }
    }

    fn arguments(index: u32, json: &str) -> StreamEvent {
        StreamEvent::Arguments {
            part: part(index),
            json: json.to_owned(),
        }
    }

    fn end_call(index: u32, code: &str) -> StreamEvent {
        let name = ToolName::new(tools::NAME).unwrap();
        let function = ToolFunction::new(name, json!({"code": code}));
        StreamEvent::End {
            part: part(index),
            content: AssistantContent::ToolCall(ToolCall::from_wire("call", function)),
        }
    }

    /// Feeds `events` to a reply, returning the code of each call that came out.
    fn read(events: Vec<StreamEvent>, history: &mut History) -> Vec<String> {
        let mut reply = Reply::default();
        events
            .into_iter()
            .filter_map(|event| reply.event(event, history))
            .map(|call| tools::code(&call).unwrap().to_owned())
            .collect()
    }

    #[test]
    fn text_goes_into_the_history_as_it_streams() {
        let mut history = History::default();
        history.begin("add a box".into());
        let calls = read(
            vec![
                start(0, PartKind::Text),
                text(0, "Adding "),
                text(0, "a box."),
            ],
            &mut history,
        );
        assert!(calls.is_empty());
        assert_eq!(
            history.runs()[0].entries,
            [Entry::Text("Adding a box.".into())]
        );
    }

    #[test]
    fn a_call_shows_its_code_while_streaming_and_comes_out_when_complete() {
        let mut history = History::default();
        history.begin("add a box".into());
        let mut reply = Reply::default();
        for event in [
            start(1, PartKind::ToolCall),
            arguments(1, r#"{"code": "let b"#),
            arguments(1, r#" = rect();"#),
        ] {
            assert!(reply.event(event, &mut history).is_none());
        }
        assert_eq!(
            history.runs()[0].entries,
            [Entry::Script {
                code: "let b = rect();".into(),
                outcome: None,
            }]
        );

        let call = reply
            .event(end_call(1, "let b = rect();"), &mut history)
            .unwrap();
        assert_eq!(tools::code(&call), Ok("let b = rect();"));
    }

    #[test]
    fn each_call_starts_from_fresh_arguments() {
        let mut history = History::default();
        history.begin("two boxes".into());
        let calls = read(
            vec![
                start(0, PartKind::ToolCall),
                arguments(0, r#"{"code": "rect();"}"#),
                end_call(0, "rect();"),
                start(1, PartKind::ToolCall),
                arguments(1, r#"{"code": "te"#),
            ],
            &mut history,
        );
        assert_eq!(calls, ["rect();"]);
        // The second call's code so far isn't mixed with the first's arguments.
        assert_eq!(
            history.runs()[0].entries[1],
            Entry::Script {
                code: "te".into(),
                outcome: None,
            }
        );
    }
}
