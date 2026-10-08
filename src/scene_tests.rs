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
