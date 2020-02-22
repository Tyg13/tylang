use super::*;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ExpressionKind {
    Variable(Variable),
    Constant(Constant),
    Group(Rc<Expression>),
    BinaryOp {
        kind: BinaryOpKind,
        lhs: Rc<Expression>,
        rhs: Rc<Expression>,
    },
}

impl std::fmt::Display for ExpressionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ExpressionKind::*;
        match self {
            Variable(var) => write!(f, "{}", var),
            Constant(con) => write!(f, "{}", con),
            Group(expr) => write!(f, "{}", expr),
            BinaryOp { kind, lhs, rhs } => write!(
                f,
                "{kind:?}({lhs}, {rhs})",
                kind = kind,
                lhs = *lhs,
                rhs = *rhs
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub span: Span,
    pub kind: ExpressionKind,
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Expression({span} {kind})",
            span = self.span,
            kind = self.kind,
        )
    }
}

struct BinaryOp {
    kind: BinaryOpKind,
    precedence: usize,
}

impl BinaryOp {
    fn from(kind: TokenKind) -> Option<BinaryOp> {
        use BinaryOpKind::*;
        Some(match kind {
            TokenKind::Plus => BinaryOp {
                kind: Add,
                precedence: 1,
            },
            TokenKind::Minus => BinaryOp {
                kind: Sub,
                precedence: 1,
            },
            TokenKind::Star => BinaryOp {
                kind: Mul,
                precedence: 2,
            },
            TokenKind::Slash => BinaryOp {
                kind: Div,
                precedence: 2,
            },
            _ => return None,
        })
    }
}

enum Precedence {
    Higher,
    Lower,
}

impl Parser<'_> {
    fn maybe_parse_binary_op_of_precedence(
        &mut self,
        expr: Expression,
        kind: Precedence,
    ) -> Result<Expression> {
        let token = self.peek()?;
        if let Some(op) = BinaryOp::from(token.kind) {
            let precedence = self.precedence;
            let parse_op = match kind {
                Precedence::Higher => op.precedence >= self.precedence,
                Precedence::Lower => op.precedence < self.precedence,
            };
            if parse_op {
                self.precedence = op.precedence;
                return Ok(self.parse_binary_op(expr, op.kind)?);
            }
            self.precedence = precedence;
        }
        Ok(expr)
    }

    pub fn parse_expression(&mut self) -> Result<Expression> {
        let token = self.peek()?;
        let expr = match token.kind {
            TokenKind::LeftParen => {
                let precedence = self.precedence;
                self.precedence = 0;
                let left_paren = self.expect(TokenKind::LeftParen)?;
                let expr = self.parse_expression()?;
                let right_paren = self.expect(TokenKind::RightParen)?;
                self.precedence = precedence;
                Expression {
                    span: span!(left_paren.span.start, right_paren.span.end),
                    kind: ExpressionKind::Group(Rc::new(expr)),
                }
            }
            TokenKind::Identifier => self.parse_variable().map(Expression::from)?,
            TokenKind::Number => self.parse_constant().map(Expression::from)?,
            _ => {
                return Err(Error::UnexpectedToken {
                    expected: String::from("expression"),
                    unexpected: token,
                });
            }
        };
        Ok(self.maybe_parse_binary_op_of_precedence(expr, Precedence::Higher)?)
    }

    fn parse_binary_op(&mut self, lhs: Expression, kind: BinaryOpKind) -> Result<Expression> {
        let _ = self.advance()?;
        let lhs = Rc::new(lhs);
        let rhs = Rc::new(self.parse_expression()?);
        let mut expr = Expression {
            span: span!(lhs.span.start, rhs.span.end),
            kind: ExpressionKind::BinaryOp { kind, lhs, rhs },
        };
        expr = self.maybe_parse_binary_op_of_precedence(expr, Precedence::Lower)?;
        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assert_empty, binary_op, expr, expr_con, expr_var, identifier, number, span, stmt, token,
        tree,
    };

    #[test]
    fn binary_op() {
        let (tree, stdout) = tree![
            "x = 1 + 1;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            number     { 1,         span!(1:05, 1:06) },
            token      { Plus,      span!(1:07, 1:08) },
            number     { 1,         span!(1:09, 1:10) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:11),
                Assignment {
                    dst: expr_var! {
                        span!(1:01, 1:02),
                        "x"
                    },
                    src: binary_op! {
                        span!(1:05, 1:10),
                        Add,
                        expr_con! {
                            span!(1:05, 1:06),
                            1
                        },
                        expr_con! {
                            span!(1:09, 1:10),
                            1
                        }
                    }
                }
            }],
            "Basic addition"
        );
    }

    #[test]
    fn precedence() {
        let (tree, stdout) = tree![
            "x = 2 + 2*2;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            number     { 2,         span!(1:05, 1:06) },
            token      { Plus,      span!(1:07, 1:08) },
            number     { 2,         span!(1:09, 1:10) },
            token      { Star,      span!(1:10, 1:11) },
            number     { 2,         span!(1:11, 1:12) },
            token      { SemiColon, span!(1:12, 1:13) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:13),
                Assignment {
                    dst: expr_var! {
                         span!(1:01, 1:02),
                         "x"
                    },
                    src: binary_op! {
                        span!(1:05, 1:12),
                        Add,
                        expr_con!(span!(1:05, 1:06), 2),
                        binary_op! {
                            span!(1:09, 1:12),
                            Mul,
                            expr_con!(span!(1:09, 1:10), 2),
                            expr_con!(span!(1:11, 1:12), 2)
                        }
                    }
                }
            }],
            "2 + 2 * 2 groups as 2 + (2 * 2)"
        );
        let (tree, stdout) = tree![
            "x = 2 * 2+2;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            number     { 2,         span!(1:05, 1:06) },
            token      { Star,      span!(1:07, 1:08) },
            number     { 2,         span!(1:09, 1:10) },
            token      { Plus,      span!(1:10, 1:11) },
            number     { 2,         span!(1:11, 1:12) },
            token      { SemiColon, span!(1:12, 1:13) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:13),
                Assignment {
                    dst: expr_var! {
                         span!(1:01, 1:02),
                         "x"
                    },
                    src: binary_op! {
                        span!(1:05, 1:12),
                        Add,
                        binary_op! {
                            span!(1:05, 1:10),
                            Mul,
                            expr_con!(span!(1:05, 1:06), 2),
                            expr_con!(span!(1:09, 1:10), 2)
                        },
                        expr_con!(span!(1:11, 1:12), 2)
                    }
                }
            }],
            "2 * 2 + 2 groups as (2 * 2) + 2"
        );
        let (tree, stdout) = tree![
            "x = (2 + 2)*2;",
            identifier { "x",        span!(1:01, 1:02) },
            token      { Assign,     span!(1:03, 1:04) },
            token      { LeftParen,  span!(1:05, 1:06) },
            number     { 2,          span!(1:06, 1:07) },
            token      { Plus,       span!(1:08, 1:09) },
            number     { 2,          span!(1:10, 1:11) },
            token      { RightParen, span!(1:11, 1:12) },
            token      { Star,       span!(1:12, 1:13) },
            number     { 2,          span!(1:13, 1:14) },
            token      { SemiColon,  span!(1:14, 1:15) },
        ];
        assert_empty!(stdout);
        assert_eq!(
            tree.statements,
            vec![stmt! {
                span!(1:01, 1:15),
                Assignment {
                    dst: expr_var! {
                         span!(1:01, 1:02),
                         "x"
                    },
                    src: binary_op! {
                        span!(1:05, 1:14),
                        Mul,
                        expr! {
                            span!(1:05, 1:12),
                            Group(Rc::new(binary_op! {
                                span!(1:06, 1:11),
                                Add,
                                expr_con!(span!(1:06, 1:07), 2),
                                expr_con!(span!(1:10, 1:11), 2)
                            }))
                        },
                        expr_con!(span!(1:13, 1:14), 2)
                    }
                }
            }],
            "(2 * 2) + 2 groups as expected"
        );
    }
}
