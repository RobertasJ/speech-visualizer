//! How children are aligned along an axis.

use freya::prelude::Alignment;

use super::syntax;

const NAMES: [(&str, Alignment); 6] = [
    ("start", Alignment::Start),
    ("center", Alignment::Center),
    ("end", Alignment::End),
    ("space_between", Alignment::SpaceBetween),
    ("space_evenly", Alignment::SpaceEvenly),
    ("space_around", Alignment::SpaceAround),
];

pub const SYNTAX: &str = "start | center | end | space_between | space_evenly | space_around";

pub fn parse(input: &str) -> Result<Alignment, String> {
    syntax::keyword(&syntax::one(input)?, &NAMES)
}

pub fn format(alignment: &Alignment) -> String {
    syntax::keyword_name(alignment, &NAMES)
        .expect("every alignment has a name")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_alignment_reads_back() {
        for (name, alignment) in NAMES {
            assert_eq!(parse(name), Ok(alignment.clone()));
            assert_eq!(format(&alignment), name);
        }
        assert!(parse("middle").unwrap_err().contains("space_around"));
    }
}
