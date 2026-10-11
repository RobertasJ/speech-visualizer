//! Effects: clipping, scaling, the origin they turn and scale around, and layers.

use freya::prelude::{Layer, OriginValue, Overflow, Scale, TransformOrigin};

use super::number;
use super::syntax::{self, Term};

const OVERFLOWS: [(&str, Overflow); 2] = [("none", Overflow::None), ("clip", Overflow::Clip)];

pub const OVERFLOW_SYNTAX: &str = "none | clip";

pub fn parse_overflow(input: &str) -> Result<Overflow, String> {
    syntax::keyword(&syntax::one(input)?, &OVERFLOWS)
}

pub fn format_overflow(overflow: &Overflow) -> String {
    syntax::keyword_name(overflow, &OVERFLOWS)
        .expect("every overflow has a name")
        .to_owned()
}

pub const SCALE_SYNTAX: &str = "N | (x, y)";

pub fn parse_scale(input: &str) -> Result<Scale, String> {
    match syntax::one(input)? {
        Term::Number(both) => Ok(Scale::from(both)),
        Term::Tuple(items) => {
            let [x, y] = syntax::numbers(&items, 2)?[..] else {
                unreachable!("`numbers` gives as many as asked for")
            };
            Ok(Scale::from((x, y)))
        }
        term => Err(format!("expected {SCALE_SYNTAX}, not `{term}`")),
    }
}

pub fn format_scale(scale: &Scale) -> String {
    if scale.x == scale.y {
        number::format(scale.x)
    } else {
        format!("({}, {})", number::format(scale.x), number::format(scale.y))
    }
}

/// Freya's named origins, as fractions of the width and height.
const ORIGINS: [(&str, (f32, f32)); 9] = [
    ("center", (0.5, 0.5)),
    ("top_left", (0., 0.)),
    ("top", (0.5, 0.)),
    ("top_right", (1., 0.)),
    ("left", (0., 0.5)),
    ("right", (1., 0.5)),
    ("bottom_left", (0., 1.)),
    ("bottom", (0.5, 1.)),
    ("bottom_right", (1., 1.)),
];

pub const ORIGIN_SYNTAX: &str = "center | top_left | top | top_right | left | right | bottom_left | bottom | bottom_right | (x, y), each fraction(N) or pixels(N)";

pub fn parse_origin(input: &str) -> Result<TransformOrigin, String> {
    let term = syntax::one(input)?;
    if let Term::Tuple(items) = &term {
        let [x, y] = &items[..] else {
            return Err(format!("expected (x, y), not `{term}`"));
        };
        return Ok(TransformOrigin {
            x: origin_value(x)?,
            y: origin_value(y)?,
        });
    }
    let (x, y) = syntax::keyword(&term, &ORIGINS)
        .map_err(|_| format!("expected {ORIGIN_SYNTAX}, not `{term}`"))?;
    Ok(TransformOrigin {
        x: OriginValue::Fraction(x),
        y: OriginValue::Fraction(y),
    })
}

fn origin_value(term: &Term) -> Result<OriginValue, String> {
    if let Some(args) = syntax::call(term, "fraction") {
        return Ok(OriginValue::Fraction(syntax::numbers(args, 1)?[0]));
    }
    if let Some(args) = syntax::call(term, "pixels") {
        return Ok(OriginValue::Pixels(syntax::numbers(args, 1)?[0]));
    }
    Err(format!("expected fraction(N) or pixels(N), not `{term}`"))
}

pub fn format_origin(origin: &TransformOrigin) -> String {
    if let (OriginValue::Fraction(x), OriginValue::Fraction(y)) = (origin.x, origin.y)
        && let Some(name) = syntax::keyword_name(&(x, y), &ORIGINS)
    {
        return name.to_owned();
    }
    let value = |value: OriginValue| match value {
        OriginValue::Fraction(fraction) => format!("fraction({})", number::format(fraction)),
        OriginValue::Pixels(pixels) => format!("pixels({})", number::format(pixels)),
    };
    format!("({}, {})", value(origin.x), value(origin.y))
}

pub const LAYER_SYNTAX: &str = "N | relative(N) | overlay | overlay_level(N), N from 0 to 16";

pub fn parse_layer(input: &str) -> Result<Layer, String> {
    let term = syntax::one(input)?;
    match &term {
        Term::Number(layer) => number::whole::<i16>(*layer).map(Layer::Relative),
        _ if term.name() == Some("overlay") => Ok(Layer::Overlay),
        _ => {
            if let Some(args) = syntax::call(&term, "relative") {
                return number::whole::<i16>(syntax::numbers(args, 1)?[0]).map(Layer::Relative);
            }
            if let Some(args) = syntax::call(&term, "overlay_level") {
                return number::whole::<u8>(syntax::numbers(args, 1)?[0]).map(Layer::OverlayLevel);
            }
            Err(format!("expected {LAYER_SYNTAX}, not `{term}`"))
        }
    }
}

pub fn format_layer(layer: &Layer) -> String {
    match layer {
        Layer::Relative(layer) => layer.to_string(),
        Layer::Overlay => "overlay".to_owned(),
        Layer::OverlayLevel(level) => format!("overlay_level({level})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_and_scale() {
        assert_eq!(format_overflow(&parse_overflow("clip").unwrap()), "clip");
        assert_eq!(format_scale(&parse_scale("1.5").unwrap()), "1.5");
        assert_eq!(format_scale(&parse_scale("(2, 0.5)").unwrap()), "(2, 0.5)");
        assert!(parse_scale("(1, 2, 3)").is_err());
    }

    #[test]
    fn origins_by_name_or_parts() {
        assert_eq!(
            format_origin(&parse_origin("top_right").unwrap()),
            "top_right"
        );
        assert_eq!(parse_origin("center"), Ok(TransformOrigin::center()));
        let written = "(pixels(10), fraction(0.25))";
        assert_eq!(format_origin(&parse_origin(written).unwrap()), written);
        assert_eq!(
            format_origin(&parse_origin("(fraction(0), fraction(1))").unwrap()),
            "bottom_left"
        );
        assert!(
            parse_origin("(1, 2)")
                .unwrap_err()
                .contains("fraction(N) or pixels(N)")
        );
        assert!(parse_origin("middle").is_err());
    }

    #[test]
    fn layers() {
        assert_eq!(parse_layer("2"), Ok(Layer::Relative(2)));
        assert_eq!(parse_layer("relative(-1)"), Ok(Layer::Relative(-1)));
        assert_eq!(format_layer(&parse_layer("overlay").unwrap()), "overlay");
        assert_eq!(
            format_layer(&parse_layer("overlay_level(3)").unwrap()),
            "overlay_level(3)"
        );
        assert!(parse_layer("1.5").is_err());
    }
}
