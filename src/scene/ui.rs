mod display;
mod window;

use freya::prelude::*;

use super::Scene;

/// Creates a scene owned by the calling component, and shows it in a window of its own
/// that closes when the component unmounts.
pub fn use_scene() -> State<Scene> {
    let scene = use_state(Scene::default);
    use_state(|| display::spawn(scene));
    scene
}
