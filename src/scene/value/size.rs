//! Sizes, for widths and heights, and visible sizes.

use freya::prelude::{Size, VisibleSize};
use freya::torin::geometry::Length;

use super::number;
use super::syntax::{self, Term};

pub const SYNTAX: &str =
    "auto | fill | fill_minimum | px(N) | percent(N) | window_percent(N) | flex(N), or N for px(N)";

pub fn parse(input: &str) -> Result<Size, String> {
    let term = syntax::one(input)?;
    if let Term::Number(pixels) = term {
        return Ok(Size::px(pixels));
    }
    if let Some(name) = term.name() {
        return match name {
            "auto" => Ok(Size::auto()),
            "fill" => Ok(Size::fill()),
            "fill_minimum" => Ok(Size::fill_minimum()),
            _ => Err(format!("expected a size like {SYNTAX}, not `{term}`")),
        };
    }
    if let Term::Call(name, args) = &term {
        let [value] = syntax::numbers(args, 1)?[..] else {
            unreachable!("`numbers` gives as many as asked for")
        };
        match name.as_str() {
            "px" => return Ok(Size::px(value)),
            "percent" => return Ok(Size::percent(value)),
            "window_percent" => return Ok(Size::window_percent(value)),
            "flex" => return Ok(Size::flex(value)),
            _ => {}
        }
    }
    Err(format!("expected a size like {SYNTAX}, not `{term}`"))
}

pub fn format(size: &Size) -> String {
    let call = |name: &str, length: &Length| format!("{name}({})", number::format(length.get()));
    match size {
        Size::Inner => "auto".to_owned(),
        Size::Fill => "fill".to_owned(),
        Size::FillMinimum => "fill_minimum".to_owned(),
        Size::Pixels(length) => call("px", length),
        Size::Percentage(length) => call("percent", length),
        Size::RootPercentage(length) => call("window_percent", length),
        Size::Flex(length) => call("flex", length),
        // Only Rust code makes these.
        Size::Fn(_) => "func".to_owned(),
    }
}

pub const VISIBLE_SYNTAX: &str = "full | inner_percentage(N)";

pub fn parse_visible(input: &str) -> Result<VisibleSize, String> {
    let term = syntax::one(input)?;
    if term.name() == Some("full") {
        return Ok(VisibleSize::Full);
    }
    if let Some(args) = syntax::call(&term, "inner_percentage") {
        let percent = syntax::numbers(args, 1)?[0];
        return Ok(VisibleSize::InnerPercentage(Length::new(percent)));
    }
    Err(format!("expected {VISIBLE_SYNTAX}, not `{term}`"))
}

pub fn format_visible(size: &VisibleSize) -> String {
    match size {
        VisibleSize::Full => "full".to_owned(),
        VisibleSize::InnerPercentage(length) => {
            format!("inner_percentage({})", number::format(length.get()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_back_as_written() {
        for written in [
            "auto",
            "fill",
            "fill_minimum",
            "px(120)",
            "percent(50)",
            "window_percent(25.5)",
            "flex(1)",
        ] {
            assert_eq!(format(&parse(written).unwrap()), written);
        }
    }

    #[test]
    fn a_number_is_pixels_and_constructors_read_too() {
        assert_eq!(parse("120"), Ok(Size::px(120.)));
        assert_eq!(parse("fill()"), Ok(Size::fill()));
    }

    #[test]
    fn bad_sizes_say_what_sizes_look_like() {
        assert!(
            parse("huge")
                .unwrap_err()
                .starts_with("expected a size like auto")
        );
        assert!(
            parse("px(1, 2)")
                .unwrap_err()
                .contains("expected 1 numbers")
        );
        assert!(parse("em(2)").is_err());
    }

    #[test]
    fn visible_sizes() {
        assert_eq!(format_visible(&parse_visible("full").unwrap()), "full");
        assert_eq!(
            format_visible(&parse_visible("inner_percentage(50)").unwrap()),
            "inner_percentage(50)"
        );
        assert!(parse_visible("half").is_err());
    }
}
