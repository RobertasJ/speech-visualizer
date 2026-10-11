//! The functions scripts call: finding and adding elements, and the `Element` type
//! with its properties and methods. [`FUNCTIONS`](super::FUNCTIONS) describes them.

use rhai::{Array, Dynamic, Engine, EvalAltResult, INT};

use super::{SceneRef, show};
use crate::scene::{Element, ElementId, Scene, SceneError};

/// A script's handle to an element, which scripts know as `Element`. Using one whose
/// element was removed fails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Handle(pub(super) ElementId);

type Result<T> = std::result::Result<T, Box<EvalAltResult>>;

/// Registers the `Element` type and the functions on `engine`, all changing `scene`.
pub(super) fn register(engine: &mut Engine, scene: &SceneRef) {
    engine.register_type_with_name::<Handle>("Element");

    // Finding and adding.
    let s = scene.clone();
    engine.register_fn("el", move |id: INT| -> Result<Handle> {
        let missing = || -> Box<EvalAltResult> { format!("no element {id}").into() };
        let id = u64::try_from(id).map(ElementId).map_err(|_| missing())?;
        if s.with(|scene| scene.contains(id)) {
            Ok(Handle(id))
        } else {
            Err(missing())
        }
    });
    let s = scene.clone();
    engine.register_fn("rect", move || {
        change(&s, |scene| scene.add_rect(None)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("text", move |content: String| {
        change(&s, |scene| scene.add_text(None, content)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("roots", move || s.with(|scene| handles(scene.roots())));
    let s = scene.clone();
    engine.register_fn("clear", move || s.with(Scene::clear));

    // Adding inside a rect.
    let s = scene.clone();
    engine.register_fn("add_rect", move |parent: &mut Handle| {
        change(&s, |scene| scene.add_rect(Some(parent.0))).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("add_text", move |parent: &mut Handle, content: String| {
        change(&s, |scene| scene.add_text(Some(parent.0), content)).map(Handle)
    });

    // Properties.
    let s = scene.clone();
    engine.register_get("text", move |handle: &mut Handle| -> Result<String> {
        let id = handle.0;
        change(&s, |scene| match scene.get(id) {
            Some(Element::Text { text }) => Ok(text.clone()),
            Some(Element::Rect { .. }) => Err(SceneError::ExpectedText(id)),
            None => Err(SceneError::Missing(id)),
        })
    });
    let s = scene.clone();
    engine.register_set("text", move |handle: &mut Handle, text: String| {
        change(&s, |scene| scene.set_text(handle.0, text))
    });
    let s = scene.clone();
    engine.register_get("desc", move |handle: &mut Handle| {
        change(&s, |scene| scene.description(handle.0).map(str::to_owned))
    });
    let s = scene.clone();
    engine.register_set("desc", move |handle: &mut Handle, description: String| {
        change(&s, |scene| scene.set_description(handle.0, description))
    });
    let s = scene.clone();
    engine.register_get("id", move |handle: &mut Handle| -> Result<INT> {
        let id = handle.0;
        change(&s, |scene| {
            if scene.contains(id) {
                Ok(id.0 as INT)
            } else {
                Err(SceneError::Missing(id))
            }
        })
    });
    let s = scene.clone();
    engine.register_get("parent", move |handle: &mut Handle| -> Result<Dynamic> {
        let parent = change(&s, |scene| scene.parent(handle.0))?;
        Ok(parent.map_or(Dynamic::UNIT, |id| Dynamic::from(Handle(id))))
    });
    let s = scene.clone();
    engine.register_get("children", move |handle: &mut Handle| -> Result<Array> {
        let id = handle.0;
        change(&s, |scene| match scene.get(id) {
            Some(Element::Rect { children }) => Ok(handles(children)),
            Some(Element::Text { .. }) => Err(SceneError::ExpectedRect(id)),
            None => Err(SceneError::Missing(id)),
        })
    });

    // Methods.
    let s = scene.clone();
    engine.register_fn("remove", move |handle: &mut Handle| {
        change(&s, |scene| scene.remove(handle.0)).map(|_removed| ())
    });
    // Rhai's own `print` and `debug` call `to_string` and `to_debug`, but if those
    // fail they show just the type's name. Having elements' own lets printing a removed
    // one throw, like using it any other way does.
    for name in ["to_string", "to_debug", "print", "debug"] {
        let s = scene.clone();
        engine.register_fn(name, move |handle: &mut Handle| {
            change(&s, |scene| show::element(scene, handle.0))
        });
    }
}

/// Calls `f` with the scene, turning its error into a script error.
fn change<R>(
    scene: &SceneRef,
    f: impl FnOnce(&mut Scene) -> std::result::Result<R, SceneError>,
) -> Result<R> {
    scene.with(f).map_err(|err| err.to_string().into())
}

/// Handles to `ids`, as a script array.
fn handles(ids: &[ElementId]) -> Array {
    ids.iter().map(|&id| Dynamic::from(Handle(id))).collect()
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::cell::RefCell;
    use std::rc::Rc;

    use rhai::Scope;

    use super::*;

    /// An engine with only these functions, on a scene the test can look at.
    struct Api {
        engine: Engine,
        scope: Scope<'static>,
        scene: Rc<RefCell<Scene>>,
    }

    impl Api {
        fn new() -> Self {
            let scene = Rc::new(RefCell::new(Scene::default()));
            let mut engine = Engine::new();
            register(&mut engine, &SceneRef::new(scene.clone()));
            Api {
                engine,
                scope: Scope::new(),
                scene,
            }
        }

        /// Runs `code`, keeping its variables for the next.
        fn eval<T: Any + Clone>(&mut self, code: &str) -> std::result::Result<T, String> {
            self.engine
                .eval_with_scope::<T>(&mut self.scope, code)
                .map_err(|err| err.to_string())
        }

        /// Runs `code`, which must succeed, for what it changes.
        fn run(&mut self, code: &str) {
            if let Err(error) = self.eval::<Dynamic>(code) {
                panic!("{code}: {error}");
            }
        }

        /// Runs `code`, which must fail, and returns the error.
        fn fails(&mut self, code: &str) -> String {
            self.eval::<Dynamic>(code)
                .err()
                .unwrap_or_else(|| panic!("{code} succeeded"))
        }
    }

    #[test]
    fn adds_nests_and_edits() {
        let mut api = Api::new();
        api.run(
            r#"
            let header = rect();
            header.desc = "page header";
            let title = header.add_text("Hello");
            title.text = "Welcome";
            let inner = header.add_rect();
            inner.add_text("deep");
            text("after");
            "#,
        );
        let scene = api.scene.borrow();
        let [header, title, inner, deep, after] = [1, 2, 3, 4, 5].map(ElementId);
        assert_eq!(scene.roots(), &[header, after]);
        assert_eq!(
            scene.get(header),
            Some(&Element::Rect {
                children: vec![title, inner]
            })
        );
        assert_eq!(
            scene.get(title),
            Some(&Element::Text {
                text: "Welcome".into()
            })
        );
        assert_eq!(scene.description(header), Ok("page header"));
        assert_eq!(scene.parent(deep), Ok(Some(inner)));
    }

    #[test]
    fn reads_back_what_was_added() {
        let mut api = Api::new();
        api.run(r#"let panel = rect(); panel.desc = "panel"; let t = panel.add_text("Hi");"#);
        assert_eq!(api.eval::<String>("el(2).text"), Ok("Hi".into()));
        assert_eq!(api.eval::<INT>("t.id"), Ok(2));
        assert_eq!(api.eval::<String>("t.parent.desc"), Ok("panel".into()));
        assert_eq!(api.eval::<String>("t.desc"), Ok(String::new()));
        assert_eq!(api.eval::<()>("panel.parent"), Ok(()));
        assert_eq!(
            api.eval::<Handle>("panel.children[0]"),
            Ok(Handle(ElementId(2)))
        );
        assert_eq!(api.eval::<Handle>("roots()[0]"), Ok(Handle(ElementId(1))));
        assert_eq!(api.eval::<INT>("roots().len()"), Ok(1));
    }

    #[test]
    fn el_of_a_missing_id_fails_with_the_message_and_line() {
        let mut api = Api::new();
        let error = api.fails("let a = 1;\nel(9)");
        assert!(error.contains("no element 9"), "{error}");
        assert!(error.contains("line 2"), "{error}");
        assert!(api.fails("el(-1)").contains("no element -1"));
    }

    #[test]
    fn text_on_a_rect_and_adding_to_text_fail() {
        let mut api = Api::new();
        api.run(r#"let r = rect(); let t = text("hi");"#);
        assert!(api.fails("r.text").contains("element 1 is not text"));
        assert!(
            api.fails(r#"r.text = "x""#)
                .contains("element 1 is not text")
        );
        assert!(
            api.fails(r#"t.add_text("x")"#)
                .contains("element 2 is not a rect")
        );
        assert!(
            api.fails("t.add_rect()")
                .contains("element 2 is not a rect")
        );
        assert!(api.fails("t.children").contains("element 2 is not a rect"));
    }

    #[test]
    fn remove_takes_children_and_their_handles_then_fail() {
        let mut api = Api::new();
        api.run(
            r#"let outer = rect(); let inner = outer.add_rect(); let leaf = inner.add_text("x");"#,
        );
        api.run("outer.remove()");
        assert!(api.scene.borrow().roots().is_empty());
        for (code, id) in [
            ("leaf.text", 3),
            ("leaf.text = \"y\"", 3),
            ("inner.id", 2),
            ("outer.desc", 1),
            ("outer.parent", 1),
            ("inner.children", 2),
            ("inner.add_text(\"y\")", 2),
            ("outer.remove()", 1),
            ("print(leaf)", 3),
            ("debug(leaf)", 3),
            ("leaf.to_string()", 3),
            ("el(3)", 3),
        ] {
            let error = api.fails(code);
            assert!(
                error.contains(&format!("no element {id}")),
                "{code}: {error}"
            );
        }
    }

    #[test]
    fn clear_removes_everything() {
        let mut api = Api::new();
        api.run(r#"rect(); text("hi"); clear();"#);
        assert!(api.scene.borrow().roots().is_empty());
        assert_eq!(api.eval::<INT>("roots().len()"), Ok(0));
    }

    #[test]
    fn print_and_debug_show_elements() {
        let mut api = Api::new();
        api.run(r#"let t = text("Hi"); t.desc = "greeting";"#);
        assert_eq!(
            api.eval::<String>("t.to_string()"),
            Ok(r#"#1 text "Hi" (greeting)"#.into())
        );
        assert_eq!(
            api.eval::<String>("`${t}`"),
            Ok(r#"#1 text "Hi" (greeting)"#.into())
        );
        assert_eq!(
            api.eval::<String>("t.to_debug()"),
            Ok(r#"#1 text "Hi" (greeting)"#.into())
        );
    }
}
