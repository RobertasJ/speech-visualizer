//! Text style, which passes down: a span takes what its paragraph doesn't set, and a
//! paragraph what its rects don't. Kept in its text style data, as Freya's `color()`,
//! `font_size()`, `font_family()` and so on set it.

use freya::prelude::{FontFeature, LetterSpacing, TextStyleData};

use super::{Access, Group, Property, Shorthand, list, optional, parse_optional, text_value};
use crate::scene::value::{color, font, number, text};

/// A text property, which `inherit` leaves to the parent: none in Freya.
macro_rules! inherited {
    ($name:literal, $syntax:expr, $description:literal, $field:ident, $parse:expr, $format:expr $(,)?) => {
        one!(
            $name,
            $syntax,
            $description,
            |style| style.$field,
            |input| parse_optional(input, "inherit", $parse),
            |value: &Option<_>| optional(value.as_ref(), $format, "inherit"),
        )
    };
}

/// Freya's setters that add a font feature.
macro_rules! features {
    ($($name:literal => $feature:ident),* $(,)?) => {
        &[$(Shorthand {
            name: $name,
            description: concat!("Adds the ", stringify!($feature), " font feature."),
            apply: |style| style.font_features.push(FontFeature::$feature),
        }),*]
    };
}

fn parse_letter_spacing(input: &str) -> Result<LetterSpacing, String> {
    number::parse(input).map(LetterSpacing::from)
}

fn format_letter_spacing(spacing: &LetterSpacing) -> String {
    number::format(f32::from(*spacing))
}

pub const GROUP: Group<TextStyleData> = Group {
    name: "text",
    description: "How its text looks. Each passes down to what's inside it, and `inherit`, the \
                  default, takes its parent's.",
    properties: &[
        inherited!(
            "color",
            color::SYNTAX,
            "The text's color.",
            color,
            color::parse_fill,
            color::format_fill
        ),
        inherited!(
            "font_size",
            font::SIZE_SYNTAX,
            "How big the text is.",
            font_size,
            font::parse_size,
            font::format_size,
        ),
        Property {
            name: "font_family",
            syntax: "a font's name, like Inter",
            description: "Adds a font to use, the first that has a character winning.",
            access: Access::List {
                get: |style| list(&style.font_families, ToString::to_string),
                add: |style, value| {
                    style.font_families.push(text_value(value));
                    Ok(())
                },
                clear: |style| style.font_families.clear(),
            },
        },
        inherited!(
            "font_weight",
            font::WEIGHT_SYNTAX,
            "How bold the text is.",
            font_weight,
            font::parse_weight,
            font::format_weight,
        ),
        inherited!(
            "font_slant",
            font::SLANT_SYNTAX,
            "Whether the text is upright or slanted.",
            font_slant,
            font::parse_slant,
            font::format_slant,
        ),
        inherited!(
            "font_width",
            font::WIDTH_SYNTAX,
            "How narrow or wide the letters are.",
            font_width,
            font::parse_width,
            font::format_width,
        ),
        inherited!(
            "text_align",
            text::ALIGN_SYNTAX,
            "How lines of text line up.",
            text_align,
            text::parse_align,
            text::format_align,
        ),
        inherited!(
            "text_overflow",
            text::OVERFLOW_SYNTAX,
            "What shows where text is cut off, as by max_lines.",
            text_overflow,
            text::parse_overflow,
            text::format_overflow,
        ),
        inherited!(
            "text_decoration",
            text::DECORATION_SYNTAX,
            "A line under, over or through the text.",
            text_decoration,
            text::parse_decoration,
            text::format_decoration,
        ),
        inherited!(
            "text_height",
            text::HEIGHT_SYNTAX,
            "Whether the first line's ascent and the last's descent count towards its height.",
            text_height,
            text::parse_height,
            text::format_height,
        ),
        inherited!(
            "letter_spacing",
            number::SYNTAX,
            "Extra space between letters.",
            letter_spacing,
            parse_letter_spacing,
            format_letter_spacing,
        ),
        Property {
            name: "text_shadow",
            syntax: text::SHADOW_SYNTAX,
            description: "Adds a shadow behind the text; it can have several.",
            access: Access::List {
                get: |style| list(&style.text_shadows, text::format_shadow),
                add: |style, value| {
                    style.text_shadows.push(text::parse_shadow(value)?);
                    Ok(())
                },
                clear: |style| style.text_shadows.clear(),
            },
        },
        Property {
            name: "font_feature",
            syntax: font::FEATURE_SYNTAX,
            description: "Adds an OpenType font feature.",
            access: Access::List {
                get: |style| list(&style.font_features, font::format_feature),
                add: |style, value| {
                    style.font_features.push(font::parse_feature(value)?);
                    Ok(())
                },
                clear: |style| style.font_features.clear(),
            },
        },
    ],
    shorthands: features![
        "font_tabular" => TABULAR_NUMBERS,
        "font_small_caps" => SMALL_CAPS,
        "font_slashed_zero" => SLASHED_ZERO,
        "font_fractions" => FRACTIONS,
        "font_no_ligatures" => NO_LIGATURES,
    ],
};
