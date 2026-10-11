//! The script functions, described once for both the console's help and the agent's
//! prompt. The properties are described by each kind of element's tables.

/// A function scripts can call, as help shows it.
pub struct Function {
    /// How it's called, like `p.add_span(text) -> Element`.
    pub signature: &'static str,
    /// What it does, in one line.
    pub description: &'static str,
}

const fn function(signature: &'static str, description: &'static str) -> Function {
    Function {
        signature,
        description,
    }
}

/// Every function scripts can call on the scene. `e` stands for any element, `r` for a
/// rect and `p` for a paragraph.
pub const FUNCTIONS: &[Function] = &[
    function(
        "el(id) -> Element",
        "The element with that id. Fails if there's none.",
    ),
    function(
        "root() -> Element",
        "The rect that fills the window and holds everything; its id is 0.",
    ),
    function("rect() -> Element", "Adds a rect at the end of the root."),
    function(
        "paragraph() -> Element",
        "Adds a paragraph at the end of the root.",
    ),
    function("clear()", "Removes everything in the root."),
    function(
        "r.add_rect() -> Element",
        "Adds a rect at the end of rect r.",
    ),
    function(
        "r.add_paragraph() -> Element",
        "Adds a paragraph at the end of rect r.",
    ),
    function(
        "p.add_span(text) -> Element",
        "Adds a span with that text at the end of paragraph p.",
    ),
    function("e.kind", "\"rect\", \"paragraph\" or \"span\"."),
    function("e.id", "e's id."),
    function(
        "e.desc",
        "What e is for, \"\" if it has no description. Can be set.",
    ),
    function("e.parent", "The element e is in, or () for the root."),
    function("e.children", "The elements in e, in order."),
    function("e.remove()", "Removes e and everything in it."),
    function(
        "e.name",
        "Reads property `name`: a string, or an array of them for a list.",
    ),
    function(
        "e.name = value",
        "Sets property `name` from a string or a number. A list gets one more.",
    ),
    function(
        "e.name(value) -> Element",
        "Sets property `name` the same way, and gives e back, for chaining.",
    ),
    function("e.clear_name() -> Element", "Empties list property `name`."),
    function(
        "e.shorthand() -> Element",
        "Applies a shorthand, like expanded(), and gives e back.",
    ),
    function(
        "print(e)",
        "Shows e, like `#5 span \"Hello\" (greeting)` or `#4 rect (page header)`.",
    ),
];
