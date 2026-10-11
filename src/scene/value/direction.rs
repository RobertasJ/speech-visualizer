//! Which way a rect lays out its children.

use freya::prelude::Direction;

use super::syntax;

const NAMES: [(&str, Direction); 2] = [
    ("vertical", Direction::Vertical),
    ("horizontal", Direction::Horizontal),
];

pub const SYNTAX: &str = "vertical | horizontal";

pub fn parse(input: &str) -> Result<Direction, String> {
    syntax::keyword(&syntax::one(input)?, &NAMES)
}

pub fn format(direction: &Direction) -> String {
    syntax::keyword_name(direction, &NAMES)
        .expect("every direction has a name")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_read_back() {
        assert_eq!(parse("horizontal"), Ok(Direction::Horizontal));
        assert_eq!(format(&Direction::Vertical), "vertical");
        assert!(parse("diagonal").is_err());
    }
}
