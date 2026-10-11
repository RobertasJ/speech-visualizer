//! How a rect sizes and places its children: as they are, fitted to the widest,
//! sharing space with `flex` sizes, or wrapped onto new lines.

use freya::prelude::Content;

use super::{number, syntax};

pub const SYNTAX: &str = "normal | fit | flex | wrap | wrap_spacing(N)";

pub fn parse(input: &str) -> Result<Content, String> {
    let term = syntax::one(input)?;
    if let Some(args) = syntax::call(&term, "wrap_spacing") {
        return Ok(Content::wrap_spacing(syntax::numbers(args, 1)?[0]));
    }
    match term.name() {
        Some("normal") => Ok(Content::normal()),
        Some("fit") => Ok(Content::fit()),
        Some("flex") => Ok(Content::flex()),
        Some("wrap") => Ok(Content::wrap()),
        _ => Err(format!("expected {SYNTAX}, not `{term}`")),
    }
}

pub fn format(content: &Content) -> String {
    match content {
        Content::Normal => "normal".to_owned(),
        Content::Fit => "fit".to_owned(),
        Content::Flex => "flex".to_owned(),
        Content::Wrap { wrap_spacing: None } => "wrap".to_owned(),
        Content::Wrap {
            wrap_spacing: Some(spacing),
        } => format!("wrap_spacing({})", number::format(*spacing)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contents_read_back_as_written() {
        for written in ["normal", "fit", "flex", "wrap", "wrap_spacing(8)"] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
        assert!(parse("grid").is_err());
    }
}
