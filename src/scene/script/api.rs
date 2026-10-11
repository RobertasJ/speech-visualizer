//! The functions scripts call: finding and adding elements, and the `Element` type
//! with its properties and methods. [`FUNCTIONS`](super::FUNCTIONS) describes them, and
//! each kind's property tables describe its properties.

use std::any::Any;
use std::collections::HashSet;

use rhai::{Array, Dynamic, Engine, EvalAltResult, INT};

use super::{SceneRef, show};
use crate::scene::{ElementId, Scene, SceneError, Value, property_docs};

/// A script's handle to an element, which scripts know as `Element`. Using one whose
/// element was removed fails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Handle(pub(super) ElementId);

type Result<T> = std::result::Result<T, Box<EvalAltResult>>;

/// Registers the `Element` type and the functions on `engine`, all changing `scene`.
pub(super) fn register(engine: &mut Engine, scene: &SceneRef) {
    engine.register_type_with_name::<Handle>("Element");
    register_finding_and_adding(engine, scene);
    register_element(engine, scene);
    register_properties(engine, scene);
}

fn register_finding_and_adding(engine: &mut Engine, scene: &SceneRef) {
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
    engine.register_fn("root", || Handle(ElementId::ROOT));
    let s = scene.clone();
    engine.register_fn("rect", move || {
        change(&s, |scene| scene.add_rect(ElementId::ROOT)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("paragraph", move || {
        change(&s, |scene| scene.add_paragraph(ElementId::ROOT)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("clear", move || s.with(Scene::clear));

    let s = scene.clone();
    engine.register_fn("add_rect", move |parent: &mut Handle| {
        change(&s, |scene| scene.add_rect(parent.0)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("add_paragraph", move |parent: &mut Handle| {
        change(&s, |scene| scene.add_paragraph(parent.0)).map(Handle)
    });
    let s = scene.clone();
    engine.register_fn("add_span", move |parent: &mut Handle, text: String| {
        change(&s, |scene| scene.add_span(parent.0, text)).map(Handle)
    });
}

/// What every element has, whatever its kind.
fn register_element(engine: &mut Engine, scene: &SceneRef) {
    let s = scene.clone();
    engine.register_get("kind", move |handle: &mut Handle| -> Result<String> {
        let id = handle.0;
        change(&s, |scene| {
            let element = scene.get(id).ok_or(SceneError::Missing(id))?;
            Ok(element.kind().name().to_owned())
        })
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
    engine.register_get("desc", move |handle: &mut Handle| {
        change(&s, |scene| scene.description(handle.0).map(str::to_owned))
    });
    let s = scene.clone();
    engine.register_set("desc", move |handle: &mut Handle, description: String| {
        change(&s, |scene| scene.set_description(handle.0, description))
    });
    let s = scene.clone();
    engine.register_get("parent", move |handle: &mut Handle| -> Result<Dynamic> {
        let parent = change(&s, |scene| scene.parent(handle.0))?;
        Ok(parent.map_or(Dynamic::UNIT, |id| Dynamic::from(Handle(id))))
    });
    let s = scene.clone();
    engine.register_get("children", move |handle: &mut Handle| -> Result<Array> {
        change(&s, |scene| scene.children(handle.0).map(handles))
    });
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

/// Every property and shorthand any kind of element has. Using one an element's kind
/// doesn't have fails.
fn register_properties(engine: &mut Engine, scene: &SceneRef) {
    let mut registered = HashSet::new();
    for (group, _) in property_docs() {
        for property in &group.properties {
            if registered.insert(property.name) {
                register_property(engine, scene, property.name, property.list);
            }
        }
        for &(name, _) in &group.shorthands {
            if registered.insert(name) {
                register_shorthand(engine, scene, name);
            }
        }
    }
}

/// `e.name` to read it, `e.name = value` to set it, and `e.name(value)` to set it and
/// get `e` back, for chaining. A value can be text or a number. A list property also
/// gets `e.clear_name()`.
fn register_property(engine: &mut Engine, scene: &SceneRef, name: &'static str, list: bool) {
    let s = scene.clone();
    engine.register_get(name, move |handle: &mut Handle| -> Result<Dynamic> {
        Ok(match change(&s, |scene| scene.property(handle.0, name))? {
            Value::One(value) => value.into(),
            Value::List(items) => items
                .into_iter()
                .map(Dynamic::from)
                .collect::<Array>()
                .into(),
        })
    });
    register_setters::<String>(engine, scene, name);
    register_setters::<INT>(engine, scene, name);
    register_setters::<rhai::FLOAT>(engine, scene, name);
    if list {
        let s = scene.clone();
        engine.register_fn(
            format!("clear_{name}"),
            move |handle: &mut Handle| -> Result<Handle> {
                change(&s, |scene| scene.clear_property(handle.0, name))?;
                Ok(*handle)
            },
        );
    }
}

/// Setting `name` from a `V`, which is shown as text for the property to parse.
fn register_setters<V: Any + Clone + ToString>(
    engine: &mut Engine,
    scene: &SceneRef,
    name: &'static str,
) {
    let s = scene.clone();
    engine.register_set(name, move |handle: &mut Handle, value: V| {
        change(&s, |scene| {
            scene.set_property(handle.0, name, &value.to_string())
        })
    });
    let s = scene.clone();
    engine.register_fn(
        name,
        move |handle: &mut Handle, value: V| -> Result<Handle> {
            change(&s, |scene| {
                scene.set_property(handle.0, name, &value.to_string())
            })?;
            Ok(*handle)
        },
    );
}

/// `e.name()`, which applies one of Freya's shorthand setters and gives `e` back.
fn register_shorthand(engine: &mut Engine, scene: &SceneRef, name: &'static str) {
    let s = scene.clone();
    engine.register_fn(name, move |handle: &mut Handle| -> Result<Handle> {
        change(&s, |scene| scene.apply_shorthand(handle.0, name))?;
        Ok(*handle)
    });
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
    use std::cell::RefCell;
    use std::rc::Rc;

    use rhai::Scope;

    use super::*;
    use crate::scene::Kind;

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

        /// Runs `code`, which must give a string, and returns it.
        fn text(&mut self, code: &str) -> String {
            self.eval::<String>(code)
                .unwrap_or_else(|error| panic!("{code}: {error}"))
        }

        /// Runs `code`, which must fail, and returns the error.
        fn fails(&mut self, code: &str) -> String {
            self.eval::<Dynamic>(code)
                .err()
                .unwrap_or_else(|| panic!("{code} succeeded"))
        }
    }

    #[test]
    fn adds_rects_paragraphs_and_spans() {
        let mut api = Api::new();
        api.run(
            r#"
            let header = rect();
            header.desc = "page header";
            let title = header.add_paragraph();
            title.add_span("Hello ");
            title.add_span("world");
            let row = header.add_rect();
            paragraph();
            "#,
        );
        let scene = api.scene.borrow();
        let [header, title, hello, world, row, after] = [1, 2, 3, 4, 5, 6].map(ElementId);
        assert_eq!(scene.children(ElementId::ROOT), Ok(&[header, after][..]));
        assert_eq!(scene.children(header), Ok(&[title, row][..]));
        assert_eq!(scene.children(title), Ok(&[hello, world][..]));
        assert_eq!(scene.get(row).map(|e| e.kind()), Some(Kind::Rect));
        assert_eq!(scene.description(header), Ok("page header"));
    }

    #[test]
    fn reads_back_what_was_added() {
        let mut api = Api::new();
        api.run(r#"let card = rect(); card.desc = "box"; let p = card.add_paragraph(); let s = p.add_span("Hi");"#);
        assert_eq!(api.text("s.kind"), "span");
        assert_eq!(api.text("el(1).kind"), "rect");
        assert_eq!(api.eval::<INT>("s.id"), Ok(3));
        assert_eq!(api.text("s.parent.parent.desc"), "box");
        assert_eq!(
            api.eval::<Handle>("card.parent"),
            Ok(Handle(ElementId::ROOT))
        );
        assert_eq!(api.eval::<()>("root().parent"), Ok(()));
        assert_eq!(
            api.eval::<Handle>("p.children[0]"),
            Ok(Handle(ElementId(3)))
        );
        assert_eq!(
            api.eval::<Handle>("root().children[0]"),
            Ok(Handle(ElementId(1)))
        );
        assert_eq!(api.text("s.text"), "Hi");
    }

    #[test]
    fn properties_are_set_from_text_or_numbers_and_read_as_text() {
        let mut api = Api::new();
        api.run(
            r#"
            let card = rect();
            card.width = "fill";
            card.padding = 8;
            card.opacity = 0.5;
            card.background = "rgb(30, 30, 46)";
            "#,
        );
        assert_eq!(api.text("card.width"), "fill");
        assert_eq!(api.text("card.padding"), "8");
        assert_eq!(api.text("card.opacity"), "0.5");
        assert_eq!(api.text("card.background"), "rgb(30, 30, 46)");
        assert_eq!(api.text("card.height"), "auto");
    }

    #[test]
    fn setters_chain_like_freyas_builder() {
        let mut api = Api::new();
        api.run(
            r#"let card = rect().width("px(300)").padding("(12, 16)").expanded().rounded_lg();"#,
        );
        assert_eq!(api.text("card.width"), "fill");
        assert_eq!(api.text("card.padding"), "(12, 16)");
        assert_eq!(api.text("card.corner_radius"), "8");
        let span = api.text(r#"paragraph().add_span("x").color("red").font_size(24).text"#);
        assert_eq!(span, "x");
    }

    #[test]
    fn list_properties_add_until_cleared() {
        let mut api = Api::new();
        api.run(r#"let card = rect(); card.shadow = "y(2) blur(4)"; card.shadow("y(8) blur(16) color(black)");"#);
        assert_eq!(api.eval::<INT>("card.shadow.len()"), Ok(2));
        assert_eq!(
            api.text("card.shadow[1]"),
            "x(0) y(8) blur(16) spread(0) color(black)"
        );
        api.run("card.clear_shadow()");
        assert_eq!(api.eval::<INT>("card.shadow.len()"), Ok(0));
    }

    #[test]
    fn bad_values_and_properties_fail_with_why() {
        let mut api = Api::new();
        api.run(r#"let card = rect(); let s = card.add_paragraph().add_span("x");"#);
        assert!(
            api.fails(r#"card.width = "huge""#)
                .contains("width: expected a size like")
        );
        assert!(
            api.fails(r#"s.width = "fill""#)
                .contains("element 3 is a span, which has no width")
        );
        assert!(
            api.fails("s.background")
                .contains("which has no background")
        );
        assert!(api.fails("card.nonsense").contains("nonsense"));
    }

    #[test]
    fn elements_go_only_where_they_fit() {
        let mut api = Api::new();
        api.run(r#"let card = rect(); let p = card.add_paragraph(); let s = p.add_span("x");"#);
        assert!(
            api.fails(r#"card.add_span("x")"#)
                .contains("spans go in a paragraph")
        );
        assert!(api.fails("p.add_rect()").contains("holds only spans"));
        assert!(
            api.fails("s.add_paragraph()")
                .contains("can't hold anything")
        );
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
    fn remove_takes_children_and_their_handles_then_fail() {
        let mut api = Api::new();
        api.run(
            r#"let outer = rect(); let p = outer.add_paragraph(); let leaf = p.add_span("x");"#,
        );
        api.run("outer.remove()");
        assert!(
            api.scene
                .borrow()
                .children(ElementId::ROOT)
                .unwrap()
                .is_empty()
        );
        for (code, id) in [
            ("leaf.text", 3),
            ("leaf.color = \"red\"", 3),
            ("leaf.color(\"red\")", 3),
            ("p.id", 2),
            ("p.kind", 2),
            ("outer.desc", 1),
            ("outer.expanded()", 1),
            ("p.add_span(\"y\")", 2),
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
        assert!(
            api.fails("root().remove()")
                .contains("the root can't be removed")
        );
    }

    #[test]
    fn clear_empties_the_root() {
        let mut api = Api::new();
        api.run(r#"rect(); paragraph(); clear();"#);
        assert!(
            api.scene
                .borrow()
                .children(ElementId::ROOT)
                .unwrap()
                .is_empty()
        );
        assert_eq!(api.eval::<INT>("root().children.len()"), Ok(0));
    }

    #[test]
    fn elements_show_their_kind_and_text() {
        let mut api = Api::new();
        api.run(r#"let s = paragraph().add_span("Hi"); s.desc = "greeting";"#);
        let shown = r#"#2 span "Hi" (greeting)"#;
        assert_eq!(api.text("s.to_string()"), shown);
        assert_eq!(api.text("`${s}`"), shown);
        assert_eq!(api.text("s.to_debug()"), shown);
    }
}
