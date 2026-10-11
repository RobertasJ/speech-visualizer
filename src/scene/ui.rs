mod display;
mod window;

use freya::prelude::*;

use super::Scene;
use super::script::Store;

/// Creates a scene owned by the calling component, and shows it in a window of its own
/// that closes when the component unmounts.
pub fn use_scene() -> State<Scene> {
    let scene = use_state(Scene::default);
    use_state(|| display::spawn(scene));
    scene
}

/// Scripts change a component's scene through its state, so what shows it redraws.
/// Reading through it counts as a change too, since a store only lends the scene
/// mutably.
impl Store for State<Scene> {
    fn with<R>(&self, f: impl FnOnce(&mut Scene) -> R) -> R {
        let mut scene = *self;
        f(&mut scene.write())
    }
}

#[cfg(test)]
mod tests {
    use freya_testing::TestingRunner;

    use super::*;
    use crate::scene::Script;

    fn app() -> impl IntoElement {
        rect()
    }

    #[test]
    fn scripts_change_a_scene_kept_in_state() {
        let (test, scene) = TestingRunner::new(
            app,
            (100., 100.).into(),
            |runner| runner.provide_root_context(|| State::create(Scene::default())),
            1.,
        );
        let outcome = test.run_in(|| Script::new(scene).run(r#"rect().add_text("hi").id"#));
        assert_eq!(outcome.output, ["2"]);
        assert_eq!(scene.peek().roots().len(), 1);
    }
}
