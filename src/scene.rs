//! What the display window shows: a tree of elements that only changes through
//! [`Command`]s, and is rendered from scratch after every change (Elm's model and update).

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_channel::{mpsc, oneshot};
use futures_lite::StreamExt;

/// Identifies an element. Assigned by the [`Scene`] when the element is added, and
/// never reused, even after the element is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(pub u64);

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    /// A box around its children, laid out top to bottom.
    Rect {
        children: Vec<ElementId>,
    },
    Text {
        text: String,
    },
}

/// An element, by id or by a name given with [`Command::Name`].
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
    #[error("no element {0}")]
    Missing(ElementId),
    #[error("no element named '{0}'")]
    UnknownName(String),
    #[error("element {0} is not a rect")]
    ExpectedRect(ElementId),
    #[error("element {0} is not text")]
    ExpectedText(ElementId),
    /// Only from a [`SceneSender`]: nothing applies commands anymore.
    #[error("the scene is gone")]
    Closed,
}

/// What applying a command gives: the id of the element it added, if any.
pub type CommandResult = Result<Option<ElementId>, CommandError>;

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    entries: HashMap<ElementId, Entry>,
    /// Top-level elements, in order.
    roots: Vec<ElementId>,
    /// The last id handed out.
    last_id: u64,
    /// Lowercase names and the elements they're on. Starts out as [`number_names`], so
    /// a name can be on an element that doesn't exist yet.
    names: HashMap<String, ElementId>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            roots: Vec::new(),
            last_id: 0,
            names: number_names(),
        }
    }
}

/// Ids up to ten by their word, since speech often spells them out: "three" is element
/// 3. A scene starts with these, and gets them back on clear and unname all.
fn number_names() -> HashMap<String, ElementId> {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .iter()
        .zip(0..)
        .map(|(&word, id)| (word.to_owned(), ElementId(id)))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    element: Element,
    /// None = top level.
    parent: Option<ElementId>,
}

impl Scene {
    /// Applies `command`, returning the id of the element it added, if any. A command
    /// that fails changes nothing.
    pub fn apply(&mut self, command: Command) -> Result<Option<ElementId>, CommandError> {
        // Targets are resolved before anything changes, so an unknown name changes nothing.
        match command {
            Command::AddRect { parent } => {
                let parent = parent.map(|parent| self.resolve(&parent)).transpose()?;
                let rect = Element::Rect {
                    children: Vec::new(),
                };
                self.add(parent, rect).map(Some)
            }
            Command::AddText { parent, text } => {
                let parent = parent.map(|parent| self.resolve(&parent)).transpose()?;
                self.add(parent, Element::Text { text }).map(Some)
            }
            Command::SetText { id, text } => {
                let id = self.resolve(&id)?;
                let entry = self.entries.get_mut(&id).ok_or(CommandError::Missing(id))?;
                match &mut entry.element {
                    Element::Text { text: old } => *old = text,
                    Element::Rect { .. } => return Err(CommandError::ExpectedText(id)),
                }
                Ok(None)
            }
            Command::Remove { id } => {
                let id = self.resolve(&id)?;
                self.remove(id).map(|()| None)
            }
            Command::Name { target, name } => {
                let id = self.resolve(&target)?;
                if !self.entries.contains_key(&id) {
                    return Err(CommandError::Missing(id));
                }
                self.names.insert(name.to_lowercase(), id);
                Ok(None)
            }
            Command::Unname { name } => {
                let name = name.to_lowercase();
                match self.names.remove(&name) {
                    Some(_) => Ok(None),
                    None => Err(CommandError::UnknownName(name)),
                }
            }
            Command::UnnameAll => {
                self.names = number_names();
                Ok(None)
            }
            Command::Clear => {
                self.entries.clear();
                self.roots.clear();
                self.names = number_names();
                Ok(None)
            }
        }
    }

    /// The top-level elements, in order.
    pub fn roots(&self) -> &[ElementId] {
        &self.roots
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.entries.get(&id).map(|entry| &entry.element)
    }

    /// The names element `id` has, sorted.
    pub fn names_of(&self, id: ElementId) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .names
            .iter()
            .filter(|&(_, &named)| named == id)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        names
    }

    /// The id `target` stands for, whether or not that element exists.
    fn resolve(&self, target: &Target) -> Result<ElementId, CommandError> {
        match target {
            Target::Id(id) => Ok(*id),
            Target::Name(name) => {
                let name = name.to_lowercase();
                self.names
                    .get(&name)
                    .copied()
                    .ok_or(CommandError::UnknownName(name))
            }
        }
    }

    fn add(
        &mut self,
        parent: Option<ElementId>,
        element: Element,
    ) -> Result<ElementId, CommandError> {
        let id = ElementId(self.last_id + 1);
        self.children_mut(parent)?.push(id);
        self.last_id = id.0;
        self.entries.insert(id, Entry { element, parent });
        Ok(id)
    }

    fn remove(&mut self, id: ElementId) -> Result<(), CommandError> {
        let entry = self.entries.remove(&id).ok_or(CommandError::Missing(id))?;
        if let Ok(siblings) = self.children_mut(entry.parent) {
            siblings.retain(|&sibling| sibling != id);
        }
        let mut removed = vec![id];
        let mut orphans = vec![entry.element];
        while let Some(element) = orphans.pop() {
            if let Element::Rect { children } = element {
                for child in children {
                    if let Some(entry) = self.entries.remove(&child) {
                        removed.push(child);
                        orphans.push(entry.element);
                    }
                }
            }
        }
        self.names.retain(|_, named| !removed.contains(named));
        Ok(())
    }

    fn children_mut(
        &mut self,
        parent: Option<ElementId>,
    ) -> Result<&mut Vec<ElementId>, CommandError> {
        let Some(id) = parent else {
            return Ok(&mut self.roots);
        };
        let entry = self.entries.get_mut(&id).ok_or(CommandError::Missing(id))?;
        match &mut entry.element {
            Element::Rect { children } => Ok(children),
            Element::Text { .. } => Err(CommandError::ExpectedRect(id)),
        }
    }
}

impl Command {
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
    pub fn parse(line: &str) -> Result<Self, String> {
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

/// Creates a [`SceneSender`] for any thread, and the [`SceneReceiver`] that applies what
/// it sends.
pub fn channel() -> (SceneSender, SceneReceiver) {
    let (tx, rx) = mpsc::unbounded();
    (SceneSender { tx }, SceneReceiver { rx })
}

type Request = (Command, oneshot::Sender<CommandResult>);

/// Sends commands to the scene from any thread. They're applied in the order they're
/// sent, by [`SceneReceiver::apply_all`].
#[derive(Debug, Clone)]
pub struct SceneSender {
    tx: mpsc::UnboundedSender<Request>,
}

impl SceneSender {
    /// Queues `command`, without a way to find out how it went.
    pub fn send(&self, command: Command) {
        drop(self.request(command));
    }

    /// Queues `command`; the [`Reply`] gives its result once it has been applied.
    pub fn request(&self, command: Command) -> Reply {
        let (reply_tx, reply_rx) = oneshot::channel();
        // If nothing applies commands anymore, `reply_tx` is dropped here and the
        // reply comes back as Closed.
        let _ = self.tx.unbounded_send((command, reply_tx));
        Reply(reply_rx)
    }
}

impl PartialEq for SceneSender {
    fn eq(&self, other: &Self) -> bool {
        self.tx.same_receiver(&other.tx)
    }
}

/// The result of a sent command, once it has been applied. Await it, or [`wait`] for
/// it outside the thread that applies commands (which would deadlock).
///
/// [`wait`]: Reply::wait
pub struct Reply(oneshot::Receiver<CommandResult>);

impl Reply {
    /// Blocks until the command has been applied.
    pub fn wait(self) -> CommandResult {
        futures_lite::future::block_on(self)
    }
}

impl Future for Reply {
    type Output = CommandResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<CommandResult> {
        Pin::new(&mut self.0)
            .poll(cx)
            .map(|reply| reply.unwrap_or(Err(CommandError::Closed)))
    }
}

/// The receiving end of [`channel`].
pub struct SceneReceiver {
    rx: mpsc::UnboundedReceiver<Request>,
}

impl SceneReceiver {
    /// Applies commands in the order they come in, until every sender is gone.
    pub async fn apply_all(mut self, mut apply: impl FnMut(Command) -> CommandResult) {
        while let Some((command, reply)) = self.rx.next().await {
            let _ = reply.send(apply(command));
        }
    }
}

/// The first word of `text` and the rest, both without surrounding whitespace.
fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim();
    let (word, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    (word, rest.trim_start())
}

/// An optional leading `in <id>`.
fn parse_parent(text: &str) -> Result<(Option<Target>, &str), String> {
    match split_word(text) {
        ("in", rest) => parse_target(rest).map(|(target, rest)| (Some(target), rest)),
        _ => Ok((None, text)),
    }
}

/// An id in digits, or any other word as a name, optionally after "number": "3",
/// "number 3", "three" and "number three" all work (the last two through
/// [`number_names`]).
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

/// One word that isn't an id, lowercased.
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

    fn run(scene: &mut Scene, line: &str) -> Result<Option<ElementId>, CommandError> {
        scene.apply(Command::parse(line).expect("parses"))
    }

    #[test]
    fn adds_nested_elements_with_fresh_ids() {
        let mut scene = Scene::default();
        let rect = run(&mut scene, "rect").unwrap().unwrap();
        let text = run(&mut scene, "text in 1 Hello there").unwrap().unwrap();
        assert_eq!((rect, text), (ElementId(1), ElementId(2)));
        assert_eq!(scene.roots(), &[rect]);
        assert_eq!(
            scene.get(rect),
            Some(&Element::Rect {
                children: vec![text]
            })
        );
        assert_eq!(
            scene.get(text),
            Some(&Element::Text {
                text: "Hello there".into()
            })
        );
    }

    #[test]
    fn remove_takes_children_and_ids_are_not_reused() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        run(&mut scene, "rect in 1").unwrap();
        run(&mut scene, "text in 2 deep").unwrap();
        run(&mut scene, "remove 1").unwrap();
        assert!(scene.roots().is_empty());
        assert_eq!(scene.get(ElementId(3)), None);
        assert_eq!(run(&mut scene, "text again"), Ok(Some(ElementId(4))));
    }

    #[test]
    fn failed_commands_change_nothing() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        let before = scene.clone();
        assert_eq!(
            run(&mut scene, "rect in 1"),
            Err(CommandError::ExpectedRect(ElementId(1)))
        );
        assert_eq!(
            run(&mut scene, "set 9 x"),
            Err(CommandError::Missing(ElementId(9)))
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn sender_works_from_another_thread() {
        let (sender, receiver) = channel();
        let worker = std::thread::spawn(move || {
            let rect = sender.request(Command::AddRect { parent: None }).wait();
            let text = Command::AddText {
                parent: rect.clone().ok().flatten().map(Target::Id),
                text: "hi".into(),
            };
            (rect, sender.request(text).wait())
        });

        let mut scene = Scene::default();
        futures_lite::future::block_on(receiver.apply_all(|command| scene.apply(command)));
        assert_eq!(
            worker.join().unwrap(),
            (Ok(Some(ElementId(1))), Ok(Some(ElementId(2))))
        );
        assert_eq!(
            scene.get(ElementId(2)),
            Some(&Element::Text { text: "hi".into() })
        );
    }

    #[test]
    fn sending_without_a_receiver_fails() {
        let (sender, receiver) = channel();
        drop(receiver);
        assert_eq!(
            sender.request(Command::Clear).wait(),
            Err(CommandError::Closed)
        );
    }

    #[test]
    fn rectangle_is_rect() {
        assert_eq!(
            Command::parse("rectangle in 2"),
            Ok(Command::AddRect {
                parent: Some(Target::Id(ElementId(2)))
            })
        );
    }

    #[test]
    fn ids_up_to_ten_are_named_by_their_word() {
        let mut scene = Scene::default();
        assert_eq!(
            run(&mut scene, "text in three hi"),
            Err(CommandError::Missing(ElementId(3)))
        );
        for _ in 0..11 {
            run(&mut scene, "rect").unwrap();
        }
        assert_eq!(scene.names_of(ElementId(1)), ["one"]);
        assert!(scene.names_of(ElementId(11)).is_empty());
        assert_eq!(run(&mut scene, "text in three hi"), Ok(Some(ElementId(12))));
        assert_eq!(run(&mut scene, "remove ten"), Ok(None));
        assert_eq!(
            run(&mut scene, "remove eleven"),
            Err(CommandError::UnknownName("eleven".into()))
        );
    }

    #[test]
    fn ids_and_names_can_follow_number() {
        assert_eq!(
            Command::parse("text in number two hi"),
            Ok(Command::AddText {
                parent: Some(Target::Name("two".into())),
                text: "hi".into()
            })
        );
        assert_eq!(
            Command::parse("remove number 4"),
            Ok(Command::Remove {
                id: Target::Id(ElementId(4))
            })
        );
        assert!(Command::parse("remove number").is_err());
    }

    #[test]
    fn names_work_wherever_ids_do() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        assert_eq!(run(&mut scene, "name 1 test"), Ok(None));
        let text = run(&mut scene, "text in test hello").unwrap().unwrap();
        let inner = run(&mut scene, "rect in test").unwrap().unwrap();
        assert_eq!(
            scene.get(ElementId(1)),
            Some(&Element::Rect {
                children: vec![text, inner]
            })
        );

        run(&mut scene, "name 2 greeting").unwrap();
        run(&mut scene, "set greeting bye").unwrap();
        assert_eq!(scene.get(text), Some(&Element::Text { text: "bye".into() }));

        run(&mut scene, "remove test").unwrap();
        assert!(scene.roots().is_empty());
    }

    #[test]
    fn names_ignore_case() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        run(&mut scene, "name 1 Test").unwrap();
        assert_eq!(scene.names_of(ElementId(1)), ["one", "test"]);
        assert_eq!(run(&mut scene, "text in TEST hi"), Ok(Some(ElementId(2))));
        // Not lowercased by parsing this time.
        let remove = Command::Remove {
            id: Target::Name("tEsT".into()),
        };
        assert_eq!(scene.apply(remove), Ok(None));
        assert!(scene.roots().is_empty());
    }

    #[test]
    fn an_element_can_have_several_names() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        run(&mut scene, "name 1 zed").unwrap();
        run(&mut scene, "name zed alpha").unwrap();
        assert_eq!(scene.names_of(ElementId(1)), ["alpha", "one", "zed"]);
    }

    #[test]
    fn naming_again_moves_the_name() {
        let mut scene = Scene::default();
        run(&mut scene, "text a").unwrap();
        run(&mut scene, "text b").unwrap();
        run(&mut scene, "name 1 first").unwrap();
        run(&mut scene, "name 1 test").unwrap();
        run(&mut scene, "name 2 test").unwrap();
        assert_eq!(scene.names_of(ElementId(1)), ["first", "one"]);
        assert_eq!(scene.names_of(ElementId(2)), ["test", "two"]);
        run(&mut scene, "set test changed").unwrap();
        assert_eq!(
            scene.get(ElementId(2)),
            Some(&Element::Text {
                text: "changed".into()
            })
        );
    }

    #[test]
    fn remove_drops_the_names_of_everything_removed() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        run(&mut scene, "rect in 1").unwrap();
        run(&mut scene, "text in 2 deep").unwrap();
        run(&mut scene, "text kept").unwrap();
        for line in ["name 1 outer", "name 2 inner", "name 3 deep", "name 4 kept"] {
            run(&mut scene, line).unwrap();
        }
        run(&mut scene, "remove outer").unwrap();
        for name in ["outer", "inner", "deep", "three"] {
            assert_eq!(
                run(&mut scene, &format!("set {name} x")),
                Err(CommandError::UnknownName(name.into()))
            );
        }
        assert!(scene.names_of(ElementId(3)).is_empty());
        assert_eq!(scene.names_of(ElementId(4)), ["four", "kept"]);
    }

    #[test]
    fn clear_drops_names() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        run(&mut scene, "name 1 greeting").unwrap();
        run(&mut scene, "clear").unwrap();
        run(&mut scene, "text again").unwrap();
        assert_eq!(
            run(&mut scene, "set greeting x"),
            Err(CommandError::UnknownName("greeting".into()))
        );
        // The number words come back.
        assert_eq!(run(&mut scene, "set two x"), Ok(None));
    }

    #[test]
    fn unknown_names_and_missing_elements_change_nothing() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        run(&mut scene, "name 1 box").unwrap();
        let before = scene.clone();
        assert_eq!(
            run(&mut scene, "remove x"),
            Err(CommandError::UnknownName("x".into()))
        );
        assert_eq!(
            run(&mut scene, "text in nowhere hi"),
            Err(CommandError::UnknownName("nowhere".into()))
        );
        assert_eq!(
            run(&mut scene, "name nowhere lid"),
            Err(CommandError::UnknownName("nowhere".into()))
        );
        // Would move "box" if it went through.
        assert_eq!(
            run(&mut scene, "name 9 box"),
            Err(CommandError::Missing(ElementId(9)))
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn name_parses_one_lowercase_word() {
        assert_eq!(
            Command::parse("name 3 Box"),
            Ok(Command::Name {
                target: Target::Id(ElementId(3)),
                name: "box".into()
            })
        );
        assert_eq!(
            Command::parse("name box lid"),
            Ok(Command::Name {
                target: Target::Name("box".into()),
                name: "lid".into()
            })
        );
    }

    #[test]
    fn unname_removes_one_name() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        run(&mut scene, "name 1 a").unwrap();
        run(&mut scene, "name 1 b").unwrap();
        assert_eq!(run(&mut scene, "unname a"), Ok(None));
        assert_eq!(scene.names_of(ElementId(1)), ["b", "one"]);
        assert_eq!(run(&mut scene, "set b bye"), Ok(None));
        assert_eq!(
            run(&mut scene, "set a x"),
            Err(CommandError::UnknownName("a".into()))
        );
        assert_eq!(
            scene.get(ElementId(1)),
            Some(&Element::Text { text: "bye".into() })
        );
    }

    #[test]
    fn unname_of_an_unknown_name_changes_nothing() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        run(&mut scene, "name 1 a").unwrap();
        let before = scene.clone();
        assert_eq!(
            run(&mut scene, "unname b"),
            Err(CommandError::UnknownName("b".into()))
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn unname_all_leaves_only_the_number_words_and_keeps_the_elements() {
        let mut scene = Scene::default();
        run(&mut scene, "rect").unwrap();
        run(&mut scene, "text in 1 hi").unwrap();
        for line in ["name 1 box", "name 2 greeting", "name 2 hello"] {
            run(&mut scene, line).unwrap();
        }
        let before = scene.clone();
        assert_eq!(run(&mut scene, "unname all"), Ok(None));
        assert_eq!(scene.roots(), before.roots());
        for id in [ElementId(1), ElementId(2)] {
            assert_eq!(scene.get(id), before.get(id));
        }
        assert_eq!(scene.names_of(ElementId(1)), ["one"]);
        assert_eq!(scene.names_of(ElementId(2)), ["two"]);
        assert_eq!(
            run(&mut scene, "remove box"),
            Err(CommandError::UnknownName("box".into()))
        );
    }

    #[test]
    fn unname_all_without_names_succeeds() {
        let mut scene = Scene::default();
        assert_eq!(run(&mut scene, "unname all"), Ok(None));
        assert_eq!(scene, Scene::default());
    }

    #[test]
    fn unname_ignores_case() {
        let mut scene = Scene::default();
        run(&mut scene, "text hi").unwrap();
        run(&mut scene, "name 1 test").unwrap();
        assert_eq!(run(&mut scene, "unname Test"), Ok(None));
        assert_eq!(scene.names_of(ElementId(1)), ["one"]);
        assert_eq!(Command::parse("unname ALL"), Ok(Command::UnnameAll));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Command::parse("").is_err());
        assert!(Command::parse("jump").is_err());
        assert!(Command::parse("rect in").is_err());
        assert!(Command::parse("clear now").is_err());
        assert!(Command::parse("name 1").is_err());
        assert!(Command::parse("name 1 in").is_err());
        assert!(Command::parse("name 1 number").is_err());
        assert!(Command::parse("name 1 all").is_err());
        assert!(Command::parse("name 1 a b").is_err());
        assert!(Command::parse("unname").is_err());
        assert!(Command::parse("unname 3").is_err());
        assert!(Command::parse("unname a b").is_err());
    }
}
