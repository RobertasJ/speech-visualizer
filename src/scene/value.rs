//! Values as the strings scripts write them, after the names in Freya's Rust API, and
//! back: each kind of value has a module that parses it into Freya's type and formats
//! it again, with the syntax it takes for the docs. See [`syntax`] for the grammar.

pub mod alignment;
pub mod border;
pub mod color;
pub mod content;
pub mod corner_radius;
pub mod direction;
pub mod effect;
pub mod font;
pub mod gaps;
pub mod number;
pub mod position;
pub mod shadow;
pub mod size;
pub mod syntax;
pub mod text;
