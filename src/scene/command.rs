//! Typed or spoken commands, which change a [`Scene`] and its [`Names`].

use std::str::FromStr;

use super::names::Names;
use super::{ElementId, Scene, SceneError};

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
mod tests {
    use super::*;
    use crate::scene::Element;

    #[derive(Debug, Clone, PartialEq, Default)]
    struct World {
        scene: Scene,
        names: Names,
    }

    fn run(world: &mut World, line: &str) -> Result<Option<ElementId>, CommandError> {
        let command = line.parse::<Command>().expect("parses");
        command.apply(&mut world.scene, &mut world.names)
    }

    #[test]
    fn rectangle_is_rect() {
        assert_eq!(
            "rectangle in 2".parse::<Command>(),
            Ok(Command::AddRect {
                parent: Some(Target::Id(ElementId(2)))
            })
        );
    }

    #[test]
    fn ids_up_to_ten_are_named_by_their_word() {
        let mut world = World::default();
        assert_eq!(
            run(&mut world, "text in three hi"),
            Err(CommandError::Scene(SceneError::Missing(ElementId(3))))
        );
        for _ in 0..11 {
            run(&mut world, "rect").unwrap();
        }
        assert_eq!(world.names.of(ElementId(1)), ["one"]);
        assert!(world.names.of(ElementId(11)).is_empty());
        assert_eq!(run(&mut world, "text in three hi"), Ok(Some(ElementId(12))));
        assert_eq!(run(&mut world, "remove ten"), Ok(None));
        assert_eq!(
            run(&mut world, "remove eleven"),
            Err(CommandError::UnknownName("eleven".into()))
        );
    }

    #[test]
    fn ids_and_names_can_follow_number() {
        assert_eq!(
            "text in number two hi".parse::<Command>(),
            Ok(Command::AddText {
                parent: Some(Target::Name("two".into())),
                text: "hi".into()
            })
        );
        assert_eq!(
            "remove number 4".parse::<Command>(),
            Ok(Command::Remove {
                id: Target::Id(ElementId(4))
            })
        );
        assert!("remove number".parse::<Command>().is_err());
    }

    #[test]
    fn names_work_wherever_ids_do() {
        let mut world = World::default();
        run(&mut world, "rect").unwrap();
        assert_eq!(run(&mut world, "name 1 test"), Ok(None));
        let text = run(&mut world, "text in test hello").unwrap().unwrap();
        let inner = run(&mut world, "rect in test").unwrap().unwrap();
        assert_eq!(
            world.scene.get(ElementId(1)),
            Some(&Element::Rect {
                children: vec![text, inner]
            })
        );

        run(&mut world, "name 2 greeting").unwrap();
        run(&mut world, "set greeting bye").unwrap();
        assert_eq!(
            world.scene.get(text),
            Some(&Element::Text { text: "bye".into() })
        );

        run(&mut world, "remove test").unwrap();
        assert!(world.scene.roots().is_empty());
    }

    #[test]
    fn names_ignore_case() {
        let mut world = World::default();
        run(&mut world, "rect").unwrap();
        run(&mut world, "name 1 Test").unwrap();
        assert_eq!(world.names.of(ElementId(1)), ["one", "test"]);
        assert_eq!(run(&mut world, "text in TEST hi"), Ok(Some(ElementId(2))));
        // Not lowercased by parsing this time.
        let remove = Command::Remove {
            id: Target::Name("tEsT".into()),
        };
        assert_eq!(remove.apply(&mut world.scene, &mut world.names), Ok(None));
        assert!(world.scene.roots().is_empty());
    }

    #[test]
    fn an_element_can_have_several_names() {
        let mut world = World::default();
        run(&mut world, "text hi").unwrap();
        run(&mut world, "name 1 zed").unwrap();
        run(&mut world, "name zed alpha").unwrap();
        assert_eq!(world.names.of(ElementId(1)), ["alpha", "one", "zed"]);
    }

    #[test]
    fn naming_again_moves_the_name() {
        let mut world = World::default();
        run(&mut world, "text a").unwrap();
        run(&mut world, "text b").unwrap();
        run(&mut world, "name 1 first").unwrap();
        run(&mut world, "name 1 test").unwrap();
        run(&mut world, "name 2 test").unwrap();
        assert_eq!(world.names.of(ElementId(1)), ["first", "one"]);
        assert_eq!(world.names.of(ElementId(2)), ["test", "two"]);
        run(&mut world, "set test changed").unwrap();
        assert_eq!(
            world.scene.get(ElementId(2)),
            Some(&Element::Text {
                text: "changed".into()
            })
        );
    }

    #[test]
    fn remove_drops_the_names_of_everything_removed() {
        let mut world = World::default();
        run(&mut world, "rect").unwrap();
        run(&mut world, "rect in 1").unwrap();
        run(&mut world, "text in 2 deep").unwrap();
        run(&mut world, "text kept").unwrap();
        for line in ["name 1 outer", "name 2 inner", "name 3 deep", "name 4 kept"] {
            run(&mut world, line).unwrap();
        }
        run(&mut world, "remove outer").unwrap();
        for name in ["outer", "inner", "deep", "three"] {
            assert_eq!(
                run(&mut world, &format!("set {name} x")),
                Err(CommandError::UnknownName(name.into()))
            );
        }
        assert!(world.names.of(ElementId(3)).is_empty());
        assert_eq!(world.names.of(ElementId(4)), ["four", "kept"]);
    }

    #[test]
    fn clear_drops_names() {
        let mut world = World::default();
        run(&mut world, "text hi").unwrap();
        run(&mut world, "name 1 greeting").unwrap();
        run(&mut world, "clear").unwrap();
        run(&mut world, "text again").unwrap();
        assert_eq!(
            run(&mut world, "set greeting x"),
            Err(CommandError::UnknownName("greeting".into()))
        );
        // The number words come back.
        assert_eq!(run(&mut world, "set two x"), Ok(None));
    }

    #[test]
    fn unknown_names_and_missing_elements_change_nothing() {
        let mut world = World::default();
        run(&mut world, "rect").unwrap();
        run(&mut world, "name 1 box").unwrap();
        let before = world.clone();
        assert_eq!(
            run(&mut world, "remove x"),
            Err(CommandError::UnknownName("x".into()))
        );
        assert_eq!(
            run(&mut world, "text in nowhere hi"),
            Err(CommandError::UnknownName("nowhere".into()))
        );
        assert_eq!(
            run(&mut world, "name nowhere lid"),
            Err(CommandError::UnknownName("nowhere".into()))
        );
        // Would move "box" if it went through.
        assert_eq!(
            run(&mut world, "name 9 box"),
            Err(CommandError::Scene(SceneError::Missing(ElementId(9))))
        );
        assert_eq!(world, before);
    }

    #[test]
    fn name_parses_one_lowercase_word() {
        assert_eq!(
            "name 3 Box".parse::<Command>(),
            Ok(Command::Name {
                target: Target::Id(ElementId(3)),
                name: "box".into()
            })
        );
        assert_eq!(
            "name box lid".parse::<Command>(),
            Ok(Command::Name {
                target: Target::Name("box".into()),
                name: "lid".into()
            })
        );
    }

    #[test]
    fn unname_removes_one_name() {
        let mut world = World::default();
        run(&mut world, "text hi").unwrap();
        run(&mut world, "name 1 a").unwrap();
        run(&mut world, "name 1 b").unwrap();
        assert_eq!(run(&mut world, "unname a"), Ok(None));
        assert_eq!(world.names.of(ElementId(1)), ["b", "one"]);
        assert_eq!(run(&mut world, "set b bye"), Ok(None));
        assert_eq!(
            run(&mut world, "set a x"),
            Err(CommandError::UnknownName("a".into()))
        );
        assert_eq!(
            world.scene.get(ElementId(1)),
            Some(&Element::Text { text: "bye".into() })
        );
    }

    #[test]
    fn unname_of_an_unknown_name_changes_nothing() {
        let mut world = World::default();
        run(&mut world, "text hi").unwrap();
        run(&mut world, "name 1 a").unwrap();
        let before = world.clone();
        assert_eq!(
            run(&mut world, "unname b"),
            Err(CommandError::UnknownName("b".into()))
        );
        assert_eq!(world, before);
    }

    #[test]
    fn unname_all_leaves_only_the_number_words_and_keeps_the_elements() {
        let mut world = World::default();
        run(&mut world, "rect").unwrap();
        run(&mut world, "text in 1 hi").unwrap();
        for line in ["name 1 box", "name 2 greeting", "name 2 hello"] {
            run(&mut world, line).unwrap();
        }
        let before = world.clone();
        assert_eq!(run(&mut world, "unname all"), Ok(None));
        assert_eq!(world.scene.roots(), before.scene.roots());
        for id in [ElementId(1), ElementId(2)] {
            assert_eq!(world.scene.get(id), before.scene.get(id));
        }
        assert_eq!(world.names.of(ElementId(1)), ["one"]);
        assert_eq!(world.names.of(ElementId(2)), ["two"]);
        assert_eq!(
            run(&mut world, "remove box"),
            Err(CommandError::UnknownName("box".into()))
        );
    }

    #[test]
    fn unname_all_without_names_succeeds() {
        let mut world = World::default();
        assert_eq!(run(&mut world, "unname all"), Ok(None));
        assert_eq!(world, World::default());
    }

    #[test]
    fn unname_ignores_case() {
        let mut world = World::default();
        run(&mut world, "text hi").unwrap();
        run(&mut world, "name 1 test").unwrap();
        assert_eq!(run(&mut world, "unname Test"), Ok(None));
        assert_eq!(world.names.of(ElementId(1)), ["one"]);
        assert_eq!("unname ALL".parse::<Command>(), Ok(Command::UnnameAll));
    }

    #[test]
    fn rejects_bad_input() {
        assert!("".parse::<Command>().is_err());
        assert!("jump".parse::<Command>().is_err());
        assert!("rect in".parse::<Command>().is_err());
        assert!("clear now".parse::<Command>().is_err());
        assert!("name 1".parse::<Command>().is_err());
        assert!("name 1 in".parse::<Command>().is_err());
        assert!("name 1 number".parse::<Command>().is_err());
        assert!("name 1 all".parse::<Command>().is_err());
        assert!("name 1 a b".parse::<Command>().is_err());
        assert!("unname".parse::<Command>().is_err());
        assert!("unname 3".parse::<Command>().is_err());
        assert!("unname a b".parse::<Command>().is_err());
    }
}
