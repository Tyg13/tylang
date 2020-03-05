use super::*;
use std::convert::TryFrom;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
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
    Call {
        name: String,
        arguments: Vec<Expression>,
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
            Call { name, arguments } => write!(
                f,
                "{}({})",
                name,
                arguments
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
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

impl TryFrom<TokenKind> for BinaryOp {
    type Error = ();
    fn try_from(kind: TokenKind) -> std::result::Result<Self, Self::Error> {
        use BinaryOpKind::*;
        Ok(match kind {
            TokenKind::LeftAngle => BinaryOp {
                kind: Lt,
                precedence: 1,
            },
            TokenKind::LeftAngleEquals => BinaryOp {
                kind: Lte,
                precedence: 1,
            },
            TokenKind::RightAngle => BinaryOp {
                kind: Gt,
                precedence: 1,
            },
            TokenKind::RightAngleEquals => BinaryOp {
                kind: Gte,
                precedence: 1,
            },
            TokenKind::EqualsEquals => BinaryOp {
                kind: Eq,
                precedence: 1,
            },
            TokenKind::Plus => BinaryOp {
                kind: Add,
                precedence: 2,
            },
            TokenKind::Minus => BinaryOp {
                kind: Sub,
                precedence: 2,
            },
            TokenKind::Star => BinaryOp {
                kind: Mul,
                precedence: 3,
            },
            TokenKind::Slash => BinaryOp {
                kind: Div,
                precedence: 3,
            },
            _ => return Err(()),
        })
    }
}

enum Precedence {
    Higher,
    Lower,
}

struct Context {
    precedence: usize,
}

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_expression_in_context(&mut Context { precedence: 0 })
    }

    fn parse_expression_in_context(&mut self, context: &mut Context) -> Result<Expression> {
        let expr = match self.peek()?.kind {
            TokenKind::LeftParen => {
                let left_paren = self.expect(TokenKind::LeftParen)?;
                let expr = self.parse_expression()?;
                let right_paren = self.expect(TokenKind::RightParen)?;
                Expression {
                    span: span!(left_paren, right_paren),
                    kind: ExpressionKind::Group(Rc::new(expr)),
                }
            }
            TokenKind::Identifier => {
                let variable = self.expect(TokenKind::Identifier)?;
                match self.peek()?.kind {
                    TokenKind::LeftParen => self.parse_call(variable)?,
                    _ => {
                        let identifier = self.ident(variable)?;
                        Expression::from(Variable {
                            span: variable.span,
                            identifier,
                        })
                    }
                }
            }
            TokenKind::Number => self.parse_constant().map(Expression::from)?,
            _ => {
                return Err(Error::UnexpectedToken(String::from("expression")));
            }
        };
        Ok(self.maybe_parse_binary_op_of_precedence(expr, Precedence::Higher, context)?)
    }

    fn maybe_parse_binary_op_of_precedence(
        &mut self,
        expr: Expression,
        kind: Precedence,
        context: &mut Context,
    ) -> Result<Expression> {
        if let Ok(op) = BinaryOp::try_from(self.peek()?.kind) {
            let precedence = context.precedence;
            if match kind {
                Precedence::Higher => op.precedence >= context.precedence,
                Precedence::Lower => op.precedence < context.precedence,
            } {
                context.precedence = op.precedence;
                return Ok(self.parse_binary_op(expr, op.kind, context)?);
            }
            context.precedence = precedence;
        }
        Ok(expr)
    }

    fn parse_binary_op(
        &mut self,
        lhs: Expression,
        kind: BinaryOpKind,
        context: &mut Context,
    ) -> Result<Expression> {
        let _ = self.advance()?;
        let lhs = Rc::new(lhs);
        let rhs = Rc::new(self.parse_expression_in_context(context)?);
        let mut expr = Expression {
            span: span!(lhs, rhs),
            kind: ExpressionKind::BinaryOp { kind, lhs, rhs },
        };
        expr = self.maybe_parse_binary_op_of_precedence(expr, Precedence::Lower, context)?;
        Ok(expr)
    }

    fn parse_call(&mut self, func: Token) -> Result<Expression> {
        let _left_paren = self.expect(TokenKind::LeftParen)?;
        let right_paren;
        let arguments = {
            let mut arguments = Vec::new();
            loop {
                match self.peek()?.kind {
                    TokenKind::RightParen => {
                        right_paren = self.advance()?;
                        break arguments;
                    }
                    _ => arguments.push(self.parse_expression()?),
                }
            }
        };
        let name = self.ident(func)?;
        Ok(Expression {
            span: span!(func, right_paren),
            kind: ExpressionKind::Call { name, arguments },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_empty, binary_op, expr, expr_con, expr_var, span, stmt, tree};
    use pretty_assertions::assert_eq;

    #[test]
    fn binary_op() {
        let (tree, stdout) = tree![
            "x = 1 + 1;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Equals,    span!(1:03, 1:04) },
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
            token      { Equals,    span!(1:03, 1:04) },
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
            token      { Equals,    span!(1:03, 1:04) },
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
            token      { Equals,     span!(1:03, 1:04) },
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

    #[macro_export]
    macro_rules! expr {
        ($span:expr, $kind:ident $($args:tt)+) => {
            $crate::parser::Expression {
                span: $span,
                kind: $crate::parser::ExpressionKind::$kind $($args)+
            }
        };
    }
    #[macro_export]
    macro_rules! expr_var {
        ($span:expr, $ident:expr) => {
            $crate::expr!($span, Variable($crate::var!($span, $ident)));
        };
    }
    #[macro_export]
    macro_rules! expr_con {
        ($span:expr, $value:expr) => {
            $crate::expr!($span, Constant($crate::con!($span, $value)));
        };
    }
    #[macro_export]
    macro_rules! binary_op {
        ($span:expr, $op:ident, $lhs:expr, $rhs:expr) => {
            $crate::expr!(
                $span,
                BinaryOp {
                    kind: $crate::parser::BinaryOpKind::$op,
                    lhs: Rc::new($lhs),
                    rhs: Rc::new($rhs),
                }
            )
        };
    }
}
