//! Math typesetting: LaTeX subset → TeX-style layout using an OpenType MATH font.

mod layout;
pub mod otf;
mod parse;
mod symbols;

pub use layout::{Item, MathBox, MathFont};
