use std::collections::HashSet;

use cst::green::SyntaxKind::{self, *};
use cst::syntax;
use utils::Position;

pub mod grammar;
pub mod input;
pub mod output;

mod types;

pub use input::Input;
pub use output::Output;
pub use types::*;

pub trait TokenSource {
    fn kind_at(&self, index: usize) -> SyntaxKind;
    fn text_at(&self, index: usize) -> &str;
}

pub trait EventSink {
    fn start_node(&mut self, kind: SyntaxKind);
    fn finish_node(&mut self);
    fn n_tokens(&mut self, kind: SyntaxKind, n: usize);
    fn error(&mut self, msg: String);
}

pub fn parse(input: Input) -> Output {
    output::parse(input, grammar::EntryPoint::Module)
}

pub fn parse_str(s: &str) -> Output {
    parse_str_from_entry(s, grammar::EntryPoint::Module)
}

pub fn parse_str_from_entry(s: &str, entry: grammar::EntryPoint) -> Output {
    output::parse(Input::lex(s.as_ref()), entry)
}

#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub pos: Position,
    pub len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn check_from_entry(
        input: &str,
        expected: expect_test::Expect,
        entry: grammar::EntryPoint,
    ) {
        let input = input.trim();
        eprintln!("{}", input);
        let Output { root, errors } = parse_str_from_entry(input, entry);
        eprintln!("{:#?}", errors);
        expected.assert_eq(&root.to_string());
        assert_eq!(errors.len(), 0);
    }

    pub fn check_tree(input: &str, expected: expect_test::Expect) {
        check_from_entry(input.trim(), expected, grammar::EntryPoint::Module);
    }
}
