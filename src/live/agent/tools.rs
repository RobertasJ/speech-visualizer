//! The `run_script` tool, which the agent changes the scene with.

use rig::completion::ToolDefinition;
use rig::message::{ToolCall, ToolName, ToolResult, ToolResultContent};
use rig::streaming::parse_partial_arguments;
use serde_json::{Value, json};

use crate::scene::{Outcome, Scene, Script};

pub const NAME: &str = "run_script";

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(NAME).expect("the name isn't empty"),
        description: "Runs Rhai code on the display, at once. Returns what it printed and \
            its result, `ok`, or the error, in which case the script was undone. Then the \
            scene as it is now."
            .to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "code": {"type": "string", "description": "The Rhai code to run."},
            },
            "required": ["code"],
        }),
    }
}

/// The code a call carries, or why it can't be run.
pub fn code(call: &ToolCall) -> Result<&str, String> {
    let name = call.function.name.as_str();
    if name != NAME {
        return Err(format!("there's no tool called {name}, only {NAME}"));
    }
    call.function
        .arguments
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{NAME} needs `code`, a string"))
}

/// The code so far, from a call's arguments as they stream in.
pub fn partial_code(arguments: &str) -> Option<String> {
    let arguments = parse_partial_arguments(arguments);
    arguments.get("code")?.as_str().map(str::to_owned)
}

/// Runs what `call` asks for with `script`. Returns the code it ran and how it went. A
/// call that can't be run fails without running anything, and says why.
pub fn run(call: &ToolCall, script: &mut Script) -> (String, Outcome) {
    match code(call) {
        Ok(code) => (code.to_owned(), script.run(code)),
        Err(error) => {
            let outcome = Outcome {
                output: Vec::new(),
                error: Some(error),
            };
            (String::new(), outcome)
        }
    }
}

/// What goes back to the agent for `call`, which ran with `outcome`: how it went, then
/// `scene`. A failed script comes back as an error.
pub fn answer(call: &ToolCall, outcome: &Outcome, scene: &Scene) -> ToolResult {
    let content = vec![ToolResultContent::text(result(outcome, scene))];
    if outcome.error.is_some() {
        call.error_result(content)
    } else {
        call.result(content)
    }
}

/// The text of an answer: how the script went, then the scene.
fn result(outcome: &Outcome, scene: &Scene) -> String {
    // The whole scene on every call costs tokens; drop it here if that gets too much.
    let scene = serde_json::to_string_pretty(scene).expect("a scene serializes");
    format!("{}\n\nscene:\n{scene}", outcome.lines().join("\n"))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use rig::message::ToolFunction;

    use super::*;

    fn call(name: &str, arguments: Value) -> ToolCall {
        let name = ToolName::new(name).unwrap();
        ToolCall::from_wire("call-1", ToolFunction::new(name, arguments))
    }

    #[test]
    fn code_comes_from_a_run_script_call() {
        let script = call(NAME, json!({"code": "rect()"}));
        assert_eq!(code(&script), Ok("rect()"));
        assert!(
            code(&call(NAME, json!({})))
                .unwrap_err()
                .contains("needs `code`")
        );
        assert!(code(&call(NAME, json!({"code": 1}))).is_err());
        let other = call("draw", json!({"code": "rect()"}));
        assert!(code(&other).unwrap_err().contains("no tool called draw"));
    }

    #[test]
    fn partial_code_reads_arguments_as_they_stream() {
        assert_eq!(partial_code(""), None);
        assert_eq!(partial_code(r#"{"co"#), None);
        assert_eq!(
            partial_code(r#"{"code": "let b = re"#).as_deref(),
            Some("let b = re")
        );
        assert_eq!(
            partial_code(r#"{"code": "a;\nb"}"#).as_deref(),
            Some("a;\nb")
        );
    }

    #[test]
    fn run_runs_the_code_on_the_script() {
        let scene = Rc::new(RefCell::new(Scene::default()));
        let mut script = Script::new(scene.clone());
        let (code, outcome) = run(&call(NAME, json!({"code": "rect()"})), &mut script);
        assert_eq!(code, "rect()");
        assert_eq!(outcome.output, ["#1 rect"]);
        assert_eq!(scene.borrow().roots().len(), 1);

        // A call that can't be run doesn't touch the script.
        let (code, outcome) = run(&call("draw", json!({"code": "rect()"})), &mut script);
        assert_eq!(code, "");
        assert!(outcome.error.unwrap().contains("no tool called draw"));
        assert_eq!(scene.borrow().roots().len(), 1);
    }

    #[test]
    fn answer_marks_failed_scripts_as_errors() {
        let scene = Scene::default();
        let call = call(NAME, json!({"code": "el(9)"}));
        let failed = Outcome {
            output: Vec::new(),
            error: Some("no element 9".into()),
        };
        let answer = answer(&call, &failed, &scene);
        assert!(answer.is_error);
        assert_eq!(answer.name.as_str(), NAME);
        assert!(!super::answer(&call, &Outcome::default(), &scene).is_error);
    }

    #[test]
    fn result_is_the_outcome_then_the_scene() {
        let mut scene = Scene::default();
        scene.add_rect(None).unwrap();
        let outcome = Outcome {
            output: vec!["hi".into()],
            error: None,
        };
        let result = result(&outcome, &scene);
        assert!(result.starts_with("hi\n\nscene:\n{"), "{result}");
        assert!(result.contains(r#""next_id": 2"#), "{result}");
        assert!(super::result(&Outcome::default(), &scene).starts_with("ok\n"));
    }
}
