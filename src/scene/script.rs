//! Rhai scripts that change a [`Scene`], as the agent and the console run them.
//!
//! A [`Script`] keeps its variables from one run to the next. A run that fails is
//! undone, scene and variables both. Functions defined with `fn` only last for the run
//! that defines them.

mod api;
mod show;
mod table;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rhai::{Dynamic, Engine, Scope};

use super::Scene;

pub use table::FUNCTIONS;

/// Somewhere a scene is kept, which scripts change through [`with`](Self::with).
pub trait Store: Clone + 'static {
    /// Calls `f` with the scene.
    fn with<R>(&self, f: impl FnOnce(&mut Scene) -> R) -> R;
}

/// What came of running a script.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    /// Every line it printed, then its result if it isn't `()`.
    pub output: Vec<String>,
    /// Why it failed, with the line and position. It was undone if so.
    pub error: Option<String>,
}

impl Outcome {
    /// What it printed, then the error if it failed, or just `ok` if neither.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = self.output.clone();
        match &self.error {
            Some(error) => lines.push(format!("error: {error}")),
            None if lines.is_empty() => lines.push("ok".to_owned()),
            None => {}
        }
        lines
    }
}

/// Runs scripts on a scene, keeping their variables from one run to the next. Keep it
/// for as long as the scene.
pub struct Script {
    engine: Engine,
    scope: Scope<'static>,
    scene: SceneRef,
    /// What the running script has printed so far.
    printed: Rc<RefCell<Vec<String>>>,
}

impl Script {
    pub fn new(store: impl Store) -> Self {
        let scene = SceneRef::new(store);
        let printed = Rc::new(RefCell::new(Vec::new()));

        let mut engine = Engine::new();
        // Scripts run on the UI thread, so a runaway one mustn't hang it.
        engine
            .set_max_operations(100_000)
            .set_max_string_size(10_000)
            .set_max_array_size(1_000)
            .disable_symbol("eval");
        let lines = printed.clone();
        engine.on_print(move |text| lines.borrow_mut().extend(text.split('\n').map(Into::into)));
        let lines = printed.clone();
        engine.on_debug(move |text, _, _| {
            lines.borrow_mut().extend(text.split('\n').map(Into::into));
        });
        api::register(&mut engine, &scene);

        Script {
            engine,
            scope: Scope::new(),
            scene,
            printed,
        }
    }

    /// Runs `code`. If it fails, the scene and the variables are put back as they were,
    /// but what it printed before failing stays in the outcome.
    pub fn run(&mut self, code: &str) -> Outcome {
        let scene = self.scene.with(|scene| scene.clone());
        let scope = deep_copy(&self.scope);

        let result = self
            .engine
            .eval_with_scope::<Dynamic>(&mut self.scope, code);
        let mut output = std::mem::take(&mut *self.printed.borrow_mut());
        let error = match result {
            Ok(value) => {
                if !value.is_unit() {
                    output.push(show::value(&self.scene, &value));
                }
                None
            }
            Err(err) => {
                self.scene.with(|current| *current = scene);
                self.scope = scope;
                Some(err.to_string())
            }
        };
        Outcome { output, error }
    }

    /// The variables scripts can see, oldest first, each with its value as text. Of
    /// several with the same name, only the latest is visible.
    pub fn variables(&self) -> Vec<(String, String)> {
        let mut seen = HashSet::new();
        // Newest first, so the first of each name is the visible one.
        let mut variables: Vec<_> = self
            .scope
            .iter_raw()
            .filter(|&(name, ..)| seen.insert(name))
            .map(|(name, _, value)| (name.to_owned(), show::value(&self.scene, value)))
            .collect();
        variables.reverse();
        variables
    }
}

/// A copy of `scope` that shares no values with it. A plain clone would share any
/// variable a closure has captured, so a failed run's changes to it couldn't be undone.
fn deep_copy(scope: &Scope<'static>) -> Scope<'static> {
    let mut copy = Scope::new();
    // `iter` gives each value as an unshared copy, oldest first.
    for (name, constant, value) in scope.iter() {
        if constant {
            copy.push_constant_dynamic(name, value);
        } else {
            copy.push_dynamic(name, value);
        }
    }
    copy
}

/// For tests, anywhere in the crate: a scene shared between a script and the test.
#[cfg(test)]
impl Store for Rc<RefCell<Scene>> {
    fn with<R>(&self, f: impl FnOnce(&mut Scene) -> R) -> R {
        f(&mut self.borrow_mut())
    }
}

/// The scene a script runs on, whatever [`Store`] it's in, so the engine's functions
/// needn't be generic over it.
#[derive(Clone)]
struct SceneRef(Rc<WithScene>);

/// Calls what it's given with the scene: [`Store::with`], as a trait object.
type WithScene = dyn Fn(&mut dyn FnMut(&mut Scene));

impl SceneRef {
    fn new(store: impl Store) -> Self {
        SceneRef(Rc::new(move |f: &mut dyn FnMut(&mut Scene)| store.with(f)))
    }

    /// Calls `f` with the scene.
    fn with<R>(&self, f: impl FnOnce(&mut Scene) -> R) -> R {
        let mut f = Some(f);
        let mut result = None;
        (self.0)(&mut |scene: &mut Scene| result = f.take().map(|f| f(scene)));
        result.expect("a store calls what it's given")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn script() -> (Script, Rc<RefCell<Scene>>) {
        let scene = Rc::new(RefCell::new(Scene::default()));
        (Script::new(scene.clone()), scene)
    }

    /// Runs `code`, which must succeed, and returns its output.
    fn ok(script: &mut Script, code: &str) -> Vec<String> {
        let outcome = script.run(code);
        assert_eq!(outcome.error, None, "{code}");
        outcome.output
    }

    /// Runs `code`, which must fail, and returns the error.
    fn fails(script: &mut Script, code: &str) -> String {
        script
            .run(code)
            .error
            .unwrap_or_else(|| panic!("{code} succeeded"))
    }

    #[test]
    fn output_is_what_was_printed_then_the_result() {
        let (mut script, _) = script();
        assert_eq!(
            ok(&mut script, r#"print("a"); debug(1); print("b\nc"); 5"#),
            ["a", "1", "b", "c", "5"]
        );
    }

    #[test]
    fn printing_an_element_shows_it_or_throws_if_it_was_removed() {
        let (mut script, _) = script();
        assert_eq!(
            ok(&mut script, r#"let t = text("Hi"); print(t); debug(t);"#),
            [r#"#1 text "Hi""#, r#"#1 text "Hi""#]
        );
        ok(&mut script, "t.remove();");
        let outcome = script.run(r#"print("before"); print(t)"#);
        assert_eq!(outcome.output, ["before"]);
        let error = outcome.error.unwrap();
        assert!(error.contains("no element 1"), "{error}");
    }

    #[test]
    fn a_failing_script_is_undone_but_keeps_what_it_printed() {
        let (mut script, scene) = script();
        ok(&mut script, "let kept = rect(); let n = 1;");
        let before = scene.borrow().clone();

        let outcome = script
            .run(r#"n = 2; let extra = rect(); kept.desc = "changed"; print("before"); el(99);"#);
        assert_eq!(outcome.output, ["before"]);
        assert!(outcome.error.is_some());

        assert_eq!(*scene.borrow(), before);
        assert_eq!(ok(&mut script, "n"), ["1"]);
        assert!(fails(&mut script, "extra").contains("Variable not found"));
    }

    #[test]
    fn undo_reaches_variables_a_closure_captured() {
        let (mut script, _) = script();
        ok(&mut script, "let v = [1]; let f = || v.len(); f.call();");
        fails(&mut script, r#"v.push(2); throw "fail""#);
        assert_eq!(ok(&mut script, "v"), ["[1]"]);
    }

    #[test]
    fn variables_persist_across_runs() {
        let (mut script, _) = script();
        ok(&mut script, "let x = 1;");
        ok(&mut script, "x = x + 1;");
        assert_eq!(ok(&mut script, "x"), ["2"]);
        ok(&mut script, r#"let x = "replaced";"#);
        assert_eq!(ok(&mut script, "x"), [r#""replaced""#]);
        assert!(fails(&mut script, "y = 1").contains("Variable not found: y"));
    }

    #[test]
    fn variables_shows_only_the_visible_value_of_a_name() {
        let (mut script, _) = script();
        ok(&mut script, r#"let a = 1; let b = rect(); b.desc = "box";"#);
        // A `let` in a later run shadows `a`; within one run it would replace it.
        ok(&mut script, r#"let a = "two"; let c = [b, ()];"#);
        let variables = [
            ("b", "#1 rect (box)"),
            ("a", r#""two""#),
            ("c", "[#1 rect (box), ()]"),
        ]
        .map(|(name, value)| (name.to_owned(), value.to_owned()));
        assert_eq!(script.variables(), variables);

        ok(&mut script, "b.remove();");
        assert_eq!(script.variables()[0].1, "#1 (removed)");
    }

    #[test]
    fn functions_only_last_for_their_run() {
        let (mut script, _) = script();
        assert_eq!(ok(&mut script, "fn double(x) { x * 2 } double(2)"), ["4"]);
        assert!(fails(&mut script, "double(3)").contains("Function not found"));
        assert_eq!(ok(&mut script, "fn double(x) { x * 3 } double(2)"), ["6"]);
    }

    #[test]
    fn descriptions_appear_in_the_scene_json() {
        let (mut script, scene) = script();
        ok(
            &mut script,
            r#"let r = rect(); r.desc = "page header"; r.add_text("Hi");"#,
        );
        assert_eq!(
            serde_json::to_value(&*scene.borrow()).unwrap(),
            json!({
                "next_id": 3,
                "elements": [
                    {"id": 1, "desc": "page header", "rect": [{"id": 2, "text": "Hi"}]},
                ],
            })
        );
    }

    #[test]
    fn a_long_loop_stops_at_the_operation_limit() {
        let (mut script, _) = script();
        let error = fails(&mut script, "let n = 0; loop { n += 1; }");
        assert!(error.contains("Too many operations"), "{error}");
    }

    #[test]
    fn eval_is_disabled() {
        let (mut script, _) = script();
        assert!(fails(&mut script, r#"eval("1")"#).contains("'eval' is disabled"));
    }

    #[test]
    fn outcome_lines() {
        let outcome = |output: &[&str], error: Option<&str>| Outcome {
            output: output.iter().map(|&line| line.to_owned()).collect(),
            error: error.map(Into::into),
        };
        assert_eq!(outcome(&[], None).lines(), ["ok"]);
        assert_eq!(outcome(&["a", "b"], None).lines(), ["a", "b"]);
        assert_eq!(outcome(&["a"], Some("bad")).lines(), ["a", "error: bad"]);
    }
}
