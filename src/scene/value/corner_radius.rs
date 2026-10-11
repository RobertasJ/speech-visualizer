//! How round the corners are.

use freya::prelude::CornerRadius;

use super::number;
use super::syntax::{self, Term};

pub const SYNTAX: &str = "N | (top, bottom) | (top_left, top_right, bottom_right, bottom_left), then optionally with_smoothing(N)";

pub fn parse(input: &str) -> Result<CornerRadius, String> {
    let terms = syntax::terms(input)?;
    let (radius, rest) = match terms.split_first() {
        Some((Term::Number(all), rest)) => (CornerRadius::from(*all), rest),
        Some((Term::Tuple(items), rest)) => {
            let radius = match syntax::numbers(items, items.len())?[..] {
                [all] => CornerRadius::from(all),
                [top, bottom] => CornerRadius::from((top, bottom)),
                [top_left, top_right, bottom_right, bottom_left] => {
                    CornerRadius::from((top_left, top_right, bottom_right, bottom_left))
                }
                _ => return Err(format!("expected 1, 2 or 4 numbers, not {}", items.len())),
            };
            (radius, rest)
        }
        _ => return Err(format!("expected {SYNTAX}, not `{input}`")),
    };
    match rest {
        [] => Ok(radius),
        [smoothing] => match syntax::call(smoothing, "with_smoothing") {
            Some(args) => Ok(radius.with_smoothing(syntax::numbers(args, 1)?[0])),
            None => Err(format!("expected with_smoothing(N), not `{smoothing}`")),
        },
        _ => Err(format!("expected {SYNTAX}, not `{input}`")),
    }
}

/// The shortest way to write `radius`.
pub fn format(radius: &CornerRadius) -> String {
    let corners = [
        radius.top_left(),
        radius.top_right(),
        radius.bottom_right(),
        radius.bottom_left(),
    ];
    let [top_left, top_right, bottom_right, bottom_left] = corners;
    let mut shown = if corners.iter().all(|&corner| corner == top_left) {
        number::format(top_left)
    } else if top_left == top_right && bottom_left == bottom_right {
        format!(
            "({}, {})",
            number::format(top_left),
            number::format(bottom_left)
        )
    } else {
        let corners: Vec<String> = corners.map(number::format).into();
        format!("({})", corners.join(", "))
    };
    if radius.smoothing() > 0. {
        shown.push_str(&format!(
            " with_smoothing({})",
            number::format(radius.smoothing())
        ));
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radii_read_back_in_their_shortest_form() {
        for written in ["8", "(8, 0)", "(1, 2, 3, 4)", "6 with_smoothing(0.6)"] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
        assert_eq!(format(&parse("(5, 5, 5, 5)").unwrap()), "5");
    }

    #[test]
    fn bad_radii() {
        assert!(parse("round").is_err());
        assert!(parse("(1, 2, 3)").is_err());
        assert!(parse("4 smooth(1)").unwrap_err().contains("with_smoothing"));
    }
}
