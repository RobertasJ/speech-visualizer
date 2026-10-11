//! A paragraph: a block of text made of spans, wrapped to its width, like Freya's
//! `paragraph()`. Kept as Freya's own `ParagraphElement`; its spans are elements of
//! their own in the scene, put in it when it's drawn.

use std::borrow::Cow;

use freya::elements::paragraph::ParagraphElement;
use freya::prelude::EffectData;

use super::style::{Group, Parts, Reach, effects, layout, optional, text};
use super::value::{effect, number, text as text_value};

/// Its own properties, beyond the groups it shares.
pub const OWN: Group<ParagraphElement> = Group {
    name: "paragraph",
    description: "A block of text made of spans, which it wraps to its width.",
    properties: &[
        one!(
            "max_lines",
            number::OPTIONAL_SYNTAX,
            "Cuts it off after N lines, ending with its text_overflow.",
            |paragraph| paragraph.max_lines,
            number::parse_whole_optional::<usize>,
            |lines: &Option<usize>| optional(lines.as_ref(), ToString::to_string, "none"),
        ),
        one!(
            "line_height",
            number::OPTIONAL_SYNTAX,
            "The height of each line, as a multiple of the font size.",
            |paragraph| paragraph.line_height,
            number::parse_optional,
            |height: &Option<f32>| number::format_optional(*height),
        ),
        one!(
            "vertical_align",
            text_value::VERTICAL_ALIGN_SYNTAX,
            "Where its text sits when it's taller than the text.",
            |paragraph| paragraph.vertical_align,
            text_value::parse_vertical_align,
            text_value::format_vertical_align,
        ),
        one!(
            "layer",
            effect::LAYER_SYNTAX,
            "Draws it in front of or behind its siblings: higher is in front.",
            |paragraph| paragraph.relative_layer,
            effect::parse_layer,
            effect::format_layer,
        ),
    ],
    shorthands: &[],
};

pub const PARTS: Parts<ParagraphElement> = Parts(&[
    &Reach::<ParagraphElement, _> {
        group: &layout::GROUP,
        read: |paragraph| Cow::Borrowed(&paragraph.layout.layout),
        write: |paragraph| &mut paragraph.layout.layout,
    },
    &Reach::<ParagraphElement, _> {
        group: &effects::GROUP,
        read: |paragraph| {
            paragraph
                .effect
                .as_ref()
                .map_or_else(|| Cow::Owned(EffectData::default()), Cow::Borrowed)
        },
        write: |paragraph| paragraph.effect.get_or_insert_with(EffectData::default),
    },
    &Reach::<ParagraphElement, _> {
        group: &text::GROUP,
        read: |paragraph| Cow::Borrowed(&paragraph.text_style_data),
        write: |paragraph| &mut paragraph.text_style_data,
    },
    &Reach::<ParagraphElement, _> {
        group: &OWN,
        read: |paragraph| Cow::Borrowed(paragraph),
        write: |paragraph| paragraph,
    },
]);

/// A new paragraph, as `paragraph()` makes it.
pub fn new() -> ParagraphElement {
    ParagraphElement::default()
}
