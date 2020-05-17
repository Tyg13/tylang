use crate::lexer::TokenMap;
use crate::util::{ArmPosition, Source};
use std::rc::Rc;

mod constant;
mod error;
mod expression;
mod function;
pub mod parser;
mod statement;
mod r#type;
mod variable;
pub mod visit;
pub use constant::*;
use error::*;
pub use expression::*;
pub use function::*;
pub use parser::*;
pub use r#type::*;
pub use statement::*;
pub use variable::*;
pub use visit::Visitor;

#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub statements: Vec<Statement>,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner: Vec<String> = self
            .statements
            .iter()
            .map(|statement| statement.kind.to_string())
            .collect();
        write!(f, "[{}]", inner.join(","))
    }
}

pub fn parse(source: &Source, map: TokenMap, out: &mut dyn std::io::Write) -> Tree {
    log::debug!("Parsing {}", source.file());
    Parser::new(map).parse()
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub functions: Vec<Function>,
    any_errors: bool,
}

impl Tree {
    pub fn valid(&self) -> bool {
        !self.any_errors
    }
}
