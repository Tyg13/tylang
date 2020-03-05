use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub span: Span,
    pub identifier: String,
}

impl std::fmt::Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Variable({span} {identifier})",
            span = self.span,
            identifier = self.identifier,
        )
    }
}

impl From<Variable> for Expression {
    fn from(var: Variable) -> Self {
        Self {
            span: var.span,
            kind: ExpressionKind::Variable(var),
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_variable(&mut self) -> Result<Variable> {
        let token = self.expect(TokenKind::Identifier)?;
        let span = token.span;
        let identifier = self.ident(token)?;
        Ok(Variable { span, identifier })
    }
}

#[cfg(test)]
mod tests {
    #[macro_export]
    macro_rules! var {
        ($span:expr, $ident:expr) => {
            $crate::parser::Variable {
                span: $span,
                identifier: String::from($ident),
            }
        };
    }
}
