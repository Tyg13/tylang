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
    Print(Expression),
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
            Print(expr) => write!(f, "Print({})", expr),
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
            "Statement({span}) {kind}",
            span = self.span,
            kind = self.kind
        )
    }
}

impl Parser<'_> {
    pub fn parse_statements(&mut self) {
        let mut any_errors = false;
        loop {
            match self.parse_statement() {
                Ok(Some(statement)) => self.tree.statements.push(statement),
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
            self.tree.statements.clear();
        }
    }

    fn parse_statement(&mut self) -> Result<Option<Statement>> {
        let token = match self.peek() {
            Ok(token) => token,
            Err(Error::EOF) => return Ok(None),
            Err(err) => return Err(err),
        };
        match token.kind {
            TokenKind::Let => self.parse_declaration(),
            TokenKind::Print => self.parse_print(),
            TokenKind::Identifier => self.parse_assignment(),
            _ => {
                return Err(Error::UnexpectedToken {
                    expected: String::from("statement"),
                    unexpected: token,
                });
            }
        }
        .map(Some)
    }

    fn parse_declaration(&mut self) -> Result<Statement> {
        let let_keyword = self.expect(TokenKind::Let)?;
        let var = self.parse_variable()?;
        let initializer = if self.maybe(TokenKind::Assign).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(let_keyword.span.start, semi.span.end),
            kind: StatementKind::Declaration { var, initializer },
        })
    }

    fn parse_assignment(&mut self) -> Result<Statement> {
        let dst = self.parse_variable().map(Expression::from)?;
        self.expect(TokenKind::Assign)?;
        let src = self.parse_expression()?;
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(dst.span.start, semi.span.end),
            kind: StatementKind::Assignment { src, dst },
        })
    }

    fn parse_print(&mut self) -> Result<Statement> {
        let print = self.expect(TokenKind::Print)?;
        let expr = self.parse_expression()?;
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(print.span.start, semi.span.end),
            kind: StatementKind::Print(expr),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_empty, expr_con, expr_var, identifier, number, span, stmt, token, tree, var,
    };

    #[test]
    fn declaration() {
        let (tree, stdout) = tree![
            "let x = 10;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Assign,    span!(1:07, 1:08) },
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
            token      { Assign,    span!(1:07, 1:08) },
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
            token      { Assign,    span!(1:03, 1:04) },
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
}
