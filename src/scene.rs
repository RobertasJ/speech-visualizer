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

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Adds an empty rect at the end of `parent`'s children, or of the top level.
    AddRect { parent: Option<ElementId> },
    /// Adds text at the end of `parent`'s children, or of the top level.
    AddText {
        parent: Option<ElementId>,
        text: String,
    },
    /// Replaces the text of a text element.
    SetText { id: ElementId, text: String },
    /// Removes an element and everything in it.
    Remove { id: ElementId },
    /// Removes everything.
    Clear,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error("no element {0}")]
    Missing(ElementId),
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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Scene {
    entries: HashMap<ElementId, Entry>,
    /// Top-level elements, in order.
    roots: Vec<ElementId>,
    /// The last id handed out.
    last_id: u64,
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
        match command {
            Command::AddRect { parent } => {
                let rect = Element::Rect {
                    children: Vec::new(),
                };
                self.add(parent, rect).map(Some)
            }
            Command::AddText { parent, text } => self.add(parent, Element::Text { text }).map(Some),
            Command::SetText { id, text } => {
                let entry = self.entries.get_mut(&id).ok_or(CommandError::Missing(id))?;
                match &mut entry.element {
                    Element::Text { text: old } => *old = text,
                    Element::Rect { .. } => return Err(CommandError::ExpectedText(id)),
                }
                Ok(None)
            }
            Command::Remove { id } => self.remove(id).map(|()| None),
            Command::Clear => {
                self.entries.clear();
                self.roots.clear();
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
        let mut orphans = vec![entry.element];
        while let Some(element) = orphans.pop() {
            if let Element::Rect { children } = element {
                orphans.extend(
                    children
                        .iter()
                        .filter_map(|child| self.entries.remove(child))
                        .map(|entry| entry.element),
                );
            }
        }
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
    /// rect [in <id>]
    /// text [in <id>] <text>
    /// set <id> <text>
    /// remove <id>
    /// clear
    /// ```
    pub fn parse(line: &str) -> Result<Self, String> {
        let (word, rest) = split_word(line);
        let command = match word {
            "rect" => {
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
                let (id, text) = parse_id(rest)?;
                Command::SetText {
                    id,
                    text: text.to_owned(),
                }
            }
            "remove" => {
                let (id, rest) = parse_id(rest)?;
                expect_end(rest)?;
                Command::Remove { id }
            }
            "clear" => {
                expect_end(rest)?;
                Command::Clear
            }
            "" => return Err("type a command".into()),
            other => {
                return Err(format!(
                    "unknown command '{other}', expected rect, text, set, remove or clear"
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
    /// Queues `command`. Drop the [`Reply`] if you don't need the result.
    pub fn send(&self, command: Command) -> Reply {
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
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "for worker threads; only the tests wait so far")
    )]
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
fn parse_parent(text: &str) -> Result<(Option<ElementId>, &str), String> {
    match split_word(text) {
        ("in", rest) => parse_id(rest).map(|(id, rest)| (Some(id), rest)),
        _ => Ok((None, text)),
    }
}

fn parse_id(text: &str) -> Result<(ElementId, &str), String> {
    let (word, rest) = split_word(text);
    let id = word
        .parse()
        .map_err(|_| format!("expected an element id, got '{word}'"))?;
    Ok((ElementId(id), rest))
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
            let rect = sender.send(Command::AddRect { parent: None }).wait();
            let text = Command::AddText {
                parent: rect.clone().ok().flatten(),
                text: "hi".into(),
            };
            (rect, sender.send(text).wait())
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
            sender.send(Command::Clear).wait(),
            Err(CommandError::Closed)
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Command::parse("").is_err());
        assert!(Command::parse("jump").is_err());
        assert!(Command::parse("remove x").is_err());
        assert!(Command::parse("rect in").is_err());
        assert!(Command::parse("clear now").is_err());
    }
}
