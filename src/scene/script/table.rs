//! The script functions, described once for both the console's help and the agent's
//! prompt.

/// A function scripts can call, as help shows it.
pub struct Function {
    /// How it's called, like `e.add_text(content) -> Element`. `e` is an element.
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

/// Every function scripts can call on the scene. `e` stands for an element.
pub const FUNCTIONS: &[Function] = &[
    function(
        "el(id) -> Element",
        "The element with that id. Fails if there's none.",
    ),
    function(
        "rect() -> Element",
        "Adds an empty rect at the end of the top level.",
    ),
    function(
        "text(content) -> Element",
        "Adds text at the end of the top level.",
    ),
    function("roots() -> [Element]", "The top-level elements, in order."),
    function("clear()", "Removes every element."),
    function(
        "e.add_rect() -> Element",
        "Adds an empty rect at the end of rect e.",
    ),
    function(
        "e.add_text(content) -> Element",
        "Adds text at the end of rect e.",
    ),
    function("e.text", "The text of text element e. Can be set."),
    function(
        "e.desc",
        "What e is for, \"\" if it has no description. Can be set.",
    ),
    function("e.id", "e's id."),
    function(
        "e.parent",
        "The rect e is in, or () if it's at the top level.",
    ),
    function("e.children", "The elements in rect e, in order."),
    function("e.remove()", "Removes e and everything in it."),
    function(
        "print(e)",
        "Shows e, like `#5 text \"Hello\" (greeting)` or `#4 rect (page header)`.",
    ),
];
