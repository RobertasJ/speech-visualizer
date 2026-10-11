//! A shadow, written as Freya's builder calls:
//! `x(0) y(4) blur(12) spread(0) color(black)`, and `inset()` for one inside.

use freya::prelude::Shadow;

use super::syntax::{self, Term};
use super::{color, number};

pub const SYNTAX: &str =
    "x(N) y(N) blur(N) spread(N) color(color), and inset() for a shadow inside";

/// Starts from `Shadow::new()` and applies each call in turn.
pub fn parse(input: &str) -> Result<Shadow, String> {
    let mut shadow = Shadow::new();
    for term in syntax::terms(input)? {
        let Term::Call(name, args) = &term else {
            return Err(format!("expected calls like {SYNTAX}, not `{term}`"));
        };
        shadow = match (name.as_str(), &args[..]) {
            ("x", [x]) => shadow.x(x.number()?),
            ("y", [y]) => shadow.y(y.number()?),
            ("blur", [blur]) => shadow.blur(blur.number()?),
            ("spread", [spread]) => shadow.spread(spread.number()?),
            ("color", [color]) => shadow.color(color::from_term(color)?),
            ("inset", []) => shadow.inset(),
            ("normal", []) => shadow.normal(),
            _ => return Err(format!("expected calls like {SYNTAX}, not `{term}`")),
        };
    }
    Ok(shadow)
}

/// Every part, so it reads back the same. Freya keeps a shadow's position and sizes to
/// itself, but shows them as `Normal 0 4 12 0`; this reads them from there.
pub fn format(shadow: &Shadow) -> String {
    let shown = shadow.to_string();
    let mut parts = shown.split_whitespace();
    let inset = parts.next() == Some("Inset");
    let mut size = || {
        parts
            .next()
            .and_then(|part| part.parse().ok())
            .unwrap_or(0.)
    };
    let [x, y, blur, spread] = [size(), size(), size(), size()].map(number::format);
    let mut written = format!(
        "x({x}) y({y}) blur({blur}) spread({spread}) color({})",
        color::format(&shadow.color)
    );
    if inset {
        written.push_str(" inset()");
    }
    written
}

#[cfg(test)]
mod tests {
    use freya::prelude::Color;

    use super::*;

    #[test]
    fn shadows_read_back_as_written() {
        for written in [
            "x(0) y(4) blur(12) spread(0) color(argb(77, 0, 0, 0))",
            "x(1.5) y(-2) blur(0) spread(3) color(red) inset()",
        ] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
    }

    #[test]
    fn calls_apply_in_order_from_a_new_shadow() {
        assert_eq!(
            parse("y(4) blur(8) color(black)").unwrap(),
            Shadow::new().y(4.).blur(8.).color(Color::BLACK)
        );
        assert_eq!(
            format(&Shadow::new()),
            format!(
                "x(0) y(0) blur(0) spread(0) color({})",
                color::format(&Shadow::new().color)
            )
        );
    }

    #[test]
    fn bad_shadows() {
        assert!(parse("big").is_err());
        assert!(parse("x(1, 2)").is_err());
        assert!(
            parse("glow(3)")
                .unwrap_err()
                .starts_with("expected calls like")
        );
    }
}
