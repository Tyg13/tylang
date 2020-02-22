use crate::lexer::{Token, TokenKind, TokenMap};
use crate::span;
use crate::util::{ArmPosition, Source, Span};
use std::rc::Rc;

mod constant;
mod error;
mod expression;
mod statement;
mod variable;
pub use constant::*;
pub use error::*;
pub use expression::*;
pub use statement::*;
pub use variable::*;

#[derive(Debug, PartialEq)]
pub struct Tree {
    pub statements: Vec<Statement>,
}

impl Tree {
    fn new() -> Self {
        Self { statements: vec![] }
    }
}

pub fn parse(source: &Source, map: TokenMap, out: &mut dyn std::io::Write) -> Tree {
    Parser::new(source, map, out).parse()
}

type Result<T> = std::result::Result<T, Error>;

pub struct Parser<'a> {
    source: &'a Source,
    out: &'a mut dyn std::io::Write,
    map: TokenMap,
    index: usize,
    backtrack_index: usize,
    precedence: usize,
    tree: Tree,
}

impl<'a> Parser<'a> {
    fn new(source: &'a Source, map: TokenMap, out: &'a mut dyn std::io::Write) -> Self {
        Self {
            source,
            out,
            map,
            index: 0,
            backtrack_index: 0,
            precedence: 0,
            tree: Tree::new(),
        }
    }

    fn parse(self) -> Tree {
        let mut this = self;
        this.parse_statements();
        this.tree
    }

    fn peek(&self) -> Result<Token> {
        if self.index >= self.map.len() {
            return Err(Error::EOF);
        }
        Ok(self.map.token(self.index))
    }

    fn advance(&mut self) -> Result<Token> {
        let token = self.peek()?;
        self.sync();
        self.index = self
            .index
            .checked_add(1)
            .expect("Overflow in parser token index");
        Ok(token)
    }

    fn advance_until(&mut self, kind: TokenKind) -> Option<Token> {
        loop {
            match self.advance() {
                Ok(token) => {
                    if token.kind == TokenKind::SemiColon {
                        return Some(token);
                    }
                }
                Err(Error::EOF) => return None,
                Err(_) => continue,
            }
        }
    }

    fn sync(&mut self) {
        self.backtrack_index = self.index;
    }

    fn backtrack(&mut self) {
        self.index = self.backtrack_index;
    }

    fn maybe(&mut self, kind: TokenKind) -> Option<Token> {
        self.expect(kind).ok()
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        let token = self.advance()?;
        if token.kind != kind {
            self.backtrack();
            return Err(Error::UnexpectedToken {
                expected: kind.to_string(),
                unexpected: token,
            });
        }
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::*;
    use crate::util::SourceBuilder;

    #[test]
    fn stops_on_eof() {
        let source = SourceBuilder::new().build();
        let mut out = Vec::new();
        let mut parser = Parser::new(&source, TokenMap::new(), &mut out);
        assert_eq!(parser.advance(), Err(Error::EOF));
        let old_index = parser.index;
        assert_eq!(parser.advance(), Err(Error::EOF));
        let new_index = parser.index;
        assert_eq!(old_index, new_index);
    }

    #[macro_export]
    macro_rules! token {
        { $map:expr, $kind:ident, $span:expr } => {
            $map.add_token(TokenKind::$kind, $span);
        }
    }
    #[macro_export]
    macro_rules! identifier {
        { $map:expr, $name:expr, $span:expr } => {
            $map.add_ident(String::from($name), $span);
        }
    }
    #[macro_export]
    macro_rules! number {
        { $map:expr, $value:expr, $span:expr } => {
            $map.add_number($value, $span);
        }
    }
    #[macro_export]
    macro_rules! tree {
        [$source:literal, $($entry:ident { $($args:tt)* },)*] => {{
            let mut map = $crate::lexer::TokenMap::new();
            $( $entry! { map, $($args)* } )*;
            println!("{:#?}", map);
            let mut out = Vec::new();
            let tree = $crate::parser::parse(&$crate::util::SourceBuilder::new()
                .lines($source)
                .build(),
                map,
                &mut out,
            );
            (tree, String::from_utf8(out).expect("Non UTF-8 parser output!"))
        }}
    }
    #[macro_export]
    macro_rules! stmt {
        ($span:expr, $kind:ident $($args:tt)+) => {
            $crate::parser::Statement {
                span: $span,
                kind: $crate::parser::StatementKind::$kind $($args)+
            }
        };
    }
    #[macro_export]
    macro_rules! expr {
        ($span:expr, $kind:ident $($args:tt)+) => {
            $crate::parser::Expression {
                span: $span,
                kind: $crate::parser::ExpressionKind::$kind $($args)+
            }
        };
    }
    #[macro_export]
    macro_rules! var {
        ($span:expr, $ident:expr) => {
            $crate::parser::Variable {
                span: $span,
                identifier: String::from($ident),
            }
        };
    }
    #[macro_export]
    macro_rules! expr_var {
        ($span:expr, $ident:expr) => {
            $crate::expr!($span, Variable($crate::var!($span, $ident)));
        };
    }
    #[macro_export]
    macro_rules! con {
        ($span:expr, $value:expr) => {
            $crate::parser::Constant {
                span: $span,
                value: $value,
            }
        };
    }
    #[macro_export]
    macro_rules! expr_con {
        ($span:expr, $value:expr) => {
            $crate::expr!($span, Constant($crate::con!($span, $value)));
        };
    }
    #[macro_export]
    macro_rules! binary_op {
        ($span:expr, $op:ident, $lhs:expr, $rhs:expr) => {
            $crate::expr!(
                $span,
                BinaryOp {
                    kind: $crate::parser::BinaryOpKind::$op,
                    lhs: Rc::new($lhs),
                    rhs: Rc::new($rhs),
                }
            )
        };
    }
    #[macro_export]
    macro_rules! assert_empty {
        ($out:expr) => {
            assert_eq!(String::from(""), $out)
        };
    }
}
