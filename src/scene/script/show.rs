//! Values as text, for what a script returns and the variables it keeps.

use std::fmt::Write;

use rhai::Dynamic;

use super::SceneRef;
use super::api::Handle;
use crate::scene::{Element, ElementId, Scene, SceneError};

/// `dynamic` as text: an element as [`element`] shows it, or as `#5 (removed)`, a string
/// in quotes, arrays and maps item by item, and anything else as rhai shows it.
pub(super) fn value(scene: &SceneRef, dynamic: &Dynamic) -> String {
    // A variable a closure captured is shared; look at its value.
    let dynamic = dynamic.flatten_clone();
    if let Some(Handle(id)) = dynamic.clone().try_cast::<Handle>() {
        return scene
            .with(|scene| element(scene, id))
            .unwrap_or_else(|_| format!("#{id} (removed)"));
    }
    if let Ok(text) = dynamic.as_immutable_string_ref() {
        return format!("{:?}", text.as_str());
    }
    if let Ok(array) = dynamic.as_array_ref() {
        let items: Vec<String> = array.iter().map(|item| value(scene, item)).collect();
        return format!("[{}]", items.join(", "));
    }
    if let Ok(map) = dynamic.as_map_ref() {
        let entries: Vec<String> = map
            .iter()
            .map(|(key, item)| format!("{key}: {}", value(scene, item)))
            .collect();
        return format!("#{{{}}}", entries.join(", "));
    }
    if dynamic.is_unit() {
        return "()".to_owned();
    }
    dynamic.to_string()
}

/// An element as scripts print it: its id, kind, text if it has any, and description if
/// it has one, like `#5 text "Hello" (greeting)` or `#4 rect (page header)`. Fails if it
/// was removed.
pub(super) fn element(scene: &Scene, id: ElementId) -> Result<String, SceneError> {
    let mut shown = match scene.get(id).ok_or(SceneError::Missing(id))? {
        Element::Rect { .. } => format!("#{id} rect"),
        Element::Text { text } => format!("#{id} text {text:?}"),
    };
    let description = scene.description(id)?;
    if !description.is_empty() {
        write!(shown, " ({description})").expect("writing to a String can't fail");
    }
    Ok(shown)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use rhai::{Array, Map};

    use super::*;

    fn scene() -> (SceneRef, Rc<RefCell<Scene>>) {
        let mut scene = Scene::default();
        let rect = scene.add_rect(None).unwrap();
        scene.set_description(rect, "page header".into()).unwrap();
        scene.add_text(Some(rect), "Hello".into()).unwrap();
        let scene = Rc::new(RefCell::new(scene));
        (SceneRef::new(scene.clone()), scene)
    }

    #[test]
    fn elements_show_their_kind_text_and_description() {
        let (_, scene) = scene();
        let scene = scene.borrow();
        assert_eq!(
            element(&scene, ElementId(1)),
            Ok("#1 rect (page header)".into())
        );
        assert_eq!(
            element(&scene, ElementId(2)),
            Ok(r#"#2 text "Hello""#.into())
        );
        assert_eq!(
            element(&scene, ElementId(9)),
            Err(SceneError::Missing(ElementId(9)))
        );
    }

    #[test]
    fn values_show_like_rhai_literals() {
        let (scene, _) = scene();
        let element = Dynamic::from(Handle(ElementId(2)));
        let array: Array = vec![Dynamic::from(1_i64), Dynamic::from("a\"b"), Dynamic::UNIT];
        let mut map = Map::new();
        map.insert("el".into(), element.clone());
        map.insert("n".into(), Dynamic::from(2.5_f64));

        assert_eq!(value(&scene, &element), r#"#2 text "Hello""#);
        // A value is shown as it is, even an element that's gone.
        let removed = Dynamic::from(Handle(ElementId(9)));
        assert_eq!(value(&scene, &removed), "#9 (removed)");
        assert_eq!(value(&scene, &Dynamic::from("hi")), r#""hi""#);
        assert_eq!(value(&scene, &Dynamic::from(array)), r#"[1, "a\"b", ()]"#);
        assert_eq!(
            value(&scene, &Dynamic::from_map(map)),
            r#"#{el: #2 text "Hello", n: 2.5}"#
        );
        assert_eq!(value(&scene, &Dynamic::UNIT), "()");
        assert_eq!(value(&scene, &Dynamic::from(true)), "true");
    }
}
