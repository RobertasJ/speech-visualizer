//! Font weights, widths, slants, sizes and features.

use std::borrow::Cow;

use freya::prelude::{FontFeature, FontSize, FontSlant, FontWeight, FontWidth};

use super::number;
use super::syntax::{self, Term};

const WEIGHTS: [(&str, FontWeight); 11] = [
    ("invisible", FontWeight::INVISIBLE),
    ("thin", FontWeight::THIN),
    ("extra_light", FontWeight::EXTRA_LIGHT),
    ("light", FontWeight::LIGHT),
    ("normal", FontWeight::NORMAL),
    ("medium", FontWeight::MEDIUM),
    ("semi_bold", FontWeight::SEMI_BOLD),
    ("bold", FontWeight::BOLD),
    ("extra_bold", FontWeight::EXTRA_BOLD),
    ("black", FontWeight::BLACK),
    ("extra_black", FontWeight::EXTRA_BLACK),
];

pub const WEIGHT_SYNTAX: &str = "thin | extra_light | light | normal | medium | semi_bold | bold | extra_bold | black, or N from 0 to 1000";

pub fn parse_weight(input: &str) -> Result<FontWeight, String> {
    match syntax::one(input)? {
        Term::Number(weight) => number::whole::<i32>(weight).map(FontWeight::from),
        term => syntax::keyword(&term, &WEIGHTS),
    }
}

pub fn format_weight(weight: &FontWeight) -> String {
    syntax::keyword_name(weight, &WEIGHTS)
        .map_or_else(|| i32::from(*weight).to_string(), str::to_owned)
}

const WIDTHS: [(&str, FontWidth); 9] = [
    ("ultra_condensed", FontWidth::ULTRA_CONDENSED),
    ("extra_condensed", FontWidth::EXTRA_CONDENSED),
    ("condensed", FontWidth::CONDENSED),
    ("semi_condensed", FontWidth::SEMI_CONDENSED),
    ("normal", FontWidth::NORMAL),
    ("semi_expanded", FontWidth::SEMI_EXPANDED),
    ("expanded", FontWidth::EXPANDED),
    ("extra_expanded", FontWidth::EXTRA_EXPANDED),
    ("ultra_expanded", FontWidth::ULTRA_EXPANDED),
];

pub const WIDTH_SYNTAX: &str = "ultra_condensed | extra_condensed | condensed | semi_condensed | normal | semi_expanded | expanded | extra_expanded | ultra_expanded";

pub fn parse_width(input: &str) -> Result<FontWidth, String> {
    syntax::keyword(&syntax::one(input)?, &WIDTHS)
}

pub fn format_width(width: &FontWidth) -> String {
    syntax::keyword_name(width, &WIDTHS).map_or_else(|| width.get().to_string(), str::to_owned)
}

const SLANTS: [(&str, FontSlant); 3] = [
    ("upright", FontSlant::Upright),
    ("italic", FontSlant::Italic),
    ("oblique", FontSlant::Oblique),
];

pub const SLANT_SYNTAX: &str = "upright | italic | oblique";

pub fn parse_slant(input: &str) -> Result<FontSlant, String> {
    syntax::keyword(&syntax::one(input)?, &SLANTS)
}

pub fn format_slant(slant: &FontSlant) -> String {
    syntax::keyword_name(slant, &SLANTS)
        .expect("every slant has a name")
        .to_owned()
}

pub const SIZE_SYNTAX: &str = "N";

pub fn parse_size(input: &str) -> Result<FontSize, String> {
    number::parse(input).map(FontSize::from)
}

pub fn format_size(size: &FontSize) -> String {
    number::format(f32::from(*size))
}

/// The features Freya has names for.
const FEATURES: [(&str, FontFeature); 5] = [
    ("tabular_numbers", FontFeature::TABULAR_NUMBERS),
    ("small_caps", FontFeature::SMALL_CAPS),
    ("slashed_zero", FontFeature::SLASHED_ZERO),
    ("fractions", FontFeature::FRACTIONS),
    ("no_ligatures", FontFeature::NO_LIGATURES),
];

pub const FEATURE_SYNTAX: &str = "tabular_numbers | small_caps | slashed_zero | fractions | no_ligatures, or (\"tag\", N) for any OpenType feature";

pub fn parse_feature(input: &str) -> Result<FontFeature, String> {
    match syntax::one(input)? {
        Term::Tuple(items) => match &items[..] {
            [Term::Text(tag), value] => Ok(FontFeature {
                name: Cow::Owned(tag.clone()),
                value: number::whole::<i32>(value.number()?)?,
            }),
            _ => Err(format!(
                "expected (\"tag\", N), not `{}`",
                Term::Tuple(items)
            )),
        },
        term => syntax::keyword(&term, &FEATURES),
    }
}

pub fn format_feature(feature: &FontFeature) -> String {
    syntax::keyword_name(feature, &FEATURES).map_or_else(
        || format!("({:?}, {})", feature.name, feature.value),
        str::to_owned,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_by_name_or_number() {
        assert_eq!(
            format_weight(&parse_weight("semi_bold").unwrap()),
            "semi_bold"
        );
        assert_eq!(parse_weight("700"), Ok(FontWeight::BOLD));
        assert_eq!(format_weight(&parse_weight("450").unwrap()), "450");
        assert!(parse_weight("heavy").is_err());
        assert!(parse_weight("1.5").is_err());
    }

    #[test]
    fn widths_and_slants() {
        assert_eq!(
            format_width(&parse_width("condensed").unwrap()),
            "condensed"
        );
        assert_eq!(format_slant(&parse_slant("italic").unwrap()), "italic");
        assert!(parse_slant("bold").is_err());
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(&parse_size("24").unwrap()), "24");
        assert!(parse_size("big").is_err());
    }

    #[test]
    fn features_by_name_or_tag() {
        assert_eq!(
            parse_feature("tabular_numbers"),
            Ok(FontFeature::TABULAR_NUMBERS)
        );
        assert_eq!(format_feature(&FontFeature::SMALL_CAPS), "small_caps");
        let ligatures = parse_feature(r#"("dlig", 1)"#).unwrap();
        assert_eq!(format_feature(&ligatures), r#"("dlig", 1)"#);
        assert!(parse_feature("(dlig, 1)").is_err());
    }
}
