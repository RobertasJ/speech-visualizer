//! What the display window shows: a tree of Freya elements under a root rect that fills
//! the window. Each element is kept as Freya's own, and changed through [`Scene`]'s
//! methods; the display draws them from scratch after every change.
//!
//! - [`rect`], [`paragraph`] and [`span`]: each kind of element, and its properties.
//! - [`style`]: groups of properties several kinds share.
//! - [`value`]: values as scripts write them, and back.
//! - [`script`]: Rhai scripts that change a scene.

#[macro_use]
mod style;
mod paragraph;
mod rect;
mod script;
mod span;
mod ui;
mod value;

use std::collections::HashMap;
use std::fmt;

use freya::elements::paragraph::ParagraphElement;
use freya::elements::rect::RectElement;
use freya::prelude::Span;
use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};

pub use script::{FUNCTIONS, Outcome, Script};
pub use style::{GroupDocs, PropertyDocs, PropertyError, Value};
pub use ui::use_scene;

/// Identifies an element. The root is 0, and the rest are numbered by the [`Scene`] as
/// they're added, and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ElementId(pub u64);

impl ElementId {
    pub const ROOT: ElementId = ElementId(0);
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// An element, kept as Freya keeps it.
#[derive(Clone, PartialEq)]
pub enum Element {
    Rect(RectElement),
    Paragraph(ParagraphElement),
    Span(Span<'static>),
}

/// The kinds of element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Rect,
    Paragraph,
    Span,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Rect, Kind::Paragraph, Kind::Span];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Rect => "rect",
            Kind::Paragraph => "paragraph",
            Kind::Span => "span",
        }
    }

    /// Whether an element of this kind can hold one of `child`: a rect holds rects and
    /// paragraphs, and a paragraph spans.
    pub fn holds(self, child: Kind) -> bool {
        matches!(
            (self, child),
            (Kind::Rect, Kind::Rect | Kind::Paragraph) | (Kind::Paragraph, Kind::Span)
        )
    }

    /// What it holds, for errors.
    fn holding(self) -> &'static str {
        match self {
            Kind::Rect => "holds rects and paragraphs; spans go in a paragraph",
            Kind::Paragraph => "holds only spans",
            Kind::Span => "can't hold anything",
        }
    }

    /// Its property groups, with the values a new element of it has, for the docs.
    pub fn docs(self) -> Vec<GroupDocs> {
        match self {
            Kind::Rect => rect::PARTS.docs(&rect::new()),
            Kind::Paragraph => paragraph::PARTS.docs(&paragraph::new()),
            Kind::Span => span::PARTS.docs(&span::new(String::new())),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Element {
    pub fn kind(&self) -> Kind {
        match self {
            Element::Rect(_) => Kind::Rect,
            Element::Paragraph(_) => Kind::Paragraph,
            Element::Span(_) => Kind::Span,
        }
    }

    /// Its property called `name`, if its kind has one.
    pub fn property(&self, name: &str) -> Option<Value> {
        match self {
            Element::Rect(rect) => rect::PARTS.get(rect, name),
            Element::Paragraph(paragraph) => paragraph::PARTS.get(paragraph, name),
            Element::Span(span) => span::PARTS.get(span, name),
        }
    }

    /// Sets, or for a list adds to, its property called `name`, if its kind has one.
    fn set_property(&mut self, name: &str, value: &str) -> Option<Result<(), PropertyError>> {
        match self {
            Element::Rect(rect) => rect::PARTS.set(rect, name, value),
            Element::Paragraph(paragraph) => paragraph::PARTS.set(paragraph, name, value),
            Element::Span(span) => span::PARTS.set(span, name, value),
        }
    }

    fn clear_property(&mut self, name: &str) -> Option<Result<(), PropertyError>> {
        match self {
            Element::Rect(rect) => rect::PARTS.clear(rect, name),
            Element::Paragraph(paragraph) => paragraph::PARTS.clear(paragraph, name),
            Element::Span(span) => span::PARTS.clear(span, name),
        }
    }

    fn shorthand(&mut self, name: &str) -> Option<()> {
        match self {
            Element::Rect(rect) => rect::PARTS.shorthand(rect, name),
            Element::Paragraph(paragraph) => paragraph::PARTS.shorthand(paragraph, name),
            Element::Span(span) => span::PARTS.shorthand(span, name),
        }
    }

    /// Its properties that differ from a new element of its kind.
    pub fn changed(&self) -> Vec<(&'static str, Value)> {
        match self {
            Element::Rect(rect) => rect::PARTS.changed(rect, &rect::new()),
            Element::Paragraph(paragraph) => paragraph::PARTS.changed(paragraph, &paragraph::new()),
            Element::Span(span) => span::PARTS.changed(span, &span::new(String::new())),
        }
    }
}

/// Shows its kind and the properties it changed, since Freya's elements don't show.
impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}", self.kind(), self.changed())
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SceneError {
    #[error("no element {0}")]
    Missing(ElementId),
    #[error("element {parent} is a {kind}, which {holding}")]
    CantHold {
        parent: ElementId,
        kind: Kind,
        holding: &'static str,
    },
    #[error("the root can't be removed")]
    Root,
    #[error("element {id} is a {kind}, which has no {name}")]
    NoProperty {
        id: ElementId,
        kind: Kind,
        name: String,
    },
    #[error(transparent)]
    Property(#[from] PropertyError),
}

/// A change that fails changes nothing.
#[derive(Clone, PartialEq)]
pub struct Scene {
    entries: HashMap<ElementId, Entry>,
    last_id: u64,
}

#[derive(Clone, PartialEq)]
struct Entry {
    element: Element,
    /// None only for the root.
    parent: Option<ElementId>,
    children: Vec<ElementId>,
    /// What the element is for, so it can be found later; empty if it has none.
    description: String,
}

/// Just the root, which fills the window.
impl Default for Scene {
    fn default() -> Self {
        let root = Entry {
            element: Element::Rect(rect::root()),
            parent: None,
            children: Vec::new(),
            description: String::new(),
        };
        Scene {
            entries: HashMap::from([(ElementId::ROOT, root)]),
            last_id: 0,
        }
    }
}

/// Shows it as the agent sees it.
impl fmt::Debug for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        f.write_str(&json)
    }
}

impl Scene {
    /// Adds a new rect at the end of `parent`'s children.
    pub fn add_rect(&mut self, parent: ElementId) -> Result<ElementId, SceneError> {
        self.add(parent, Element::Rect(rect::new()))
    }

    /// Adds a new paragraph at the end of `parent`'s children.
    pub fn add_paragraph(&mut self, parent: ElementId) -> Result<ElementId, SceneError> {
        self.add(parent, Element::Paragraph(paragraph::new()))
    }

    /// Adds a span with `text` at the end of paragraph `parent`'s spans.
    pub fn add_span(&mut self, parent: ElementId, text: String) -> Result<ElementId, SceneError> {
        self.add(parent, Element::Span(span::new(text)))
    }

    /// Replaces the description of an element; empty means none.
    pub fn set_description(
        &mut self,
        id: ElementId,
        description: String,
    ) -> Result<(), SceneError> {
        self.entry_mut(id)?.description = description;
        Ok(())
    }

    /// Its property called `name`.
    pub fn property(&self, id: ElementId, name: &str) -> Result<Value, SceneError> {
        let element = &self.entry(id)?.element;
        element
            .property(name)
            .ok_or_else(|| no_property(id, element.kind(), name))
    }

    /// Sets its property called `name` from `value`, or for a list adds to it.
    pub fn set_property(
        &mut self,
        id: ElementId,
        name: &str,
        value: &str,
    ) -> Result<(), SceneError> {
        let element = &mut self.entry_mut(id)?.element;
        let kind = element.kind();
        match element.set_property(name, value) {
            Some(result) => Ok(result?),
            None => Err(no_property(id, kind, name)),
        }
    }

    /// Empties its list property called `name`.
    pub fn clear_property(&mut self, id: ElementId, name: &str) -> Result<(), SceneError> {
        let element = &mut self.entry_mut(id)?.element;
        let kind = element.kind();
        match element.clear_property(name) {
            Some(result) => Ok(result?),
            None => Err(no_property(id, kind, name)),
        }
    }

    /// Applies one of Freya's shorthand setters, like `expanded`.
    pub fn apply_shorthand(&mut self, id: ElementId, name: &str) -> Result<(), SceneError> {
        let element = &mut self.entry_mut(id)?.element;
        let kind = element.kind();
        element
            .shorthand(name)
            .ok_or_else(|| no_property(id, kind, name))
    }

    /// Removes an element and everything in it, returning all of their ids.
    pub fn remove(&mut self, id: ElementId) -> Result<Vec<ElementId>, SceneError> {
        if id == ElementId::ROOT {
            return Err(SceneError::Root);
        }
        let entry = self.entries.remove(&id).ok_or(SceneError::Missing(id))?;
        if let Some(parent) = entry
            .parent
            .and_then(|parent| self.entries.get_mut(&parent))
        {
            parent.children.retain(|&child| child != id);
        }
        let mut removed = vec![id];
        let mut orphans = entry.children;
        orphans.reverse();
        while let Some(child) = orphans.pop() {
            if let Some(entry) = self.entries.remove(&child) {
                removed.push(child);
                orphans.extend(entry.children.into_iter().rev());
            }
        }
        Ok(removed)
    }

    /// Removes everything in the root. Ids still aren't reused.
    pub fn clear(&mut self) {
        self.entries.retain(|&id, _| id == ElementId::ROOT);
        if let Some(root) = self.entries.get_mut(&ElementId::ROOT) {
            root.children.clear();
        }
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.entries.get(&id).map(|entry| &entry.element)
    }

    pub fn contains(&self, id: ElementId) -> bool {
        self.entries.contains_key(&id)
    }

    /// What's in an element, in order.
    pub fn children(&self, id: ElementId) -> Result<&[ElementId], SceneError> {
        self.entry(id).map(|entry| entry.children.as_slice())
    }

    /// The description of an element; empty if it has none.
    pub fn description(&self, id: ElementId) -> Result<&str, SceneError> {
        self.entry(id).map(|entry| entry.description.as_str())
    }

    /// The element an element is in, or None for the root.
    pub fn parent(&self, id: ElementId) -> Result<Option<ElementId>, SceneError> {
        self.entry(id).map(|entry| entry.parent)
    }

    fn entry(&self, id: ElementId) -> Result<&Entry, SceneError> {
        self.entries.get(&id).ok_or(SceneError::Missing(id))
    }

    fn entry_mut(&mut self, id: ElementId) -> Result<&mut Entry, SceneError> {
        self.entries.get_mut(&id).ok_or(SceneError::Missing(id))
    }

    fn add(&mut self, parent: ElementId, element: Element) -> Result<ElementId, SceneError> {
        let kind = self.entry(parent)?.element.kind();
        if !kind.holds(element.kind()) {
            return Err(SceneError::CantHold {
                parent,
                kind,
                holding: kind.holding(),
            });
        }
        let id = ElementId(self.last_id + 1);
        self.last_id = id.0;
        self.entry_mut(parent)?.children.push(id);
        let entry = Entry {
            element,
            parent: Some(parent),
            children: Vec::new(),
            description: String::new(),
        };
        self.entries.insert(id, entry);
        Ok(id)
    }
}

fn no_property(id: ElementId, kind: Kind, name: &str) -> SceneError {
    SceneError::NoProperty {
        id,
        kind,
        name: name.to_owned(),
    }
}

/// Every property group, each with the kinds of element that have it, in the order
/// they come in: what's in the docs.
pub fn property_docs() -> Vec<(GroupDocs, Vec<Kind>)> {
    let mut groups: Vec<(GroupDocs, Vec<Kind>)> = Vec::new();
    for kind in Kind::ALL {
        for docs in kind.docs() {
            match groups.iter_mut().find(|(group, _)| group.name == docs.name) {
                Some((_, kinds)) => kinds.push(kind),
                None => groups.push((docs, vec![kind])),
            }
        }
    }
    groups
}

/// Serializes as the tree it is, for reading: the id the next element will get, and the
/// root, each element with its id, its description if it has one, what's in it, and the
/// properties it has that differ from a new element's:
///
/// ```json
/// {"next_id": 3, "root": {"id": 0, "rect": [
///   {"id": 1, "desc": "greeting", "paragraph": [{"id": 2, "span": "Hi"}], "color": "red"}
/// ], "width": "fill", "height": "fill"}}
/// ```
impl Serialize for Scene {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("next_id", &(self.last_id + 1))?;
        map.serialize_entry("root", &Node(self, ElementId::ROOT))?;
        map.end()
    }
}

/// Elements of a scene, serialized as a list of trees.
struct Nodes<'a>(&'a Scene, &'a [ElementId]);

impl Serialize for Nodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Nodes(scene, ids) = *self;
        serializer.collect_seq(ids.iter().map(|&id| Node(scene, id)))
    }
}

/// An element of a scene, serialized with what's in it and the properties it changed.
struct Node<'a>(&'a Scene, ElementId);

impl Serialize for Node<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Node(scene, id) = *self;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &id)?;
        // Children and the root only list elements that exist.
        let Some(entry) = scene.entries.get(&id) else {
            return map.end();
        };
        if !entry.description.is_empty() {
            map.serialize_entry("desc", &entry.description)?;
        }
        let kind = entry.element.kind();
        match &entry.element {
            Element::Span(span) => map.serialize_entry("span", span.text.as_ref())?,
            _ => map.serialize_entry(kind.name(), &Nodes(scene, &entry.children))?,
        }
        for (name, value) in entry.element.changed() {
            // A span's text is under "span" already.
            if !(kind == Kind::Span && name == "text") {
                map.serialize_entry(name, &value)?;
            }
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn set(scene: &mut Scene, id: ElementId, name: &str, value: &str) {
        scene.set_property(id, name, value).unwrap();
    }

    fn one(value: &str) -> Result<Value, SceneError> {
        Ok(Value::One(value.to_owned()))
    }

    #[test]
    fn starts_with_a_root_that_fills_the_window() {
        let scene = Scene::default();
        assert_eq!(scene.children(ElementId::ROOT), Ok(&[][..]));
        assert_eq!(scene.parent(ElementId::ROOT), Ok(None));
        assert_eq!(scene.property(ElementId::ROOT, "width"), one("fill"));
    }

    #[test]
    fn adds_nested_elements_with_fresh_ids() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        let paragraph = scene.add_paragraph(rect).unwrap();
        let span = scene.add_span(paragraph, "Hello".into()).unwrap();
        assert_eq!([rect, paragraph, span], [1, 2, 3].map(ElementId));
        assert_eq!(scene.children(ElementId::ROOT), Ok(&[rect][..]));
        assert_eq!(scene.children(rect), Ok(&[paragraph][..]));
        assert_eq!(scene.parent(span), Ok(Some(paragraph)));
        assert_eq!(scene.get(span).map(Element::kind), Some(Kind::Span));
        assert_eq!(scene.property(span, "text"), one("Hello"));
    }

    #[test]
    fn each_kind_holds_only_what_it_can() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        let paragraph = scene.add_paragraph(rect).unwrap();
        let span = scene.add_span(paragraph, "x".into()).unwrap();
        let before = scene.clone();

        let error = scene.add_span(rect, "x".into()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "element 1 is a rect, which holds rects and paragraphs; spans go in a paragraph"
        );
        let error = scene.add_rect(paragraph).unwrap_err();
        assert_eq!(
            error.to_string(),
            "element 2 is a paragraph, which holds only spans"
        );
        let error = scene.add_paragraph(span).unwrap_err();
        assert_eq!(
            error.to_string(),
            "element 3 is a span, which can't hold anything"
        );
        assert_eq!(scene, before);
    }

    #[test]
    fn properties_are_read_and_set_by_name() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        set(&mut scene, rect, "width", "px(120)");
        set(&mut scene, rect, "background", "red");
        assert_eq!(scene.property(rect, "width"), one("px(120)"));
        let Some(Element::Rect(element)) = scene.get(rect) else {
            panic!("not a rect")
        };
        assert_eq!(element.layout.layout.width, freya::prelude::Size::px(120.));

        let error = scene.set_property(rect, "width", "huge").unwrap_err();
        assert!(
            error.to_string().starts_with("width: expected a size like"),
            "{error}"
        );
    }

    #[test]
    fn a_kind_has_only_its_properties() {
        let mut scene = Scene::default();
        let paragraph = scene.add_paragraph(ElementId::ROOT).unwrap();
        let span = scene.add_span(paragraph, "x".into()).unwrap();
        let error = scene.set_property(span, "width", "fill").unwrap_err();
        assert_eq!(error.to_string(), "element 2 is a span, which has no width");
        assert!(scene.property(paragraph, "background").is_err());
        assert!(scene.property(paragraph, "max_lines").is_ok());
    }

    #[test]
    fn lists_add_until_cleared() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        set(&mut scene, rect, "border", "width(1) fill(red)");
        set(&mut scene, rect, "border", "width(2) fill(blue)");
        assert_eq!(
            scene.property(rect, "border"),
            Ok(Value::List(vec![
                "width(1) fill(red)".into(),
                "width(2) fill(blue)".into()
            ]))
        );
        scene.clear_property(rect, "border").unwrap();
        assert_eq!(scene.property(rect, "border"), Ok(Value::List(vec![])));
        let error = scene.clear_property(rect, "width").unwrap_err();
        assert_eq!(
            error.to_string(),
            "width isn't a list, so it can't be cleared"
        );
    }

    #[test]
    fn shorthands_set_the_properties_they_stand_for() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        scene.apply_shorthand(rect, "expanded").unwrap();
        scene.apply_shorthand(rect, "center").unwrap();
        scene.apply_shorthand(rect, "rounded_lg").unwrap();
        for (name, value) in [
            ("width", "fill"),
            ("main_align", "center"),
            ("cross_align", "center"),
            ("corner_radius", "8"),
        ] {
            assert_eq!(scene.property(rect, name), one(value), "{name}");
        }
        assert!(scene.apply_shorthand(rect, "sideways").is_err());
    }

    #[test]
    fn text_style_inherits_until_set() {
        let mut scene = Scene::default();
        let paragraph = scene.add_paragraph(ElementId::ROOT).unwrap();
        let span = scene.add_span(paragraph, "x".into()).unwrap();
        assert_eq!(scene.property(span, "color"), one("inherit"));
        set(&mut scene, span, "color", "red");
        assert_eq!(scene.property(span, "color"), one("red"));
        set(&mut scene, span, "color", "inherit");
        assert_eq!(scene.property(span, "color"), one("inherit"));
    }

    #[test]
    fn remove_takes_children_and_ids_are_not_reused() {
        let mut scene = Scene::default();
        let outer = scene.add_rect(ElementId::ROOT).unwrap();
        let paragraph = scene.add_paragraph(outer).unwrap();
        let span = scene.add_span(paragraph, "deep".into()).unwrap();
        assert_eq!(scene.remove(outer), Ok(vec![outer, paragraph, span]));
        assert_eq!(scene.children(ElementId::ROOT), Ok(&[][..]));
        assert_eq!(scene.get(span), None);
        assert_eq!(scene.add_rect(ElementId::ROOT), Ok(ElementId(4)));
        assert_eq!(scene.remove(ElementId::ROOT), Err(SceneError::Root));
    }

    #[test]
    fn clear_empties_the_root_without_reusing_ids() {
        let mut scene = Scene::default();
        scene.add_rect(ElementId::ROOT).unwrap();
        set(&mut scene, ElementId::ROOT, "background", "black");
        scene.clear();
        assert_eq!(scene.children(ElementId::ROOT), Ok(&[][..]));
        assert_eq!(scene.add_rect(ElementId::ROOT), Ok(ElementId(2)));
        // The root keeps its own properties.
        assert_eq!(scene.property(ElementId::ROOT, "background"), one("black"));
    }

    #[test]
    fn serializes_as_a_tree_with_what_changed() {
        let mut scene = Scene::default();
        let rect = scene.add_rect(ElementId::ROOT).unwrap();
        scene.set_description(rect, "header".into()).unwrap();
        set(&mut scene, rect, "padding", "8");
        set(&mut scene, rect, "shadow", "y(2) blur(4) color(black)");
        let paragraph = scene.add_paragraph(rect).unwrap();
        let span = scene.add_span(paragraph, "Hello".into()).unwrap();
        set(&mut scene, span, "font_weight", "bold");
        assert_eq!(
            serde_json::to_value(&scene).unwrap(),
            json!({
                "next_id": 4,
                "root": {"id": 0, "rect": [
                    {"id": 1, "desc": "header", "rect": [
                        {"id": 2, "paragraph": [
                            {"id": 3, "span": "Hello", "font_weight": "bold"},
                        ]},
                    ], "padding": "8", "shadow": ["x(0) y(2) blur(4) spread(0) color(black)"]},
                ], "width": "fill", "height": "fill"},
            })
        );
    }

    #[test]
    fn docs_list_each_group_with_the_kinds_that_have_it() {
        let docs = property_docs();
        let kinds = |name: &str| {
            docs.iter()
                .find(|(group, _)| group.name == name)
                .map(|(_, kinds)| kinds.clone())
        };
        assert_eq!(kinds("layout"), Some(vec![Kind::Rect, Kind::Paragraph]));
        assert_eq!(
            kinds("text"),
            Some(vec![Kind::Rect, Kind::Paragraph, Kind::Span])
        );
        assert_eq!(kinds("decoration"), Some(vec![Kind::Rect]));
        assert_eq!(kinds("span"), Some(vec![Kind::Span]));
        let (layout, _) = docs
            .iter()
            .find(|(group, _)| group.name == "layout")
            .unwrap();
        let width = layout
            .properties
            .iter()
            .find(|p| p.name == "width")
            .unwrap();
        assert_eq!(width.default, Value::One("auto".into()));
    }
}
