use super::*;

pub struct Function {
    pub span: Span,
    pub identifier: String,
    pub parameters: Vec<Parameter>,
    pub body: Option<Scope>,
    pub return_type: Type,
}

pub struct Parameter {
    pub span: Span,
    pub variable: Variable,
    pub type_: Type,
}

impl Parser<'_> {
    pub(super) fn parse_function(&mut self) -> Result<Function> {
        let _fn = self.expect(TokenKind::Fn)?;
        let identifier = self
            .expect(TokenKind::Identifier)
            .and_then(|token| self.ident(token))?;
        let parameters = self.parse_parameters()?;
        let _arrow = self.expect(TokenKind::Arrow)?;
        let return_type = self.parse_type()?;
        let (span, body) = match self.peek()?.kind {
            TokenKind::SemiColon => {
                let semi = self.advance()?;
                (span!(_fn, semi), None)
            }
            TokenKind::LeftBrace => {
                let scope = self.parse_scope()?;
                (span!(_fn, scope), Some(scope))
            }
            _ => return Err(Error::UnexpectedToken(format!("`;` or `{{`"))),
        };
        Ok(Function {
            span,
            identifier,
            parameters,
            body,
            return_type,
        })
    }

    fn parse_parameters(&mut self) -> Result<Vec<Parameter>> {
        let _left_paren = self.expect(TokenKind::LeftParen)?;
        let mut parameters = Vec::new();
        loop {
            match self.peek()?.kind {
                TokenKind::RightParen => {
                    self.advance()?;
                    return Ok(parameters);
                }
                _ => {
                    let variable = self.parse_variable()?;
                    let _colon = self.expect(TokenKind::Colon)?;
                    let type_ = self.parse_type()?;
                    parameters.push(Parameter {
                        span: span!(variable, type_),
                        variable,
                        type_,
                    });
                    self.maybe(TokenKind::Comma);
                }
            }
        }
    }
}
