//! Layout: an element's size, space around it and where it's placed. Kept in its
//! layout node, as Freya's `width()`, `padding()`, `position()` and so on set it.

use freya::prelude::Size;
use freya::torin::node::Node;

use super::{Group, Shorthand};
use crate::scene::value::{gaps, position, size};

pub const GROUP: Group<Node> = Group {
    name: "layout",
    description: "Its size, the space inside and around it, and where it's placed.",
    properties: &[
        one!(
            "width",
            size::SYNTAX,
            "How wide it is.",
            |node| node.width,
            size::parse,
            size::format
        ),
        one!(
            "height",
            size::SYNTAX,
            "How tall it is.",
            |node| node.height,
            size::parse,
            size::format
        ),
        one!(
            "min_width",
            size::SYNTAX,
            "The narrowest it can be.",
            |node| node.minimum_width,
            size::parse,
            size::format,
        ),
        one!(
            "min_height",
            size::SYNTAX,
            "The shortest it can be.",
            |node| node.minimum_height,
            size::parse,
            size::format,
        ),
        one!(
            "max_width",
            size::SYNTAX,
            "The widest it can be.",
            |node| node.maximum_width,
            size::parse,
            size::format,
        ),
        one!(
            "max_height",
            size::SYNTAX,
            "The tallest it can be.",
            |node| node.maximum_height,
            size::parse,
            size::format,
        ),
        one!(
            "visible_width",
            size::VISIBLE_SYNTAX,
            "How much of its width it takes up, as a share of its content's.",
            |node| node.visible_width,
            size::parse_visible,
            size::format_visible,
        ),
        one!(
            "visible_height",
            size::VISIBLE_SYNTAX,
            "How much of its height it takes up, as a share of its content's.",
            |node| node.visible_height,
            size::parse_visible,
            size::format_visible,
        ),
        one!(
            "padding",
            gaps::SYNTAX,
            "Space between its edges and what's inside it.",
            |node| node.padding,
            gaps::parse,
            gaps::format,
        ),
        one!(
            "margin",
            gaps::SYNTAX,
            "Space around it, outside its edges.",
            |node| node.margin,
            gaps::parse,
            gaps::format,
        ),
        one!(
            "position",
            position::SYNTAX,
            "Stacked in the flow with its siblings, or taken out of it: absolute places it by \
             its parent's edges, global by the window's.",
            |node| node.position,
            position::parse,
            position::format,
        ),
    ],
    shorthands: &[Shorthand {
        name: "expanded",
        description: "Fills its parent both ways: width and height `fill`.",
        apply: |node| {
            node.width = Size::fill();
            node.height = Size::fill();
        },
    }],
};
