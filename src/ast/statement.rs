use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Declaration {
        var: Variable,
        type_: Type,
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
        block: Rc<Scope>,
    },
    Null,
}

impl std::fmt::Display for StatementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use StatementKind::*;
        match self {
            Declaration {
                var,
                type_,
                initializer,
            } => match initializer {
                Some(expr) => write!(f, "{}: {} = {}", var, type_, expr),
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
    pub(super) fn parse_scope(&mut self) -> Result<Scope> {
        let left_brace = self.expect(TokenKind::LeftBrace)?;
        let mut statements = Vec::new();
        let right_brace = loop {
            let token = self.peek()?;
            match token.kind {
                TokenKind::RightBrace => {
                    self.advance()?;
                    break token;
                }
                _ => {
                    match self.parse_statement() {
                        Ok(statement) => statements.push(statement),
                        Err(err) => {
                            self.backtrack();
                            self.report_err(err);
                            self.advance_until(TokenKind::SemiColon)?;
                        }
                    };
                }
            }
        };
        Ok(Scope {
            statements,
            span: span!(left_brace, right_brace),
        })
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        let token = self.peek()?;
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
    }

    fn parse_declaration(&mut self) -> Result<Statement> {
        let let_keyword = self.expect(TokenKind::Let)?;
        let var = self.parse_variable()?;
        let _colon = self.expect(TokenKind::Colon)?;
        let type_ = self.parse_type()?;
        let initializer = if self.maybe(TokenKind::Equals).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(let_keyword, semi),
            kind: StatementKind::Declaration {
                var,
                type_,
                initializer,
            },
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

    fn parse_local_scope(&mut self) -> Result<Statement> {
        self.parse_scope().map(|scope| Statement {
            span: scope.span,
            kind: StatementKind::Scope(scope),
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
        let block = Rc::new(self.parse_scope()?);
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
    use crate::{assert_empty, expr_con, expr_var, function, span, stmt, tree, var};

    #[test]
    fn declaration() {
        let (tree, stdout) = function![
            "foo",
            "let x = 10;",
            token      { span!(1:01, 1:04), Let       },
            identifier { span!(1:05, 1:06), "x"       },
            token      { span!(1:07, 1:08), Equals    },
            number     { span!(1:09, 1:11), 10        },
            token      { span!(1:11, 1:12), SemiColon },
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
            "fn a() { let x;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.functions[0].body.unwrap().statements,
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
            "fn a() { x = x; }",
            token      { span!(1:01, 1:03), Fn         },
            identifier { span!(1:04, 1:05), "a"        },
            token      { span!(1:05, 1:06), LeftParen  },
            token      { span!(1:06, 1:07), RightParen },
            token      { span!(1:08, 1:09), LeftBrace  },
            identifier { span!(1:10, 1:11), "x"        },
            token      { span!(1:12, 1:13), Equals     },
            identifier { span!(1:14, 1:15), "x"        },
            token      { span!(1:15, 1:16), SemiColon  },
            token      { span!(1:17, 1:18), RightBrace },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.functions[0].body.unwrap().statements,
            vec![stmt! {
                span!(1:10, 1:16),
                Assignment {
                    dst: expr_var! {
                        span!(1:10, 1:11),
                        "x"
                    },
                    src: expr_var! {
                        span!(1:14, 1:15),
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
            $crate::ast::Statement {
                span: $span,
                kind: $crate::ast::StatementKind::$kind $($args)+
            }
        };
    }
}
