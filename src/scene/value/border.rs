//! A border, written as Freya's builder calls: `width(2) fill(red) alignment(outer)`.

use freya::prelude::{Border, BorderAlignment, BorderWidth};

use super::syntax::{self, Term};
use super::{color, number};

const ALIGNMENTS: [(&str, BorderAlignment); 3] = [
    ("inner", BorderAlignment::Inner),
    ("outer", BorderAlignment::Outer),
    ("center", BorderAlignment::Center),
];

pub const SYNTAX: &str =
    "width(N) or width(top, right, bottom, left), fill(color), alignment(inner | outer | center)";

/// Starts from `Border::new()` and applies each call in turn.
pub fn parse(input: &str) -> Result<Border, String> {
    let mut border = Border::new();
    for term in syntax::terms(input)? {
        let Term::Call(name, args) = &term else {
            return Err(format!("expected calls like {SYNTAX}, not `{term}`"));
        };
        border = match (name.as_str(), &args[..]) {
            ("width", [_]) => border.width(syntax::numbers(args, 1)?[0]),
            ("width", [_, _, _, _]) => {
                let [top, right, bottom, left] = syntax::numbers(args, 4)?[..] else {
                    unreachable!("`numbers` gives as many as asked for")
                };
                border.width(BorderWidth {
                    top,
                    right,
                    bottom,
                    left,
                })
            }
            ("fill", [fill]) => border.fill(color::from_term(fill)?),
            ("alignment", [alignment]) => {
                border.alignment(syntax::keyword(alignment, &ALIGNMENTS)?)
            }
            _ => return Err(format!("expected calls like {SYNTAX}, not `{term}`")),
        };
    }
    Ok(border)
}

pub fn format(border: &Border) -> String {
    let BorderWidth {
        top,
        right,
        bottom,
        left,
    } = border.width;
    let width = if top == right && top == bottom && top == left {
        number::format(top)
    } else {
        let sides: Vec<String> = [top, right, bottom, left].map(number::format).into();
        sides.join(", ")
    };
    let mut shown = format!("width({width}) fill({})", color::format(&border.fill));
    if border.alignment != BorderAlignment::Inner {
        let alignment = syntax::keyword_name(&border.alignment, &ALIGNMENTS)
            .expect("every border alignment has a name");
        shown.push_str(&format!(" alignment({alignment})"));
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borders_read_back_as_written() {
        for written in [
            "width(2) fill(red)",
            "width(1, 2, 1, 2) fill(rgb(1, 2, 3)) alignment(outer)",
        ] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
    }

    #[test]
    fn calls_apply_in_order_from_a_new_border() {
        let border = parse("fill(blue) width(3) alignment(center)").unwrap();
        assert_eq!(
            border,
            Border::new()
                .fill(freya::prelude::Color::BLUE)
                .width(3.)
                .alignment(BorderAlignment::Center)
        );
        assert_eq!(parse("").unwrap(), Border::new());
    }

    #[test]
    fn bad_borders() {
        assert!(
            parse("thick")
                .unwrap_err()
                .starts_with("expected calls like")
        );
        assert!(parse("width(1, 2)").is_err());
        assert!(
            parse("alignment(middle)")
                .unwrap_err()
                .contains("inner, outer or center")
        );
        assert!(parse("fill(teal)").is_err());
    }
}
