#![feature(assert_matches)]

use ast::Node;

pub mod translate;
mod types;

pub use types::*;

pub type ParseResult = Result<Module, (ast::Module, cst::parser::Output)>;

pub fn parse_module_from_str(s: impl AsRef<str>) -> ParseResult {
    let output = cst::parser::parse_str(s.as_ref());
    let module = ast::Module::cast(output.root.clone()).unwrap();
    if output.errors.len() > 0 {
        Err((module, output))
    } else {
        Ok(translate::module(&module).unwrap())
    }
}
