//! What a run is given: the system prompt, and a message with the transcript, the
//! agent's earlier runs, the scene and the script variables.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::scene::{FUNCTIONS, Scene};

/// The system prompt, with the script functions filled in.
pub fn preamble() -> String {
    let functions: Vec<String> = FUNCTIONS
        .iter()
        .map(|function| format!("- `{}`: {}", function.signature, function.description))
        .collect();
    include_str!("preamble.md").replace("{functions}", &functions.join("\n"))
}

/// The message a run starts with.
#[derive(Serialize)]
pub struct Input<'a> {
    /// Everything said so far.
    pub transcript: String,
    /// What was said since the agent last finished a run.
    pub new_text: String,
    /// Its earlier runs, as text.
    pub responses: Vec<String>,
    pub scene: &'a Scene,
    /// Each script variable and its value, as text.
    pub variables: BTreeMap<String, String>,
}

impl Input<'_> {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("an input serializes")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn preamble_lists_every_function() {
        let preamble = preamble();
        assert!(!preamble.contains("{functions}"));
        for function in FUNCTIONS {
            assert!(
                preamble.contains(function.signature),
                "{}",
                function.signature
            );
        }
    }

    #[test]
    fn input_has_every_part() {
        let scene = Scene::default();
        let input = Input {
            transcript: "add a box".into(),
            new_text: "add a box".into(),
            responses: vec!["A box.\n".into()],
            scene: &scene,
            variables: [("b".to_owned(), "#1 rect".to_owned())].into(),
        };
        let input: Value = serde_json::from_str(&input.to_json()).unwrap();
        assert_eq!(
            input,
            json!({
                "transcript": "add a box",
                "new_text": "add a box",
                "responses": ["A box.\n"],
                "scene": {"next_id": 1, "elements": []},
                "variables": {"b": "#1 rect"},
            })
        );
    }
}
