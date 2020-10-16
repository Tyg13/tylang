use std::convert::TryFrom;
use std::rc::Rc;

use crate::ast::{Constant, Parse, Parser, Variable};
use crate::lexer::TokenKind;

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
    Error,
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
            Error => write!(f, "(<err>)"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
}

impl Expression {
    pub(super) fn new(kind: ExpressionKind) -> Self {
        Self { kind }
    }

    fn error() -> Self {
        Self {
            kind: ExpressionKind::Error,
        }
    }

    pub fn is_error(&self) -> bool {
        self.kind == ExpressionKind::Error
    }
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

#[derive(Debug)]
struct Context {
    precedence: usize,
}

impl Context {
    fn with_precedence<T>(&mut self, precedence: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let old_precedence = self.precedence;
        self.precedence = precedence;
        let ret = f(self);
        self.precedence = old_precedence;
        ret
    }
}

impl Parse for Expression {
    fn parse(parser: &mut Parser) -> Option<Self> {
        parse_expression_in_context(parser, &mut Context { precedence: 0 })
    }
}

fn parse_expression_in_context(parser: &mut Parser, context: &mut Context) -> Option<Expression> {
    let expr = match parser.peek()?.kind() {
        TokenKind::LeftParen => {
            let inner = parser
                .some_or_backtrack(|parser| {
                    let expr = parser.parse_one()?;
                    parser.expect(TokenKind::RightParen)?;
                    Some(expr)
                })
                .unwrap_or_else(|| {
                    parser.advance_past(TokenKind::RightParen);
                    Expression::error()
                });
            Expression::new(ExpressionKind::Group(Rc::new(inner)))
        }
        TokenKind::Identifier => {
            let identifier = parser
                .expect(TokenKind::Identifier)
                .map(|token| token.repr())?;
            if parser.maybe(TokenKind::LeftParen).is_some() {
                let arguments = parse_call(parser);
                Expression::new(ExpressionKind::Call {
                    name: identifier,
                    arguments,
                })
            } else {
                Expression::from(Variable { identifier })
            }
        }
        TokenKind::Number => parser.parse_one::<Constant>().map(Expression::from)?,
        _ => {
            parser.issue_diagnostic("Expected expression");
            Expression::error()
        }
    };
    maybe_parse_binary_op_of_precedence(parser, expr, Precedence::Higher, context)
}

fn maybe_parse_binary_op_of_precedence(
    parser: &mut Parser,
    expr: Expression,
    prec: Precedence,
    context: &mut Context,
) -> Option<Expression> {
    let kind = parser.peek()?.kind();
    if let Ok(op) = BinaryOp::try_from(kind) {
        let should_parse = match prec {
            Precedence::Higher => op.precedence >= context.precedence,
            Precedence::Lower => op.precedence < context.precedence,
        };
        if should_parse {
            return context.with_precedence(op.precedence, |context| {
                parse_binary_op(parser, expr, op.kind, context)
            });
        }
    }
    Some(expr)
}

fn parse_binary_op(
    parser: &mut Parser,
    lhs: Expression,
    kind: BinaryOpKind,
    context: &mut Context,
) -> Option<Expression> {
    parser.advance();
    let lhs = Rc::new(lhs);
    let rhs = Rc::new(parse_expression_in_context(parser, context)?);
    let expr = Expression::new(ExpressionKind::BinaryOp { kind, lhs, rhs });
    maybe_parse_binary_op_of_precedence(parser, expr, Precedence::Lower, context)
}

fn parse_call(parser: &mut Parser) -> Vec<Expression> {
    let mut arguments = Vec::new();
    while parser.maybe(TokenKind::RightParen).is_none() {
        arguments.push(parser.parse_one().unwrap_or(Expression::error()));
        if parser.maybe(TokenKind::Comma).is_none() {
            parser.expect(TokenKind::RightParen);
            break;
        }
    }
    arguments
}

#[cfg(test)]
pub mod test {
    use crate::ast::*;
    use std::rc::Rc;

    pub fn unsigned(value: usize) -> Expression {
        Expression::from(Constant {
            value,
            parity: Parity::Unsigned,
        })
    }

    pub fn call(name: &str, args: &[Expression]) -> Expression {
        Expression {
            kind: ExpressionKind::Call {
                name: name.to_string(),
                arguments: args.to_vec(),
            },
        }
    }

    pub fn expr<T: Into<Expression>>(val: T) -> Expression {
        val.into()
    }

    impl std::ops::Add for Expression {
        type Output = Self;
        fn add(self, other: Self) -> Self {
            Self {
                kind: ExpressionKind::BinaryOp {
                    kind: BinaryOpKind::Add,
                    lhs: Rc::new(self),
                    rhs: Rc::new(other),
                },
            }
        }
    }

    pub fn gt(lhs: Expression, rhs: Expression) -> Expression {
        Expression {
            kind: ExpressionKind::BinaryOp {
                kind: BinaryOpKind::Gt,
                lhs: Rc::new(lhs),
                rhs: Rc::new(rhs),
            },
        }
    }
}
