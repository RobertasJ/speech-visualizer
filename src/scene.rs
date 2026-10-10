//! What the display window shows: a tree of elements, changed through [`Scene`]'s
//! methods and rendered from scratch after every change.

mod command;
mod names;
mod ui;

use std::collections::HashMap;
use std::fmt;

pub use command::Command;
pub use names::Names;
pub use ui::use_scene;

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
    Rect { children: Vec<ElementId> },
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SceneError {
    #[error("no element {0}")]
    Missing(ElementId),
    #[error("element {0} is not a rect")]
    ExpectedRect(ElementId),
    #[error("element {0} is not text")]
    ExpectedText(ElementId),
}

/// A change that fails changes nothing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    entries: HashMap<ElementId, Entry>,
    roots: Vec<ElementId>,
    last_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    element: Element,
    /// None = top level.
    parent: Option<ElementId>,
}

impl Scene {
    /// Adds an empty rect at the end of `parent`'s children, or of the top level.
    pub fn add_rect(&mut self, parent: Option<ElementId>) -> Result<ElementId, SceneError> {
        let rect = Element::Rect {
            children: Vec::new(),
        };
        self.add(parent, rect)
    }

    /// Adds text at the end of `parent`'s children, or of the top level.
    pub fn add_text(
        &mut self,
        parent: Option<ElementId>,
        text: String,
    ) -> Result<ElementId, SceneError> {
        self.add(parent, Element::Text { text })
    }

    /// Replaces the text of a text element.
    pub fn set_text(&mut self, id: ElementId, text: String) -> Result<(), SceneError> {
        let entry = self.entries.get_mut(&id).ok_or(SceneError::Missing(id))?;
        match &mut entry.element {
            Element::Text { text: old } => *old = text,
            Element::Rect { .. } => return Err(SceneError::ExpectedText(id)),
        }
        Ok(())
    }

    /// Removes an element and everything in it, returning all of their ids.
    pub fn remove(&mut self, id: ElementId) -> Result<Vec<ElementId>, SceneError> {
        let entry = self.entries.remove(&id).ok_or(SceneError::Missing(id))?;
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
        Ok(removed)
    }

    /// Removes everything. Ids still aren't reused.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.roots.clear();
    }

    pub fn roots(&self) -> &[ElementId] {
        &self.roots
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.entries.get(&id).map(|entry| &entry.element)
    }

    pub fn contains(&self, id: ElementId) -> bool {
        self.entries.contains_key(&id)
    }

    fn add(
        &mut self,
        parent: Option<ElementId>,
        element: Element,
    ) -> Result<ElementId, SceneError> {
        let id = ElementId(self.last_id + 1);
        self.children_mut(parent)?.push(id);
        self.last_id = id.0;
        self.entries.insert(id, Entry { element, parent });
        Ok(id)
    }

    fn children_mut(
        &mut self,
        parent: Option<ElementId>,
    ) -> Result<&mut Vec<ElementId>, SceneError> {
        let Some(id) = parent else {
            return Ok(&mut self.roots);
        };
        let entry = self.entries.get_mut(&id).ok_or(SceneError::Missing(id))?;
        match &mut entry.element {
            Element::Rect { children } => Ok(children),
            Element::Text { .. } => Err(SceneError::ExpectedRect(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_nested_elements_with_fresh_ids() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(None).unwrap();
        let text = scene.add_text(Some(rect), "Hello there".into()).unwrap();
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
        let outer = scene.add_rect(None).unwrap();
        let inner = scene.add_rect(Some(outer)).unwrap();
        let text = scene.add_text(Some(inner), "deep".into()).unwrap();
        assert_eq!(scene.remove(outer), Ok(vec![outer, inner, text]));
        assert!(scene.roots().is_empty());
        assert_eq!(scene.get(text), None);
        assert_eq!(scene.add_text(None, "again".into()), Ok(ElementId(4)));
    }

    #[test]
    fn clear_does_not_reuse_ids() {
        let mut scene = Scene::default();
        scene.add_rect(None).unwrap();
        scene.clear();
        assert!(scene.roots().is_empty());
        assert_eq!(scene.add_rect(None), Ok(ElementId(2)));
    }

    #[test]
    fn failed_changes_change_nothing() {
        let mut scene = Scene::default();
        let text = scene.add_text(None, "hi".into()).unwrap();
        let before = scene.clone();
        assert_eq!(
            scene.add_rect(Some(text)),
            Err(SceneError::ExpectedRect(text))
        );
        assert_eq!(
            scene.set_text(ElementId(9), "x".into()),
            Err(SceneError::Missing(ElementId(9)))
        );
        assert_eq!(
            scene.remove(ElementId(9)),
            Err(SceneError::Missing(ElementId(9)))
        );
        assert_eq!(scene, before);
    }
}
