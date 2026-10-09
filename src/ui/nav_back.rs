use freya::prelude::*;

use crate::Screen;

#[derive(PartialEq)]
pub struct NavBack;

impl Component for NavBack {
    fn render(&self) -> impl IntoElement {
        let mut screen = use_consume::<State<Screen>>();

        Button::new()
            .on_press(move |_| screen.set(Screen::Selection))
            .child("← Back to selection")
    }
}
