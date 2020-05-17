use super::*;
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub identifier: String,
}

impl std::fmt::Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Variable({identifier})", identifier = self.identifier,)
    }
}

impl From<Variable> for Expression {
    fn from(var: Variable) -> Self {
        Self {
            kind: ExpressionKind::Variable(var),
        }
    }
}

impl Parser {
    pub(super) fn parse_variable(&self, cursor: &mut Cursor) -> Option<Variable> {
        let token = cursor.expect_token(TokenKind::Identifier);
        let identifier = self
            .map
            .ident(&token)
            .cloned()
            .expect("Couldn't get ident for variable!");
        Some(Variable { identifier })
    }
}
