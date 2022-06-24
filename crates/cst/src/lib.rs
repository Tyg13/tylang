#![feature(arc_unwrap_or_clone)]
mod hash;

pub mod green;
pub mod lexer;
pub mod parser;
pub mod syntax;
mod utils;

pub use green::SyntaxKind;
