#![feature(assert_matches)]
/// Why 'sema'?
///
/// The point of this module is to analyze beyond the syntax -- in BIR, we
/// decomposed the AST into a representation more amenable to analysis, but we
/// didn't perform any checking that the language semantics are being upheld.
///
/// What semantics do we care about checking?
///     - Variables or functions are in scope
///     - Types used are compatible or can be inferred
///     - Methods and field accesses are correct
pub mod check;
pub mod errors;
pub mod types;

pub use types::*;

#[cfg(test)]
mod tests {}
