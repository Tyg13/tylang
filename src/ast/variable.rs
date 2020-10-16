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

impl Parse for Variable {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let identifier = parser
            .expect(TokenKind::Identifier)
            .map(|token| token.as_ident().unwrap())?;
        Some(Variable { identifier })
    }
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;

    pub fn var(name: &str) -> Variable {
        let identifier = name.to_string();
        Variable { identifier }
    }
}
