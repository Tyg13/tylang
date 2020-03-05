use crate::lexer::{Token, TokenKind, TokenMap};
use crate::span;
use crate::util::{ArmPosition, Source, Span};
use std::rc::Rc;

mod constant;
mod error;
mod expression;
mod function;
mod statement;
mod r#type;
mod variable;
pub mod visit;
pub use constant::*;
use error::*;
pub use expression::*;
pub use function::*;
pub use r#type::*;
pub use statement::*;
pub use variable::*;
pub use visit::Visitor;

#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub statements: Vec<Statement>,
    pub span: Span,
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
    Parser::new(source, map, out).parse()
}

type Result<T> = std::result::Result<T, Error>;

pub struct Parser<'a> {
    source: &'a Source,
    out: &'a mut dyn std::io::Write,
    map: TokenMap,
    index: usize,
    backtrack_index: usize,
    any_errors: bool,
}

pub struct Tree {
    pub functions: Vec<Function>,
    any_errors: bool,
}

impl Tree {
    pub fn valid(&self) -> bool {
        !self.any_errors
    }
}

impl<'a> Parser<'a> {
    fn new(source: &'a Source, map: TokenMap, out: &'a mut dyn std::io::Write) -> Self {
        Self {
            source,
            out,
            map,
            index: 0,
            backtrack_index: 0,
            any_errors: false,
        }
    }

    fn parse(mut self) -> Tree {
        let mut functions = Vec::new();
        while let Ok(_) = self.peek() {
            match self.parse_function() {
                Ok(function) => functions.push(function),
                Err(err) => {
                    self.backtrack();
                    self.report_err(err);
                    if let Err(Error::EOF) = self.advance_until(TokenKind::RightBrace) {
                        break;
                    }
                }
            }
        }
        Tree {
            functions,
            any_errors: self.any_errors,
        }
    }

    fn peek(&mut self) -> Result<Token> {
        if self.index >= self.map.len() {
            return Err(Error::EOF);
        }
        self.sync();
        Ok(self.map.token(self.index))
    }

    fn advance(&mut self) -> Result<Token> {
        let token = self.peek()?;
        self.index = self
            .index
            .checked_add(1)
            .expect("Overflow in parser token index");
        Ok(token)
    }

    fn advance_until(&mut self, kind: TokenKind) -> Result<Token> {
        loop {
            match self.advance() {
                Ok(token) => {
                    if token.kind == kind {
                        return Ok(token);
                    }
                }
                eof @ Err(Error::EOF) => return eof,
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
            return Err(Error::UnexpectedToken(kind.to_string()));
        }
        Ok(token)
    }

    fn ident(&self, token: Token) -> Result<String> {
        self.map
            .idents
            .get(&token.id)
            .cloned()
            .ok_or(Error::Internal(format!(
                "{:#?} is not an identifier",
                token
            )))
    }

    fn number(&self, token: Token) -> Result<usize> {
        self.map
            .numbers
            .get(&token.id)
            .cloned()
            .ok_or(Error::Internal(format!("{:#?} is not a number", token)))
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
    macro_rules! tree {
        [$source:expr, $($entry:ident { $($args:tt)* },)*] => {{
            let mut map = $crate::lexer::TokenMap::new();
            macro_rules! token {
                { $span:expr, $kind:ident } => {
                    map.add_token(TokenKind::$kind, $span);
                }
            }
            #[allow(unused_macros)]
            macro_rules! identifier {
                { $span:expr, $name:expr } => {
                    map.add_ident(String::from($name), $span);
                }
            }
            #[allow(unused_macros)]
            macro_rules! number {
                { $span:expr, $value:expr } => {
                    map.add_number($value, $span);
                }
            }
            $( $entry! { $($args)* } )*;
            let mut out = Vec::new();
            let tree = $crate::parser::parse(&$crate::util::SourceBuilder::new()
                .source($source)
                .build(),
                map,
                &mut out,
            );
            (tree, String::from_utf8(out).expect("Non UTF-8 parser output!"))
        }}
    }
    #[macro_export]
    macro_rules! assert_empty {
        ($out:expr) => {
            assert_eq!(String::from(""), $out)
        };
    }

    #[macro_export]
    macro_rules! function {
        ($func_name:literal, $body:literal, $($entry:ident { $span:expr, $($args:tt)* },)*) => {{
            let input = concat!("fn ", $func_name, " {\n", $body, "\n}");
            let identifier_len = $func_name.len();
            let last_line = [$($span,)*].max_by_key(|span| span.end.line).unwrap_or(3);
            tree! [
                input,
                token      { span!(1:01                     , 1:03                     ), Fn         },
                identifier { span!(1:04                     , 1:04 + identifier_len    ), $func_name },
                token      { span!(1:04 + identifier_len + 1, 1:04 + identifier_len + 2), LeftParen  },
                $(
                    $entry {
                        span!(
                            $span.start.line + 1 : $span.start.column,
                            $span.end  .line + 1 : $span.end.column
                        )
                        $($args)*
                    },
                )*
                token      { span!(last_line:01, last_line:02), LeftParen  },
            ]
        }};
    }
}
