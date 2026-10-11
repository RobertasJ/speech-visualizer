//! Plain numbers, like `0.5`, and numbers that can be `none`.

use super::syntax::{self, Term};

pub const SYNTAX: &str = "N";
pub const OPTIONAL_SYNTAX: &str = "N | none";

pub fn parse(input: &str) -> Result<f32, String> {
    syntax::one(input)?.number()
}

/// `value` without needless digits, like `2` or `0.5`.
pub fn format(value: f32) -> String {
    value.to_string()
}

/// A number, or `none` for nothing.
pub fn parse_optional(input: &str) -> Result<Option<f32>, String> {
    match syntax::one(input)? {
        Term::Word(word) if word == "none" => Ok(None),
        term => term.number().map(Some),
    }
}

pub fn format_optional(value: Option<f32>) -> String {
    value.map_or_else(|| "none".to_owned(), format)
}

/// A whole number in `T`'s range, or `none`.
pub fn parse_whole_optional<T: TryFrom<i64>>(input: &str) -> Result<Option<T>, String> {
    parse_optional(input)?.map(whole).transpose()
}

/// `number`, which must be whole and in `T`'s range.
pub fn whole<T: TryFrom<i64>>(number: f32) -> Result<T, String> {
    if number.fract() != 0. {
        return Err(format!("expected a whole number, not {number}"));
    }
    T::try_from(number as i64).map_err(|_| format!("{number} is out of range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_and_show_without_needless_digits() {
        assert_eq!(parse("0.5"), Ok(0.5));
        assert_eq!(format(2.), "2");
        assert_eq!(format(0.25), "0.25");
        assert_eq!(parse("big"), Err("expected a number, not `big`".to_owned()));
    }

    #[test]
    fn optional_numbers_can_be_none() {
        assert_eq!(parse_optional("none"), Ok(None));
        assert_eq!(parse_optional("3"), Ok(Some(3.)));
        assert_eq!(format_optional(None), "none");
        assert_eq!(parse_whole_optional::<usize>("2"), Ok(Some(2)));
        assert_eq!(
            parse_whole_optional::<usize>("1.5"),
            Err("expected a whole number, not 1.5".to_owned())
        );
        assert_eq!(whole::<usize>(-1.), Err("-1 is out of range".to_owned()));
    }
}
