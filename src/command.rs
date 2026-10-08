//! Typed or spoken commands, which change a [`Scene`] and its [`Names`].

use std::str::FromStr;

use crate::names::Names;
use crate::scene::{ElementId, Scene, SceneError};

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Id(ElementId),
    /// Matched ignoring case.
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Adds an empty rect at the end of `parent`'s children, or of the top level.
    AddRect { parent: Option<Target> },
    /// Adds text at the end of `parent`'s children, or of the top level.
    AddText {
        parent: Option<Target>,
        text: String,
    },
    /// Replaces the text of a text element.
    SetText { id: Target, text: String },
    /// Removes an element and everything in it.
    Remove { id: Target },
    /// Gives an element another name. A name that's in use already moves to it.
    Name { target: Target, name: String },
    /// Removes one name; the element keeps any others.
    Unname { name: String },
    /// Removes every name but the number words, leaving the elements as they are.
    UnnameAll,
    /// Removes everything, and every name but the number words.
    Clear,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Scene(#[from] SceneError),
    #[error("no element named '{0}'")]
    UnknownName(String),
}

impl Command {
    /// Applies the command to `scene`, with targets looked up in `names`. Returns the id
    /// of the element it added, if any. A command that fails changes nothing.
    pub fn apply(
        self,
        scene: &mut Scene,
        names: &mut Names,
    ) -> Result<Option<ElementId>, CommandError> {
        // Targets are resolved before anything changes, so an unknown name changes nothing.
        match self {
            Command::AddRect { parent } => {
                let parent = parent.map(|parent| resolve(names, &parent)).transpose()?;
                Ok(Some(scene.add_rect(parent)?))
            }
            Command::AddText { parent, text } => {
                let parent = parent.map(|parent| resolve(names, &parent)).transpose()?;
                Ok(Some(scene.add_text(parent, text)?))
            }
            Command::SetText { id, text } => {
                scene.set_text(resolve(names, &id)?, text)?;
                Ok(None)
            }
            Command::Remove { id } => {
                let removed = scene.remove(resolve(names, &id)?)?;
                names.forget(&removed);
                Ok(None)
            }
            Command::Name { target, name } => {
                let id = resolve(names, &target)?;
                if !scene.contains(id) {
                    return Err(SceneError::Missing(id).into());
                }
                names.insert(&name, id);
                Ok(None)
            }
            Command::Unname { name } => {
                if names.remove(&name) {
                    Ok(None)
                } else {
                    Err(CommandError::UnknownName(name.to_lowercase()))
                }
            }
            Command::UnnameAll => {
                names.reset();
                Ok(None)
            }
            Command::Clear => {
                scene.clear();
                names.reset();
                Ok(None)
            }
        }
    }
}

impl FromStr for Command {
    type Err = String;

    /// Parses a typed command:
    ///
    /// ```text
    /// rect [in <id>]        (or rectangle)
    /// text [in <id>] <text>
    /// set <id> <text>
    /// remove <id>
    /// name <id> <name>
    /// unname <name>
    /// unname all
    /// clear
    /// ```
    ///
    /// A name works wherever an `<id>` does, and ids up to ten have their word as a
    /// name, so "three" is element 3.
    fn from_str(line: &str) -> Result<Self, String> {
        let (word, rest) = split_word(line);
        let command = match word {
            "rect" | "rectangle" => {
                let (parent, rest) = parse_parent(rest)?;
                expect_end(rest)?;
                Command::AddRect { parent }
            }
            "text" => {
                let (parent, text) = parse_parent(rest)?;
                Command::AddText {
                    parent,
                    text: text.to_owned(),
                }
            }
            "set" => {
                let (id, text) = parse_target(rest)?;
                Command::SetText {
                    id,
                    text: text.to_owned(),
                }
            }
            "remove" => {
                let (id, rest) = parse_target(rest)?;
                expect_end(rest)?;
                Command::Remove { id }
            }
            "name" => {
                let (target, rest) = parse_target(rest)?;
                let (name, rest) = split_word(rest);
                expect_end(rest)?;
                let name = parse_name(name)?;
                // Reserved so commands stay unambiguous: "in" starts a parent, "number"
                // comes before an id or name, and "unname all" removes every name.
                if matches!(name.as_str(), "in" | "number" | "all") {
                    return Err(format!("'{name}' can't be a name"));
                }
                Command::Name { target, name }
            }
            "unname" => {
                let (name, rest) = split_word(rest);
                expect_end(rest)?;
                let name = parse_name(name)?;
                if name == "all" {
                    Command::UnnameAll
                } else {
                    Command::Unname { name }
                }
            }
            "clear" => {
                expect_end(rest)?;
                Command::Clear
            }
            "" => return Err("type a command".into()),
            other => {
                return Err(format!(
                    "unknown command '{other}', expected rect, text, set, remove, name, unname or clear"
                ));
            }
        };
        Ok(command)
    }
}

/// The id `target` stands for, whether or not that element exists.
fn resolve(names: &Names, target: &Target) -> Result<ElementId, CommandError> {
    match target {
        Target::Id(id) => Ok(*id),
        Target::Name(name) => names
            .get(name)
            .ok_or_else(|| CommandError::UnknownName(name.to_lowercase())),
    }
}

fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim();
    let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    (word, rest.trim_start())
}

fn parse_parent(text: &str) -> Result<(Option<Target>, &str), String> {
    match split_word(text) {
        ("in", rest) => parse_target(rest).map(|(target, rest)| (Some(target), rest)),
        _ => Ok((None, text)),
    }
}

/// An id in digits, or any other word as a name, optionally after "number": "3",
/// "number 3", "three" and "number three" all work (the last two through
/// the number words in [`Names`]).
fn parse_target(text: &str) -> Result<(Target, &str), String> {
    let (mut word, mut rest) = split_word(text);
    if word == "number" {
        (word, rest) = split_word(rest);
    }
    if word.is_empty() {
        return Err("expected an element id or name".into());
    }
    let target = match word.parse() {
        Ok(id) => Target::Id(ElementId(id)),
        Err(_) => Target::Name(word.to_lowercase()),
    };
    Ok((target, rest))
}

fn parse_name(word: &str) -> Result<String, String> {
    if word.is_empty() {
        return Err("expected a name".into());
    }
    if word.parse::<u64>().is_ok() {
        return Err("expected a name, got an id".into());
    }
    Ok(word.to_lowercase())
}

fn expect_end(rest: &str) -> Result<(), String> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(format!("unexpected '{rest}'"))
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
