use crate::ast::{Item, ItemKind, Parse, Parser, Scope, Type, Variable};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub identifier: String,
    pub parameters: Vec<Parameter>,
    pub body: Option<Scope>,
    pub return_type: Type,
}

impl From<Function> for Item {
    fn from(f: Function) -> Self {
        Self {
            kind: ItemKind::Function(f),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub variable: Variable,
    pub type_: Type,
}

impl Parse for Function {
    fn parse(parser: &mut Parser) -> Option<Self> {
        log::debug!("function");
        parser.expect(TokenKind::Fn)?;
        let identifier = parser
            .expect(TokenKind::Identifier)
            .map(|token| token.repr())?;
        let parameters = parser.parse_one()?;
        parser.expect(TokenKind::MinusArrow)?;
        let return_type = parser.parse_one().unwrap_or_else(|| {
            parser.peek().map(|token| {
                if token.kind() != TokenKind::LeftBrace {
                    parser.advance();
                }
            });
            Type::error()
        });
        let body = if let Some(TokenKind::LeftBrace) = parser.peek().map(|token| token.kind()) {
            Some(parser.parse_one()?)
        } else {
            parser.expect(TokenKind::SemiColon);
            None
        };
        Some(Function {
            identifier,
            parameters,
            body,
            return_type,
        })
    }
}

impl Parse for Vec<Parameter> {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let mut parameters = Vec::new();
        parser.expect(TokenKind::LeftParen)?;
        while parser.maybe(TokenKind::RightParen).is_none() {
            let variable = parser.parse_one()?;
            parser.expect(TokenKind::Colon);
            let type_ = parser
                .parse_one()
                .expect("couldn't parse type in parameter");
            parameters.push(Parameter { variable, type_ });
            if parser.maybe(TokenKind::Comma).is_none() {
                if parser.maybe(TokenKind::RightParen).is_none() {
                    parser.issue_diagnostic("Missing comma in parameter list");
                    return None;
                }
                break;
            }
        }
        Some(parameters)
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::test::*;
    use crate::ast::*;

    pub fn function(
        name: &str,
        parameters: &[Parameter],
        return_type: Type,
        statements: Option<&[Statement]>,
    ) -> Function {
        let body = statements.map(|s| Scope {
            statements: s.to_vec(),
        });
        Function {
            identifier: name.to_string(),
            parameters: parameters.to_vec(),
            body,
            return_type,
        }
    }

    pub fn param(name: &str, type_: &str) -> Parameter {
        let type_ = ty(type_);
        let variable = var(name);
        Parameter { type_, variable }
    }

    #[test]
    fn empty() {
        assert_eq!(
            test::parse(
                "fn a(foo: i64, bar: i64) -> i32 {}\n\
                 fn a(foo: i64) -> i64;\
                 fn a() -> i32 {}"
            ),
            tree(&[
                item(function(
                    "a",
                    &[param("foo", "i64"), param("bar", "i64")],
                    ty("i32"),
                    Some(&[]),
                )),
                item(function("a", &[param("foo", "i64")], ty("i64"), None)),
                item(function("a", &[], ty("i32"), Some(&[])))
            ])
        );
    }

    #[test]
    fn declaration() {
        assert_eq!(
            test::parse("fn a(foo: i64, bar: i64) -> i8;"),
            tree(&[item(function(
                "a",
                &[param("foo", "i64"), param("bar", "i64")],
                ty("i8"),
                None,
            ))])
        );
    }
}
