//! A rect: a box that holds other elements and lays them out, like Freya's `rect()`.
//! Kept as Freya's own `RectElement`.

use std::borrow::Cow;

use freya::elements::rect::RectElement;
use freya::prelude::{EffectData, Size};

use super::style::{Group, Parts, Reach, container, decoration, effects, layout, text};
use super::value::effect;

/// Its own properties, beyond the groups it shares.
pub const OWN: Group<RectElement> = Group {
    name: "rect",
    description: "A box that holds rects and paragraphs, and lays them out.",
    properties: &[one!(
        "layer",
        effect::LAYER_SYNTAX,
        "Draws it in front of or behind its siblings: higher is in front.",
        |rect| rect.relative_layer,
        effect::parse_layer,
        effect::format_layer,
    )],
    shorthands: &[],
};

pub const PARTS: Parts<RectElement> = Parts(&[
    &Reach::<RectElement, _> {
        group: &layout::GROUP,
        read: |rect| Cow::Borrowed(&rect.layout.layout),
        write: |rect| &mut rect.layout.layout,
    },
    &Reach::<RectElement, _> {
        group: &container::GROUP,
        read: |rect| Cow::Borrowed(&rect.layout.layout),
        write: |rect| &mut rect.layout.layout,
    },
    &Reach::<RectElement, _> {
        group: &decoration::GROUP,
        read: |rect| Cow::Borrowed(&rect.style),
        write: |rect| &mut rect.style,
    },
    &Reach::<RectElement, _> {
        group: &effects::GROUP,
        read: |rect| {
            rect.effect
                .as_ref()
                .map_or_else(|| Cow::Owned(EffectData::default()), Cow::Borrowed)
        },
        write: |rect| rect.effect.get_or_insert_with(EffectData::default),
    },
    &Reach::<RectElement, _> {
        group: &text::GROUP,
        read: |rect| Cow::Borrowed(&rect.text_style_data),
        write: |rect| &mut rect.text_style_data,
    },
    &Reach::<RectElement, _> {
        group: &OWN,
        read: |rect| Cow::Borrowed(rect),
        write: |rect| rect,
    },
]);

/// A new rect, as `rect()` makes it.
pub fn new() -> RectElement {
    RectElement::default()
}

/// The window's rect, which fills it.
pub fn root() -> RectElement {
    let mut rect = new();
    rect.layout.layout.width = Size::fill();
    rect.layout.layout.height = Size::fill();
    rect
}
