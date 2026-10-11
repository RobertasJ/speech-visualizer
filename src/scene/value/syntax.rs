//! The grammar values are written in, after the names in Freya's Rust API: words like
//! `fill`, numbers like `120`, calls like `px(120)` or `width(2)`, tuples like
//! `(8, 16)`, text in quotes like `"…"`, and colors like `#ff8800`. A value is one or
//! more of these, separated by spaces or dots, so `new_absolute().top(10)` reads too.

use std::fmt;

/// One part of a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    Word(String),
    Number(f32),
    /// A name with arguments, like `px(120)` or `rgb(255, 0, 0)`.
    Call(String, Vec<Term>),
    Tuple(Vec<Term>),
    /// Text in double quotes.
    Text(String),
    /// A color in hex, without its `#`.
    Hex(String),
}

impl Term {
    pub fn number(&self) -> Result<f32, String> {
        match self {
            Term::Number(number) => Ok(*number),
            other => Err(format!("expected a number, not `{other}`")),
        }
    }

    /// The name of a word, or of a call without arguments, so `center()` reads like
    /// `center`.
    pub fn name(&self) -> Option<&str> {
        match self {
            Term::Word(word) => Some(word),
            Term::Call(name, args) if args.is_empty() => Some(name),
            _ => None,
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |items: &[Term]| -> String {
            let items: Vec<String> = items.iter().map(Term::to_string).collect();
            items.join(", ")
        };
        match self {
            Term::Word(word) => f.write_str(word),
            Term::Number(number) => write!(f, "{number}"),
            Term::Call(name, args) => write!(f, "{name}({})", list(args)),
            Term::Tuple(items) => write!(f, "({})", list(items)),
            Term::Text(text) => write!(f, "{text:?}"),
            Term::Hex(hex) => write!(f, "#{hex}"),
        }
    }
}

/// The terms of `input`.
pub fn terms(input: &str) -> Result<Vec<Term>, String> {
    let mut parser = Parser { input, position: 0 };
    let mut terms = Vec::new();
    loop {
        parser.skip_spaces();
        // A dot between terms, as in `new_absolute().top(10)`; one before a digit
        // starts a number.
        if !terms.is_empty() && parser.peek() == Some('.') && !parser.next_is_digit() {
            parser.bump();
            continue;
        }
        if parser.peek().is_none() {
            return Ok(terms);
        }
        terms.push(parser.term()?);
    }
}

/// The one term `input` is.
pub fn one(input: &str) -> Result<Term, String> {
    let mut terms = terms(input)?;
    match terms.len() {
        1 => Ok(terms.remove(0)),
        0 => Err("expected a value".to_owned()),
        _ => Err(format!("expected one value, not `{input}`")),
    }
}

/// The value `term` names among `names`, Freya's names for the values of a type.
pub fn keyword<T: Clone>(term: &Term, names: &[(&str, T)]) -> Result<T, String> {
    term.name()
        .and_then(|name| names.iter().find(|(known, _)| *known == name))
        .map(|(_, value)| value.clone())
        .ok_or_else(|| format!("expected {}, not `{term}`", one_of(names)))
}

/// Freya's name for `value` among `names`, if it has one.
pub fn keyword_name<T: PartialEq>(value: &T, names: &[(&'static str, T)]) -> Option<&'static str> {
    names
        .iter()
        .find(|(_, known)| known == value)
        .map(|(name, _)| *name)
}

/// `names`, listed for an error: `a, b or c`.
pub fn one_of<T>(names: &[(&str, T)]) -> String {
    let names: Vec<&str> = names.iter().map(|(name, _)| *name).collect();
    match names.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// The arguments of `term` if it calls `name`.
pub fn call<'a>(term: &'a Term, name: &str) -> Option<&'a [Term]> {
    match term {
        Term::Call(called, args) if called == name => Some(args),
        _ => None,
    }
}

/// The numbers `args` are, of which there must be `count`.
pub fn numbers(args: &[Term], count: usize) -> Result<Vec<f32>, String> {
    if args.len() != count {
        return Err(format!("expected {count} numbers, not {}", args.len()));
    }
    args.iter().map(Term::number).collect()
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn next_is_digit(&self) -> bool {
        let mut rest = self.input[self.position..].chars();
        rest.next();
        rest.next().is_some_and(|c| c.is_ascii_digit())
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.position += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, expected: char) -> bool {
        let found = self.peek() == Some(expected);
        if found {
            self.bump();
        }
        found
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> &str {
        let start = self.position;
        while self.peek().is_some_and(&keep) {
            self.bump();
        }
        &self.input[start..self.position]
    }

    fn term(&mut self) -> Result<Term, String> {
        self.skip_spaces();
        match self.peek() {
            Some('(') => {
                self.bump();
                Ok(Term::Tuple(self.list(')')?))
            }
            Some('"') => {
                self.bump();
                self.text()
            }
            Some('#') => {
                self.bump();
                let hex = self.take_while(|c| c.is_ascii_alphanumeric());
                if hex.is_empty() {
                    return Err("expected hex digits after `#`".to_owned());
                }
                Ok(Term::Hex(hex.to_owned()))
            }
            Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.') => {
                let number =
                    self.take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'));
                number
                    .parse()
                    .map(Term::Number)
                    .map_err(|_| format!("`{number}` isn't a number"))
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let word = self
                    .take_while(|c| c.is_alphanumeric() || c == '_')
                    .to_owned();
                if self.eat('(') {
                    Ok(Term::Call(word, self.list(')')?))
                } else {
                    Ok(Term::Word(word))
                }
            }
            Some(c) => Err(format!("unexpected `{c}`")),
            None => Err("expected a value".to_owned()),
        }
    }

    /// Terms separated by commas, up to `close`.
    fn list(&mut self, close: char) -> Result<Vec<Term>, String> {
        let mut items = Vec::new();
        loop {
            self.skip_spaces();
            if self.eat(close) {
                return Ok(items);
            }
            if self.peek().is_none() {
                return Err(format!("missing `{close}`"));
            }
            if !items.is_empty() {
                if !self.eat(',') {
                    return Err(format!("expected `,` or `{close}`"));
                }
                self.skip_spaces();
                if self.eat(close) {
                    return Ok(items);
                }
            }
            items.push(self.term()?);
        }
    }

    /// Text up to the closing quote, which `\"` doesn't end.
    fn text(&mut self) -> Result<Term, String> {
        let mut text = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(Term::Text(text)),
                Some('\\') => match self.bump() {
                    Some(escaped) => text.push(escaped),
                    None => break,
                },
                Some(c) => text.push(c),
                None => break,
            }
        }
        Err("missing closing `\"`".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(word: &str) -> Term {
        Term::Word(word.to_owned())
    }

    fn call(name: &str, args: Vec<Term>) -> Term {
        Term::Call(name.to_owned(), args)
    }

    #[test]
    fn reads_each_kind_of_term() {
        assert_eq!(
            terms(r#"fill 12 -0.5 px(120) (8, 16) "a \"b\"" #ff8800 center()"#),
            Ok(vec![
                word("fill"),
                Term::Number(12.),
                Term::Number(-0.5),
                call("px", vec![Term::Number(120.)]),
                Term::Tuple(vec![Term::Number(8.), Term::Number(16.)]),
                Term::Text(r#"a "b""#.to_owned()),
                Term::Hex("ff8800".to_owned()),
                call("center", vec![]),
            ])
        );
    }

    #[test]
    fn calls_nest_and_chain_with_dots() {
        assert_eq!(
            terms("fill(rgb(1, 2, 3)) new_absolute().top(10)"),
            Ok(vec![
                call(
                    "fill",
                    vec![call(
                        "rgb",
                        vec![Term::Number(1.), Term::Number(2.), Term::Number(3.)]
                    )]
                ),
                call("new_absolute", vec![]),
                call("top", vec![Term::Number(10.)]),
            ])
        );
        assert_eq!(terms(".5"), Ok(vec![Term::Number(0.5)]));
    }

    #[test]
    fn errors_say_what_went_wrong() {
        assert_eq!(terms("px(1"), Err("missing `)`".to_owned()));
        assert_eq!(terms("(1 2)"), Err("expected `,` or `)`".to_owned()));
        assert_eq!(terms("1-2"), Err("`1-2` isn't a number".to_owned()));
        assert_eq!(terms("\"open"), Err("missing closing `\"`".to_owned()));
        assert_eq!(terms("%"), Err("unexpected `%`".to_owned()));
        assert_eq!(one(""), Err("expected a value".to_owned()));
        assert_eq!(one("a b"), Err("expected one value, not `a b`".to_owned()));
    }

    #[test]
    fn keywords_name_values() {
        let names = [("start", 0), ("center", 1), ("end", 2)];
        assert_eq!(keyword(&word("center"), &names), Ok(1));
        assert_eq!(keyword(&call("end", vec![]), &names), Ok(2));
        assert_eq!(
            keyword(&word("middle"), &names),
            Err("expected start, center or end, not `middle`".to_owned())
        );
        assert_eq!(keyword_name(&2, &names), Some("end"));
        assert_eq!(keyword_name(&7, &names), None);
    }

    #[test]
    fn terms_show_as_written() {
        let shown: Vec<String> = terms(r#"px(1.5) (1, 2) "x" #fff"#)
            .unwrap()
            .iter()
            .map(Term::to_string)
            .collect();
        assert_eq!(shown, ["px(1.5)", "(1, 2)", r#""x""#, "#fff"]);
    }
}
