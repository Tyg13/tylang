use super::*;
use crate::lexer::TokenKind;

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

impl Parse for Scope {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let mut statements = Vec::new();
        parser.expect(TokenKind::LeftBrace)?;
        while parser.maybe(TokenKind::RightBrace).is_none() {
            if parser.eof() {
                return None;
            }
            let statement = parser.parse_one().unwrap_or_else(|| {
                parser.advance_past_one_of(&[TokenKind::RightBrace, TokenKind::SemiColon]);
                Statement::error()
            });
            statements.push(statement);
        }
        Some(Scope { statements })
    }
}
