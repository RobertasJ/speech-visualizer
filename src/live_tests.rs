use super::*;

#[test]
fn normalize_drops_punctuation_and_case() {
    assert_eq!(
        normalize(" Text in 3, Hello World."),
        " text in 3 hello world"
    );
}
