//! Colors, and fills, which are colors for now: Freya's gradients keep their stops
//! private, so they couldn't be read back.

use freya::prelude::{Color, Fill};

use super::syntax::{self, Term};

/// Freya's color constants, by their names in lowercase.
const NAMES: [(&str, Color); 15] = [
    ("transparent", Color::TRANSPARENT),
    ("black", Color::BLACK),
    ("dark_gray", Color::DARK_GRAY),
    ("gray", Color::GRAY),
    ("light_gray", Color::LIGHT_GRAY),
    ("dark_grey", Color::DARK_GREY),
    ("grey", Color::GREY),
    ("light_grey", Color::LIGHT_GREY),
    ("white", Color::WHITE),
    ("red", Color::RED),
    ("green", Color::GREEN),
    ("blue", Color::BLUE),
    ("yellow", Color::YELLOW),
    ("cyan", Color::CYAN),
    ("magenta", Color::MAGENTA),
];

pub const SYNTAX: &str = "red, gray, transparent… | #rrggbb | #rrggbbaa | rgb(r, g, b) | argb(a, r, g, b) | hsv(h, s, v)";

pub fn parse(input: &str) -> Result<Color, String> {
    from_term(&syntax::one(input)?)
}

/// The color `term` is, for values that hold one, like `fill(red)`.
pub fn from_term(term: &Term) -> Result<Color, String> {
    let channel = |value: f32| -> Result<u8, String> {
        if value.fract() == 0. && (0. ..=255.).contains(&value) {
            Ok(value as u8)
        } else {
            Err(format!(
                "expected a whole number from 0 to 255, not {value}"
            ))
        }
    };
    match term {
        Term::Hex(hex) => Color::from_hex(hex)
            .ok_or_else(|| format!("expected #rrggbb or #rrggbbaa, not `{term}`")),
        Term::Call(name, args) => match name.as_str() {
            "rgb" | "from_rgb" => {
                let [r, g, b] = syntax::numbers(args, 3)?[..] else {
                    unreachable!("`numbers` gives as many as asked for")
                };
                Ok(Color::from_rgb(channel(r)?, channel(g)?, channel(b)?))
            }
            "argb" | "from_argb" => {
                let [a, r, g, b] = syntax::numbers(args, 4)?[..] else {
                    unreachable!("`numbers` gives as many as asked for")
                };
                Ok(Color::from_argb(
                    channel(a)?,
                    channel(r)?,
                    channel(g)?,
                    channel(b)?,
                ))
            }
            "hsv" | "from_hsv" => {
                let [h, s, v] = syntax::numbers(args, 3)?[..] else {
                    unreachable!("`numbers` gives as many as asked for")
                };
                Ok(Color::from_hsv(h, s, v))
            }
            _ => syntax::keyword(term, &NAMES).map_err(|_| unknown(term)),
        },
        _ => syntax::keyword(term, &NAMES).map_err(|_| unknown(term)),
    }
}

fn unknown(term: &Term) -> String {
    format!("expected a color like {SYNTAX}, not `{term}`")
}

/// Its name if it has one, otherwise `rgb(…)`, or `argb(…)` if it's see-through.
pub fn format(color: &Color) -> String {
    if let Some(name) = syntax::keyword_name(color, &NAMES) {
        return name.to_owned();
    }
    let (a, r, g, b) = (color.a(), color.r(), color.g(), color.b());
    if a == 255 {
        format!("rgb({r}, {g}, {b})")
    } else {
        format!("argb({a}, {r}, {g}, {b})")
    }
}

pub fn parse_fill(input: &str) -> Result<Fill, String> {
    parse(input).map(Fill::Color)
}

pub fn format_fill(fill: &Fill) -> String {
    match fill {
        Fill::Color(color) => format(color),
        // Only Rust code makes these.
        _ => "gradient".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_read_back_as_written() {
        for written in [
            "red",
            "transparent",
            "rgb(255, 136, 0)",
            "argb(128, 255, 136, 0)",
        ] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
    }

    #[test]
    fn hex_hsv_and_constructors_read_too() {
        assert_eq!(parse("#ff8800"), Ok(Color::from_rgb(255, 136, 0)));
        assert_eq!(parse("#ff880080"), Ok(Color::from_argb(128, 255, 136, 0)));
        assert_eq!(parse("from_rgb(1, 2, 3)"), Ok(Color::from_rgb(1, 2, 3)));
        assert_eq!(parse("hsv(0, 1, 1)"), Ok(Color::from_hsv(0., 1., 1.)));
        // The first of its names, for a color with two.
        assert_eq!(format(&parse("grey").unwrap()), "gray");
    }

    #[test]
    fn bad_colors() {
        assert!(
            parse("teal")
                .unwrap_err()
                .starts_with("expected a color like")
        );
        assert!(parse("#12345").is_err());
        assert!(parse("rgb(256, 0, 0)").unwrap_err().contains("0 to 255"));
        assert!(
            parse("rgb(1, 2)")
                .unwrap_err()
                .contains("expected 3 numbers")
        );
    }

    #[test]
    fn fills_are_colors() {
        assert_eq!(parse_fill("blue"), Ok(Fill::Color(Color::BLUE)));
        assert_eq!(format_fill(&Fill::Color(Color::WHITE)), "white");
    }
}
