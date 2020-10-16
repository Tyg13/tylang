use crate::lexer::TokenMap;
use crate::util::Source;

mod constant;
mod error;
mod expression;
mod function;
mod item;
pub mod parser;
mod scope;
mod statement;
mod tree;
mod r#type;
mod variable;
mod visit;
pub use constant::*;
use error::*;
pub use expression::*;
pub use function::*;
pub use item::*;
pub use parser::*;
pub use r#type::*;
pub use scope::*;
pub use statement::*;
pub use tree::*;
pub use variable::*;
pub use visit::*;

// Ast rough sketch (future, not as currently implemented)
//
// Tree ::== [Function]*
// Function ::= "fn" Ident "(" [Param ","?]* ")" ["->" Type]? [Scope / ";"]
// Param ::= Ident ":" Type
// Expr ::= Scope / Group / Binary / Return
// Scope ::= "{" [Statement ";"]* [Expr]? "}"
// Statement ::= Decl / Expr
// Decl ::= "let" Ident [":" Type]? ["=" Expr]?
// Group ::= "(" Expr ")"
// Binary ::= Expr BinOp Expr
// Return ::= "return" Expr

pub fn parse(source: &Source, map: TokenMap, out: &mut dyn std::io::Write) -> Tree {
    log::debug!("Parsing {}", source.file());
    Parser::new(
        &map.tokens().strip_comments_and_whitespace(),
        &mut StreamHandler::new(source, out),
    )
    .parse()
}

#[cfg(test)]
pub mod test {
    use crate::ast::parser::{Parse, Parser};
    use crate::ast::Tree;
    use crate::util::SourceBuilder;

    pub use crate::ast::expression::test::*;
    pub use crate::ast::function::test::*;
    pub use crate::ast::item::test::*;
    pub use crate::ast::r#type::test::*;
    pub use crate::ast::statement::test::*;
    pub use crate::ast::tree::test::*;
    pub use crate::ast::variable::test::*;

    pub fn parse(source: &'static str) -> Tree {
        let source = SourceBuilder::new().source(source).build();
        let mut out = Vec::new();
        super::parse(&source, crate::lexer::lex(&source), &mut out)
    }

    pub fn parse_one<T: Parse>(source: &'static str) -> T {
        Parser::test(source).parse_one().unwrap()
    }
}
