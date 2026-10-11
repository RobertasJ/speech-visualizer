//! Where an element is placed: in the flow with its siblings (stacked), or taken out of
//! it and placed relative to its parent (absolute) or to the window (global), by the
//! sides given.

use freya::prelude::Position;
use freya::torin::position::PositionSides;

use super::number;
use super::syntax::{self, Term};

pub const SYNTAX: &str =
    "stacked | absolute | global, then any of top(N) right(N) bottom(N) left(N)";

pub fn parse(input: &str) -> Result<Position, String> {
    let terms = syntax::terms(input)?;
    let Some((first, sides)) = terms.split_first() else {
        return Err("expected a position".to_owned());
    };
    let mut position = match first.name() {
        Some("stacked" | "new_stacked") => Position::new_stacked(),
        Some("absolute" | "new_absolute") => Position::new_absolute(),
        Some("global" | "new_global") => Position::new_global(),
        _ => return Err(format!("expected {SYNTAX}, not `{first}`")),
    };
    for side in sides {
        let Term::Call(name, args) = side else {
            return Err(format!("expected a side like top(10), not `{side}`"));
        };
        let value = syntax::numbers(args, 1)?[0];
        position = match name.as_str() {
            "top" => position.top(value),
            "right" => position.right(value),
            "bottom" => position.bottom(value),
            "left" => position.left(value),
            _ => return Err(format!("expected top, right, bottom or left, not `{name}`")),
        };
    }
    Ok(position)
}

pub fn format(position: &Position) -> String {
    let (name, sides) = match position {
        Position::Stacked(sides) => ("stacked", sides),
        Position::Absolute(sides) => ("absolute", sides),
        Position::Global(sides) => ("global", sides),
    };
    let PositionSides {
        top,
        right,
        bottom,
        left,
    } = sides.as_ref();
    let mut shown = name.to_owned();
    for (side, value) in [
        ("top", top),
        ("right", right),
        ("bottom", bottom),
        ("left", left),
    ] {
        if let Some(value) = value {
            shown.push_str(&format!(" {side}({})", number::format(*value)));
        }
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_read_back_as_written() {
        for written in [
            "stacked",
            "absolute top(10) left(5)",
            "global right(-2.5) bottom(0)",
        ] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
    }

    #[test]
    fn freyas_constructor_chain_reads_too() {
        assert_eq!(
            parse("new_absolute().top(10).left(5)"),
            Ok(Position::new_absolute().top(10.).left(5.))
        );
    }

    #[test]
    fn bad_positions() {
        assert!(parse("").is_err());
        assert!(parse("floating").is_err());
        assert!(
            parse("absolute middle(3)")
                .unwrap_err()
                .contains("expected top")
        );
        assert!(parse("absolute 3").unwrap_err().contains("expected a side"));
    }
}
