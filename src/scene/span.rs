//! A span: a run of text in a paragraph, with its own text style, like Freya's `Span`.
//! Kept as Freya's own `Span`.

use std::borrow::Cow;

use freya::prelude::Span;

use super::style::{Access, Group, Parts, Property, Reach, text};

/// Its own properties, beyond the text style it shares.
pub const OWN: Group<Span<'static>> = Group {
    name: "span",
    description: "A run of text in a paragraph, with its own text style.",
    properties: &[Property {
        name: "text",
        syntax: "any text",
        description: "Its text, as given.",
        access: Access::One {
            get: |span| span.text.to_string(),
            set: |span, value| {
                span.text = Cow::Owned(value.to_owned());
                Ok(())
            },
        },
    }],
    shorthands: &[],
};

pub const PARTS: Parts<Span<'static>> = Parts(&[
    &Reach::<Span<'static>, _> {
        group: &OWN,
        read: |span| Cow::Borrowed(span),
        write: |span| span,
    },
    &Reach::<Span<'static>, _> {
        group: &text::GROUP,
        read: |span| Cow::Borrowed(&span.text_style_data),
        write: |span| &mut span.text_style_data,
    },
]);

/// A new span with `text`, as `Span::new()` makes it.
pub fn new(text: String) -> Span<'static> {
    Span::new(text)
}
