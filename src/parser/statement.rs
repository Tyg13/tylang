use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Declaration {
        var: Variable,
        initializer: Option<Expression>,
    },
    Assignment {
        dst: Expression,
        src: Expression,
    },
    Return(Expression),
    Scope(Scope),
    Expression(Expression),
    If {
        condition: Rc<Expression>,
        block: Rc<Statement>,
    },
    Null,
}

impl std::fmt::Display for StatementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use StatementKind::*;
        match self {
            Declaration { var, initializer } => match initializer {
                Some(expr) => write!(f, "{} = {}", var, expr),
                None => write!(f, "{}", var),
            },
            Assignment { dst, src } => write!(f, "{} = {}", dst, src),
            Return(expr) => write!(f, "Return({})", expr),
            Scope(scope) => write!(f, "Scope({})", scope),
            Expression(expr) => write!(f, "StmtExpr({})", expr),
            Null => write!(f, "Null"),
            If { condition, block } => write!(f, "If({}, {})", condition, block),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Statement({span} {kind})",
            span = self.span,
            kind = self.kind
        )
    }
}

impl Parser<'_> {
    pub(super) fn parse_scope(&mut self) -> Scope {
        let mut any_errors = false;
        let mut statements = Vec::new();
        loop {
            match self.parse_statement() {
                Ok(Some(statement)) => statements.push(statement),
                Ok(None) => break,
                Err(err) => {
                    any_errors = true;
                    self.backtrack();
                    self.report_err(err);
                    if let None = self.advance_until(TokenKind::SemiColon) {
                        break;
                    }
                }
            };
        }
        // Hack to prevent interpreter running for now
        if any_errors {
            statements.clear();
        }
        Scope { statements }
    }

    fn parse_statement(&mut self) -> Result<Option<Statement>> {
        let token = match self.peek() {
            Ok(token) => token,
            Err(Error::EOF) => return Ok(None),
            Err(err) => return Err(err),
        };
        match token.kind {
            TokenKind::Let => self.parse_declaration(),
            TokenKind::Return => self.parse_print(),
            TokenKind::Identifier => self.parse_assignment(),
            TokenKind::LeftBrace => self.parse_local_scope(),
            TokenKind::SemiColon => self.parse_null(),
            TokenKind::If => self.parse_if_statement(),
            _ => self
                .parse_expr_statement()
                .map_err(|_| Error::UnexpectedToken(String::from("statement"))),
        }
        .map(Some)
    }

    fn parse_declaration(&mut self) -> Result<Statement> {
        let let_keyword = self.expect(TokenKind::Let)?;
        let var = self.parse_variable()?;
        let initializer = if self.maybe(TokenKind::Equals).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(let_keyword, semi),
            kind: StatementKind::Declaration { var, initializer },
        })
    }

    fn parse_assignment(&mut self) -> Result<Statement> {
        let dst = self.parse_variable().map(Expression::from)?;
        self.expect(TokenKind::Equals)?;
        let src = self.parse_expression()?;
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(dst, semi),
            kind: StatementKind::Assignment { src, dst },
        })
    }

    fn parse_print(&mut self) -> Result<Statement> {
        let print = self.expect(TokenKind::Return)?;
        let expr = self.parse_expression()?;
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(print, semi),
            kind: StatementKind::Return(expr),
        })
    }

    pub(super) fn parse_local_scope(&mut self) -> Result<Statement> {
        let left_brace = self.expect(TokenKind::LeftBrace)?;
        let right_brace;
        let new_scope = {
            let mut new_scope = Scope::new();
            loop {
                if let Some(brace) = self.maybe(TokenKind::RightBrace) {
                    right_brace = brace;
                    break new_scope;
                }
                match self.parse_statement() {
                    Ok(Some(statement)) => new_scope.statements.push(statement),
                    Ok(None) => return Err(Error::EOF),
                    Err(e) => return Err(e),
                };
            }
        };
        Ok(Statement {
            span: span!(left_brace, right_brace),
            kind: StatementKind::Scope(new_scope),
        })
    }

    fn parse_expr_statement(&mut self) -> Result<Statement> {
        let expr = self.parse_expression()?;
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(expr, semi),
            kind: StatementKind::Expression(expr),
        })
    }

    fn parse_if_statement(&mut self) -> Result<Statement> {
        let _if = self.expect(TokenKind::If)?;
        let condition = Rc::new(self.parse_expression()?);
        let block = Rc::new(self.parse_local_scope()?);
        Ok(Statement {
            span: span!(_if, block),
            kind: StatementKind::If { condition, block },
        })
    }

    fn parse_null(&mut self) -> Result<Statement> {
        let token = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: token.span,
            kind: StatementKind::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_empty, expr_con, expr_var, span, stmt, tree, var};

    #[test]
    fn declaration() {
        let (tree, stdout) = tree![
            "let x = 10;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Equals,    span!(1:07, 1:08) },
            number     { 10,        span!(1:09, 1:11) },
            token      { SemiColon, span!(1:11, 1:12) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:12),
                Declaration {
                    var: var! {
                        span!(1:05, 1:06),
                        "x"
                    },
                    initializer: Some(expr_con! {
                        span!(1:09, 1:11),
                        10
                    })
                }
            }],
            "Declaration with constant initializer"
        );
        let (tree, stdout) = tree![
            "let x = x;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Equals,    span!(1:07, 1:08) },
            identifier { "x",       span!(1:08, 1:09) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:11),
                Declaration {
                    var: Variable {
                        span: span!(1:05, 1:06),
                        identifier: String::from("x")
                    },
                    initializer: Some(expr_var! {
                        span!(1:08, 1:09),
                        "x"
                    })
                }
            }],
            "Declaration with variable initializer"
        );
        let (tree, stdout) = tree![
            "let x;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:07),
                Declaration {
                    var: var!(span!(1:05, 1:06), "x"),
                    initializer: None,
                }
            }],
            "Declaration with no initializer"
        );
    }

    #[test]
    fn assignment() {
        let (tree, stdout) = tree![
            "x = x;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Equals,    span!(1:03, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:07),
                Assignment {
                    dst: expr_var! {
                        span!(1:01, 1:02),
                        "x"
                    },
                    src: expr_var! {
                        span!(1:05, 1:06),
                        "x"
                    }
                }
            }],
            "variable assignment to variable"
        );
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
}
