//! One run of the agent. It's given the transcript and the scene, and changes the scene
//! by calling `run_script`, getting each script's outcome back right away, until it
//! replies without calling it. A run can make any number of calls.

mod input;
mod reply;
mod tools;

use freya::prelude::*;
use futures_lite::StreamExt;
use rig::Model;
use rig::completion::{CompletionRequest, Effort, Message, Reasoning};
use rig::message::ToolResult;
use rig::providers::anthropic::{Anthropic, Messages};
use rig::streaming::Item;

pub use input::Input;

use super::history::History;
use super::usage;
use crate::scene::{Scene, Script};
use reply::Reply;

/// Quick, but may need more thinking than [`REASONING`] gives it to do well.
const MODEL: &str = "claude-haiku-5-5";
/// How much the model thinks before answering; raise it if the agent does badly.
const REASONING: Reasoning = Reasoning::Effort(Effort::Low);
/// rig only knows the output limit of the models it lists, and it doesn't list this
/// one, so it needs one given.
const MAX_TOKENS: u64 = 64_000;

/// The model the agent runs on, or why there's none.
pub fn model() -> Result<Model<Messages>, String> {
    let provider = Anthropic::from_env().map_err(|err| err.to_string())?;
    Ok(provider.completion(MODEL))
}

/// Runs the agent on `input`, recording what it does in the latest run of `history`
/// and running its scripts with `script`, on `scene`. False if a request failed.
pub async fn run(
    model: Model<Messages>,
    input: String,
    mut history: State<History>,
    script: State<Script>,
    scene: State<Scene>,
) -> bool {
    let mut conversation = vec![Message::user(input)];
    loop {
        match request(&model, &conversation, history, script, scene).await {
            Ok((reply, results)) => {
                if !continued(&mut conversation, reply, results) {
                    return true;
                }
            }
            Err(error) => {
                history.write().failed(error);
                return false;
            }
        }
    }
}

/// Sends `conversation`, streaming the reply into `history` and running each call as
/// soon as it's complete. Returns the reply, and what the calls give back.
async fn request(
    model: &Model<Messages>,
    conversation: &[Message],
    mut history: State<History>,
    mut script: State<Script>,
    scene: State<Scene>,
) -> Result<(Option<Message>, Vec<ToolResult>), String> {
    let (last, earlier) = conversation
        .split_last()
        .expect("a run starts with its input");
    let request = CompletionRequest::new(last.clone())
        .messages(earlier.to_vec())
        .preamble(input::preamble())
        .tool(tools::definition())
        .max_tokens(MAX_TOKENS)
        .reasoning(REASONING);
    let mut stream = model.stream(request).map_err(|err| err.to_string())?;

    let mut reply = Reply::default();
    let mut results = Vec::new();
    while let Some(item) = stream.next().await {
        let Item::Event(event) = item.map_err(|err| err.to_string())? else {
            continue;
        };
        let Some(call) = reply.event(event, &mut history.write()) else {
            continue;
        };
        let (code, outcome) = tools::run(&call, &mut script.write());
        results.push(tools::answer(&call, &outcome, &scene.peek()));
        history.write().ran(code, outcome);
    }
    let response = stream.finish().await.map_err(|err| err.to_string())?;
    history.write().used(usage::request(MODEL, &response.usage));
    Ok((response.message(), results))
}

/// Adds `reply` and what its calls gave back to `conversation`, for the next request.
/// False if it made no calls, which means the run is done.
fn continued(
    conversation: &mut Vec<Message>,
    reply: Option<Message>,
    results: Vec<ToolResult>,
) -> bool {
    if results.is_empty() {
        return false;
    }
    conversation.extend(reply);
    conversation.push(Message::tool_results(results));
    true
}

#[cfg(test)]
mod tests {
    use rig::message::{ToolCall, ToolFunction, ToolName, ToolResultContent};
    use serde_json::json;

    use super::*;

    #[test]
    fn a_reply_without_calls_ends_the_run() {
        let mut conversation = vec![Message::user("input")];
        let reply = Some(Message::assistant("Nothing to change."));
        assert!(!continued(&mut conversation, reply, Vec::new()));
        assert_eq!(conversation.len(), 1);
    }

    #[test]
    fn calls_continue_the_run_with_their_results() {
        let name = ToolName::new(tools::NAME).unwrap();
        let call = ToolCall::from_wire("call", ToolFunction::new(name, json!({"code": "1"})));
        let result = call.result(vec![ToolResultContent::text("1")]);

        let mut conversation = vec![Message::user("input")];
        let reply = Message::assistant("Checking.");
        assert!(continued(
            &mut conversation,
            Some(reply.clone()),
            vec![result.clone()]
        ));
        assert_eq!(
            conversation,
            [
                Message::user("input"),
                reply,
                Message::tool_results(vec![result]),
            ]
        );
    }
}
