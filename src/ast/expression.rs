use super::*;
use crate::lexer::{TokenKind, TokenTree, TreeKind};
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
    pub kind: ExpressionKind,
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Expression({kind})", kind = self.kind,)
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

impl Parser {
    pub(super) fn parse_expression(&self, cursor: &mut Cursor) -> Option<Expression> {
        self.parse_expression_in_context(&mut Context { precedence: 0 }, cursor)
    }

    fn parse_expression_in_context(
        &self,
        context: &mut Context,
        cursor: &mut Cursor,
    ) -> Option<Expression> {
        let expr = match cursor.peek()? {
            TokenTree::Tree(tree) => {
                let mut cursor = Cursor::new(&tree.children);
                let inner = self.parse_expression(&mut cursor)?;
                Expression {
                    kind: ExpressionKind::Group(Rc::new(inner)),
                }
            }
            TokenTree::Token(token) => match token.kind {
                TokenKind::Identifier => {
                    let variable = cursor.expect_token(TokenKind::Identifier);
                    let identifier = self.map.ident(variable).cloned()?;
                    match cursor.peek()? {
                        TokenTree::Tree(tree) => {
                            if tree.kind != TreeKind::Parens {
                                panic!("Unexpected {} instead of call arguments", tree.kind);
                            }
                            let mut cursor = Cursor::new(&tree.children);
                            let arguments = self.parse_call(&mut cursor);
                            Expression {
                                kind: ExpressionKind::Call {
                                    name: identifier,
                                    arguments,
                                },
                            }
                        }
                        _ => Expression::from(Variable { identifier }),
                    }
                }
                TokenKind::Number => self.parse_constant(cursor).map(Expression::from)?,
                _ => return None,
            },
        };
        Some(self.maybe_parse_binary_op_of_precedence(expr, Precedence::Higher, context, cursor)?)
    }

    fn maybe_parse_binary_op_of_precedence(
        &self,
        expr: Expression,
        kind: Precedence,
        context: &mut Context,
        cursor: &mut Cursor,
    ) -> Option<Expression> {
        if let Ok(op) = BinaryOp::try_from(cursor.peek_token()?.kind) {
            let precedence = context.precedence;
            let should_parse = match kind {
                Precedence::Higher => op.precedence >= context.precedence,
                Precedence::Lower => op.precedence < context.precedence,
            };
            if should_parse {
                context.precedence = op.precedence;
                return Some(self.parse_binary_op(expr, op.kind, context, cursor)?);
            }
            context.precedence = precedence;
        }
        Some(expr)
    }

    fn parse_binary_op(
        &self,
        lhs: Expression,
        kind: BinaryOpKind,
        context: &mut Context,
        cursor: &mut Cursor,
    ) -> Option<Expression> {
        cursor.next();
        let lhs = Rc::new(lhs);
        let rhs = Rc::new(self.parse_expression_in_context(context, cursor)?);
        let mut expr = Expression {
            kind: ExpressionKind::BinaryOp { kind, lhs, rhs },
        };
        expr =
            self.maybe_parse_binary_op_of_precedence(expr, Precedence::Lower, context, cursor)?;
        Some(expr)
    }

    fn parse_call(&self, cursor: &mut Cursor) -> Vec<Expression> {
        let mut arguments = Vec::new();
        while let Some(expr) = self.parse_expression(cursor) {
            arguments.push(expr);
            cursor.maybe_token(TokenKind::Comma);
        }
        arguments
    }
}
