//! How text is aligned, cut off, decorated and shadowed.

use freya::prelude::{
    TextAlign, TextDecoration, TextHeightBehavior, TextOverflow, TextShadow, VerticalAlign,
};

use super::syntax::{self, Term};
use super::{color, number};

const ALIGNS: [(&str, TextAlign); 6] = [
    ("left", TextAlign::Left),
    ("right", TextAlign::Right),
    ("center", TextAlign::Center),
    ("justify", TextAlign::Justify),
    ("start", TextAlign::Start),
    ("end", TextAlign::End),
];

pub const ALIGN_SYNTAX: &str = "left | right | center | justify | start | end";

pub fn parse_align(input: &str) -> Result<TextAlign, String> {
    syntax::keyword(&syntax::one(input)?, &ALIGNS)
}

pub fn format_align(align: &TextAlign) -> String {
    syntax::keyword_name(align, &ALIGNS)
        .expect("every text align has a name")
        .to_owned()
}

pub const OVERFLOW_SYNTAX: &str = "clip | ellipsis | custom(\"…\")";

pub fn parse_overflow(input: &str) -> Result<TextOverflow, String> {
    let term = syntax::one(input)?;
    if let Some(args) = syntax::call(&term, "custom") {
        return match args {
            [Term::Text(ending)] => Ok(TextOverflow::Custom(ending.clone())),
            _ => Err(format!(
                "expected custom(\"…\") with text in quotes, not `{term}`"
            )),
        };
    }
    match term.name() {
        Some("clip") => Ok(TextOverflow::Clip),
        Some("ellipsis") => Ok(TextOverflow::Ellipsis),
        _ => Err(format!("expected {OVERFLOW_SYNTAX}, not `{term}`")),
    }
}

pub fn format_overflow(overflow: &TextOverflow) -> String {
    match overflow {
        TextOverflow::Clip => "clip".to_owned(),
        TextOverflow::Ellipsis => "ellipsis".to_owned(),
        TextOverflow::Custom(ending) => format!("custom({ending:?})"),
    }
}

const DECORATIONS: [(&str, TextDecoration); 4] = [
    ("none", TextDecoration::None),
    ("underline", TextDecoration::Underline),
    ("overline", TextDecoration::Overline),
    ("line_through", TextDecoration::LineThrough),
];

pub const DECORATION_SYNTAX: &str = "none | underline | overline | line_through";

pub fn parse_decoration(input: &str) -> Result<TextDecoration, String> {
    syntax::keyword(&syntax::one(input)?, &DECORATIONS)
}

pub fn format_decoration(decoration: &TextDecoration) -> String {
    syntax::keyword_name(decoration, &DECORATIONS)
        .expect("every text decoration has a name")
        .to_owned()
}

const HEIGHTS: [(&str, TextHeightBehavior); 4] = [
    ("all", TextHeightBehavior::All),
    (
        "disable_first_ascent",
        TextHeightBehavior::DisableFirstAscent,
    ),
    (
        "disable_last_descent",
        TextHeightBehavior::DisableLastDescent,
    ),
    ("disable_all", TextHeightBehavior::DisableAll),
];

pub const HEIGHT_SYNTAX: &str = "all | disable_first_ascent | disable_last_descent | disable_all";

pub fn parse_height(input: &str) -> Result<TextHeightBehavior, String> {
    syntax::keyword(&syntax::one(input)?, &HEIGHTS)
}

pub fn format_height(height: &TextHeightBehavior) -> String {
    syntax::keyword_name(height, &HEIGHTS)
        .expect("every text height has a name")
        .to_owned()
}

const VERTICAL_ALIGNS: [(&str, VerticalAlign); 2] = [
    ("start", VerticalAlign::Start),
    ("center", VerticalAlign::Center),
];

pub const VERTICAL_ALIGN_SYNTAX: &str = "start | center";

pub fn parse_vertical_align(input: &str) -> Result<VerticalAlign, String> {
    syntax::keyword(&syntax::one(input)?, &VERTICAL_ALIGNS)
}

pub fn format_vertical_align(align: &VerticalAlign) -> String {
    syntax::keyword_name(align, &VERTICAL_ALIGNS)
        .expect("every vertical align has a name")
        .to_owned()
}

pub const SHADOW_SYNTAX: &str = "color(color) offset(x, y) blur_sigma(N)";

/// Starts from a black shadow with no offset or blur, and applies each call in turn.
pub fn parse_shadow(input: &str) -> Result<TextShadow, String> {
    let mut shadow = TextShadow::new(freya::prelude::Color::BLACK, (0., 0.), 0.);
    for term in syntax::terms(input)? {
        let Term::Call(name, args) = &term else {
            return Err(format!("expected calls like {SHADOW_SYNTAX}, not `{term}`"));
        };
        match (name.as_str(), &args[..]) {
            ("color", [color]) => shadow.color = color::from_term(color)?,
            ("offset", [x, y]) => shadow.offset = (x.number()?, y.number()?),
            ("blur_sigma", [blur]) => shadow.blur_sigma = f64::from(blur.number()?),
            _ => return Err(format!("expected calls like {SHADOW_SYNTAX}, not `{term}`")),
        }
    }
    Ok(shadow)
}

pub fn format_shadow(shadow: &TextShadow) -> String {
    let (x, y) = shadow.offset;
    format!(
        "color({}) offset({}, {}) blur_sigma({})",
        color::format(&shadow.color),
        number::format(x),
        number::format(y),
        number::format(shadow.blur_sigma as f32)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_read_back() {
        assert_eq!(format_align(&parse_align("justify").unwrap()), "justify");
        assert_eq!(
            format_decoration(&parse_decoration("line_through").unwrap()),
            "line_through"
        );
        assert_eq!(format_height(&parse_height("all").unwrap()), "all");
        assert_eq!(
            format_vertical_align(&parse_vertical_align("center").unwrap()),
            "center"
        );
        assert!(parse_align("middle").is_err());
    }

    #[test]
    fn overflows() {
        for written in ["clip", "ellipsis", r#"custom("…")"#] {
            assert_eq!(format_overflow(&parse_overflow(written).unwrap()), written);
        }
        assert!(
            parse_overflow("custom(dots)")
                .unwrap_err()
                .contains("in quotes")
        );
    }

    #[test]
    fn text_shadows_read_back_as_written() {
        let written = "color(red) offset(1, 2) blur_sigma(3)";
        assert_eq!(format_shadow(&parse_shadow(written).unwrap()), written);
        assert_eq!(
            format_shadow(&parse_shadow("offset(1, 1)").unwrap()),
            "color(black) offset(1, 1) blur_sigma(0)"
        );
        assert!(parse_shadow("offset(1)").is_err());
    }
}
