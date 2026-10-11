//! Effects on how an element and what's inside it are drawn. Kept in its effect data, as
//! Freya's `opacity()`, `rotation()`, `scale()` and so on set it. Ones that aren't set
//! show as their neutral value.

use freya::prelude::{EffectData, Scale};

use super::{Group, optional};
use crate::scene::value::{effect, number};

fn format_number(value: &Option<f32>, neutral: &str) -> String {
    optional(value.as_ref(), |value| number::format(*value), neutral)
}

pub const GROUP: Group<EffectData> = Group {
    name: "effects",
    description: "Effects on how it and what's inside it are drawn.",
    properties: &[
        one!(
            "opacity",
            "N from 0 to 1",
            "How opaque it is: 0 is invisible, 1 fully drawn.",
            |effect| effect.opacity,
            |input| number::parse(input).map(Some),
            |opacity| format_number(opacity, "1"),
        ),
        one!(
            "rotation",
            "N",
            "Turns it by N degrees, around its transform_origin.",
            |effect| effect.rotation,
            |input| number::parse(input).map(Some),
            |rotation| format_number(rotation, "0"),
        ),
        one!(
            "scale",
            effect::SCALE_SYNTAX,
            "Scales it, around its transform_origin.",
            |effect| effect.scale,
            |input| effect::parse_scale(input).map(Some),
            |scale: &Option<Scale>| optional(scale.as_ref(), effect::format_scale, "1"),
        ),
        one!(
            "transform_origin",
            effect::ORIGIN_SYNTAX,
            "The point rotation and scale turn and grow it around.",
            |effect| effect.transform_origin,
            effect::parse_origin,
            effect::format_origin,
        ),
        one!(
            "overflow",
            effect::OVERFLOW_SYNTAX,
            "Whether what's inside it can be drawn outside its edges, or is clipped.",
            |effect| effect.overflow,
            effect::parse_overflow,
            effect::format_overflow,
        ),
        one!(
            "backdrop_blur",
            "N",
            "Blurs what's behind it by N.",
            |effect| effect.blur,
            |input| number::parse(input).map(Some),
            |blur| format_number(blur, "0"),
        ),
    ],
    shorthands: &[],
};
