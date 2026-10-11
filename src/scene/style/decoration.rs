//! How a rect looks: its background, borders, shadows and corners. Kept in its style,
//! as Freya's `background()`, `border()`, `shadow()` and `corner_radius()` set it.

use freya::prelude::{CornerRadius, StyleState};

use super::{Access, Group, Property, Shorthand, list};
use crate::scene::value::{border, color, corner_radius, shadow};

/// Freya's `rounded_*()` setters, and the radius each sets.
macro_rules! rounded {
    ($($name:literal => $radius:literal),* $(,)?) => {
        &[$(Shorthand {
            name: $name,
            description: concat!("Rounds its corners: corner_radius ", $radius, "."),
            apply: |style| style.corner_radius = CornerRadius::new_all($radius as f32),
        }),*]
    };
}

pub const GROUP: Group<StyleState> = Group {
    name: "decoration",
    description: "How it looks: what's behind it, and its borders, shadows and corners.",
    properties: &[
        one!(
            "background",
            color::SYNTAX,
            "What's drawn behind it.",
            |style| style.background,
            color::parse_fill,
            color::format_fill,
        ),
        Property {
            name: "border",
            syntax: border::SYNTAX,
            description: "Adds a border; it can have several.",
            access: Access::List {
                get: |style| list(&style.borders, border::format),
                add: |style, value| {
                    style.borders.push(border::parse(value)?);
                    Ok(())
                },
                clear: |style| style.borders.clear(),
            },
        },
        Property {
            name: "shadow",
            syntax: shadow::SYNTAX,
            description: "Adds a shadow; it can have several.",
            access: Access::List {
                get: |style| list(&style.shadows, shadow::format),
                add: |style, value| {
                    style.shadows.push(shadow::parse(value)?);
                    Ok(())
                },
                clear: |style| style.shadows.clear(),
            },
        },
        one!(
            "corner_radius",
            corner_radius::SYNTAX,
            "How round its corners are.",
            |style| style.corner_radius,
            corner_radius::parse,
            corner_radius::format,
        ),
    ],
    shorthands: rounded![
        "rounded_none" => 0,
        "rounded" => 6,
        "rounded_sm" => 4,
        "rounded_md" => 6,
        "rounded_lg" => 8,
        "rounded_xl" => 12,
        "rounded_2xl" => 16,
        "rounded_3xl" => 24,
        "rounded_4xl" => 32,
        "rounded_full" => 99,
    ],
};
