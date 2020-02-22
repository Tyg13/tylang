use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub span: Span,
    pub value: usize,
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Constant({span} {value})",
            span = self.span,
            value = self.value
        )
    }
}

impl From<Constant> for Expression {
    fn from(cons: Constant) -> Self {
        Self {
            span: cons.span,
            kind: ExpressionKind::Constant(cons),
        }
    }
}

impl Parser<'_> {
    fn number(&self, token: Token) -> Result<usize> {
        self.map
            .numbers
            .get(&token.id)
            .cloned()
            .ok_or(Error::Internal(format!("{:#?} is not a number", token)))
    }
    pub fn parse_constant(&mut self) -> Result<Constant> {
        let token = self.expect(TokenKind::Number)?;
        let span = token.span;
        let value = self.number(token)?;
        Ok(Constant { span, value })
    }
}
