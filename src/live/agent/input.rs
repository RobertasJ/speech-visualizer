//! What a run is given: the system prompt, and a message with the transcript, the
//! agent's earlier runs, the scene and the script variables.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::Serialize;

use crate::scene::{FUNCTIONS, GroupDocs, Kind, PropertyDocs, Scene, Value, property_docs};

/// The system prompt, with the script functions and the properties filled in.
pub fn preamble() -> String {
    let functions: Vec<String> = FUNCTIONS
        .iter()
        .map(|function| format!("- `{}`: {}", function.signature, function.description))
        .collect();
    include_str!("preamble.md")
        .replace("{functions}", &functions.join("\n"))
        .replace("{properties}", &properties())
}

/// Each property group: the kinds of element that have it, what it's for, and each of
/// its properties and shorthands.
fn properties() -> String {
    let groups: Vec<String> = property_docs()
        .iter()
        .map(|(group, kinds)| group_section(group, kinds))
        .collect();
    groups.join("\n")
}

fn group_section(group: &GroupDocs, kinds: &[Kind]) -> String {
    let kinds: Vec<&str> = kinds.iter().map(|kind| kind.name()).collect();
    let mut section = if kinds == [group.name] {
        format!("### {}\n\n", group.name)
    } else {
        format!("### {} ({})\n\n", group.name, kinds.join(", "))
    };
    writeln!(section, "{}\n", group.description).expect("writing to a String");
    for property in &group.properties {
        writeln!(section, "{}", property_line(property)).expect("writing to a String");
    }
    for (name, description) in &group.shorthands {
        writeln!(section, "- `{name}()`: {description}").expect("writing to a String");
    }
    section
}

fn property_line(property: &PropertyDocs) -> String {
    let default = match &property.default {
        Value::One(value) => format!("`{value}`"),
        Value::List(items) if items.is_empty() => "none".to_owned(),
        Value::List(items) => format!("`{}`", items.join("`, `")),
    };
    let list = if property.list { ", a list" } else { "" };
    format!(
        "- `{}`{list}: {} `{}`. Default {default}.",
        property.name, property.description, property.syntax
    )
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
    use serde_json::json;

    use super::*;

    #[test]
    fn preamble_lists_every_function_and_property() {
        let preamble = preamble();
        assert!(!preamble.contains("{functions}"));
        assert!(!preamble.contains("{properties}"));
        for function in FUNCTIONS {
            assert!(
                preamble.contains(function.signature),
                "{}",
                function.signature
            );
        }
        for (group, _) in property_docs() {
            for property in &group.properties {
                assert!(
                    preamble.contains(&format!("- `{}`", property.name)),
                    "{}",
                    property.name
                );
            }
        }
    }

    #[test]
    fn properties_say_their_kinds_syntax_and_default() {
        let properties = properties();
        assert!(properties.contains("### layout (rect, paragraph)\n"));
        assert!(properties.contains("### span\n"));
        assert!(properties.contains(
            "- `padding`: Space between its edges and what's inside it. `N | (vertical, \
             horizontal) | (top, right, bottom, left)`. Default `0`."
        ));
        assert!(properties.contains("- `border`, a list: Adds a border"));
        assert!(properties.contains("Default none."));
        assert!(properties.contains("- `expanded()`: Fills its parent both ways"));
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
        let input: serde_json::Value = serde_json::from_str(&input.to_json()).unwrap();
        assert_eq!(
            input,
            json!({
                "transcript": "add a box",
                "new_text": "add a box",
                "responses": ["A box.\n"],
                "scene": {
                    "next_id": 1,
                    "root": {"id": 0, "rect": [], "width": "fill", "height": "fill"},
                },
                "variables": {"b": "#1 rect"},
            })
        );
    }
}
