//! How a rect lays out its children. Kept in its layout node, as Freya's `direction()`,
//! `main_align()`, `spacing()` and so on set it.

use freya::prelude::{Alignment, Direction};
use freya::torin::geometry::Length;
use freya::torin::node::Node;

use super::{Group, Shorthand};
use crate::scene::value::{alignment, content, direction, number};

fn parse_length(input: &str) -> Result<Length, String> {
    number::parse(input).map(Length::new)
}

fn format_length(length: &Length) -> String {
    number::format(length.get())
}

pub const GROUP: Group<Node> = Group {
    name: "container",
    description: "How it lays out what's inside it: one after another along its direction, \
                  aligned along it and across it.",
    properties: &[
        one!(
            "direction",
            direction::SYNTAX,
            "Lays its children out in a column, or in a row.",
            |node| node.direction,
            direction::parse,
            direction::format,
        ),
        one!(
            "main_align",
            alignment::SYNTAX,
            "Where its children go along its direction.",
            |node| node.main_alignment,
            alignment::parse,
            alignment::format,
        ),
        one!(
            "cross_align",
            alignment::SYNTAX,
            "Where its children go across its direction.",
            |node| node.cross_alignment,
            alignment::parse,
            alignment::format,
        ),
        one!(
            "spacing",
            number::SYNTAX,
            "Space between its children.",
            |node| node.spacing,
            parse_length,
            format_length,
        ),
        one!(
            "content",
            content::SYNTAX,
            "How its children are sized: as they are, fit to the widest, sharing space by \
             their flex(N) sizes, or wrapped onto new lines.",
            |node| node.content,
            content::parse,
            content::format,
        ),
        one!(
            "offset_x",
            number::SYNTAX,
            "Shifts its children sideways, as scrolling does.",
            |node| node.offset_x,
            parse_length,
            format_length,
        ),
        one!(
            "offset_y",
            number::SYNTAX,
            "Shifts its children up or down, as scrolling does.",
            |node| node.offset_y,
            parse_length,
            format_length,
        ),
    ],
    shorthands: &[
        Shorthand {
            name: "center",
            description: "Centers its children both ways: main_align and cross_align `center`.",
            apply: |node| {
                node.main_alignment = Alignment::Center;
                node.cross_alignment = Alignment::Center;
            },
        },
        Shorthand {
            name: "horizontal",
            description: "Lays its children out in a row: direction `horizontal`.",
            apply: |node| node.direction = Direction::Horizontal,
        },
        Shorthand {
            name: "vertical",
            description: "Lays its children out in a column: direction `vertical`.",
            apply: |node| node.direction = Direction::Vertical,
        },
    ],
};
