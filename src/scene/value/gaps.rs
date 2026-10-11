//! Space on each side, for padding and margins.

use freya::prelude::Gaps;

use super::number;
use super::syntax::{self, Term};

pub const SYNTAX: &str = "N | (vertical, horizontal) | (top, right, bottom, left)";

pub fn parse(input: &str) -> Result<Gaps, String> {
    let term = syntax::one(input)?;
    match &term {
        Term::Number(all) => Ok(Gaps::new_all(*all)),
        Term::Tuple(items) => match syntax::numbers(items, items.len())?[..] {
            [all] => Ok(Gaps::new_all(all)),
            [vertical, horizontal] => Ok(Gaps::new_symmetric(vertical, horizontal)),
            [top, right, bottom, left] => Ok(Gaps::new(top, right, bottom, left)),
            _ => Err(format!("expected 1, 2 or 4 numbers, not `{term}`")),
        },
        _ => Err(format!("expected {SYNTAX}, not `{term}`")),
    }
}

/// The shortest way to write `gaps`.
pub fn format(gaps: &Gaps) -> String {
    let (top, right, bottom, left) = (gaps.top(), gaps.right(), gaps.bottom(), gaps.left());
    if top == right && top == bottom && top == left {
        number::format(top)
    } else if top == bottom && right == left {
        format!("({}, {})", number::format(top), number::format(right))
    } else {
        let sides: Vec<String> = [top, right, bottom, left].map(number::format).into();
        format!("({})", sides.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_read_back_in_their_shortest_form() {
        for written in ["8", "(8, 16)", "(1, 2, 3, 4)"] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
        assert_eq!(format(&parse("(4, 4, 4, 4)").unwrap()), "4");
        assert_eq!(format(&parse("(4, 8, 4, 8)").unwrap()), "(4, 8)");
    }

    #[test]
    fn bad_gaps() {
        assert!(parse("(1, 2, 3)").unwrap_err().contains("1, 2 or 4"));
        assert!(parse("wide").is_err());
    }
}
