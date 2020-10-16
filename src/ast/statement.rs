use std::rc::Rc;

use crate::ast::{Expression, Parse, Parser, Scope, Type, Variable};
use crate::lexer::TokenKind;

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub var: Variable,
    pub type_: Option<Type>,
    pub initializer: Option<Expression>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Declaration(Declaration),
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
    Error,
}

impl Statement {
    pub fn error() -> Statement {
        Statement {
            kind: StatementKind::Error,
        }
    }
    fn terminated_by_semicolon(&self) -> bool {
        match self.kind {
            StatementKind::If { .. } => false,
            _ => true,
        }
    }
}

impl std::fmt::Display for StatementKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format_type = |type_: &_| match type_ {
            Some(ty) => format!("{}", ty),
            None => String::from("?"),
        };
        match self {
            StatementKind::Declaration(Declaration {
                var,
                type_,
                initializer,
            }) => match initializer {
                Some(expr) => write!(f, "{}: {} = {}", var, format_type(type_), expr),
                None => write!(f, "{}", var),
            },
            StatementKind::Assignment { dst, src } => write!(f, "{} = {}", dst, src),
            StatementKind::Return(expr) => write!(f, "Return({})", expr),
            StatementKind::Scope(scope) => write!(f, "Scope({})", scope),
            StatementKind::Expression(expr) => write!(f, "StmtExpr({})", expr),
            StatementKind::Null => write!(f, "Null"),
            StatementKind::If { condition, block } => write!(f, "If({}, {})", condition, block),
            StatementKind::Error => write!(f, "<err>"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Statement({kind})", kind = self.kind)
    }
}

impl Parse for Statement {
    fn parse(parser: &mut Parser) -> Option<Self> {
        let statement = match parser.peek()?.kind() {
            TokenKind::LeftBrace => parse_local_scope(parser)?,
            TokenKind::Let => parse_declaration(parser)?,
            TokenKind::Return => parse_return(parser)?,
            TokenKind::Identifier => parse_assignment(parser)?,
            TokenKind::SemiColon => parse_null(parser)?,
            TokenKind::If => parse_if_statement(parser)?,
            _ => parse_expr_statement(parser)?,
        };
        if statement.terminated_by_semicolon() {
            parser.expect(TokenKind::SemiColon)?;
        }
        Some(statement)
    }
}

impl From<Expression> for Statement {
    fn from(expr: Expression) -> Self {
        Self {
            kind: StatementKind::Expression(expr),
        }
    }
}

impl From<Declaration> for Statement {
    fn from(decl: Declaration) -> Self {
        Self {
            kind: StatementKind::Declaration(decl),
        }
    }
}

impl Parse for Declaration {
    fn parse(parser: &mut Parser) -> Option<Self> {
        parser.expect(TokenKind::Let)?;
        let var = parser.parse_one()?;
        let type_ = if parser.maybe(TokenKind::Colon).is_some() {
            Some(parser.parse_one()?)
        } else {
            None
        };
        let initializer = if parser.maybe(TokenKind::Equals).is_some() {
            Some(parser.parse_one()?)
        } else {
            None
        };
        Some(Declaration {
            var,
            type_,
            initializer,
        })
    }
}

fn parse_local_scope(parser: &mut Parser) -> Option<Statement> {
    parser.expect(TokenKind::LeftBrace)?;
    parser
        .some_or_backtrack(|parser| {
            let scope = parser.parse_one().unwrap();
            parser.expect(TokenKind::RightBrace)?;
            Some(Statement {
                kind: StatementKind::Scope(scope),
            })
        })
        .or_else(|| {
            parser.advance_until(TokenKind::RightBrace);
            Some(Statement::error())
        })
}

fn parse_declaration(parser: &mut Parser) -> Option<Statement> {
    let decl = parser.parse_one()?;
    Some(Statement {
        kind: StatementKind::Declaration(decl),
    })
}

fn parse_assignment(parser: &mut Parser) -> Option<Statement> {
    log::debug!("assignment");
    let dst = parser.parse_one()?;
    parser.expect(TokenKind::Equals)?;
    let src = parser.parse_one()?;
    Some(Statement {
        kind: StatementKind::Assignment { src, dst },
    })
}

fn parse_return(parser: &mut Parser) -> Option<Statement> {
    log::debug!("return");
    parser.expect(TokenKind::Return)?;
    let expr = parser.parse_one()?;
    Some(Statement {
        kind: StatementKind::Return(expr),
    })
}

fn parse_expr_statement(parser: &mut Parser) -> Option<Statement> {
    log::debug!("expr statement");
    let expr = parser.parse_one()?;
    Some(Statement {
        kind: StatementKind::Expression(expr),
    })
}

fn parse_if_statement(parser: &mut Parser) -> Option<Statement> {
    log::debug!("if");
    parser.expect(TokenKind::If)?;
    let condition = Rc::new(parser.parse_one()?);
    let block = Rc::new(parser.parse_one().unwrap());
    Some(Statement {
        kind: StatementKind::If { condition, block },
    })
}

fn parse_null(_parser: &mut Parser) -> Option<Statement> {
    log::debug!("null");
    Some(Statement {
        kind: StatementKind::Null,
    })
}

#[cfg(test)]
pub mod test {
    use crate::ast::test::*;
    use crate::ast::*;
    use std::rc::Rc;

    pub fn decl(name: &str, type_: Option<Type>, expr: Option<Expression>) -> Statement {
        stmt(Declaration {
            var: var(name),
            type_,
            initializer: expr,
        })
    }

    pub fn assign(name: &str, val: Expression) -> Statement {
        Statement {
            kind: StatementKind::Assignment {
                dst: expr(var(name)),
                src: val,
            },
        }
    }

    pub fn stmt<T: Into<Statement>>(val: T) -> Statement {
        val.into()
    }

    pub fn if_(condition: Expression, stmts: &[Statement]) -> Statement {
        Statement {
            kind: StatementKind::If {
                condition: Rc::new(condition),
                block: Rc::new(Scope {
                    statements: stmts.to_vec(),
                }),
            },
        }
    }

    pub fn ret(expr: Expression) -> Statement {
        Statement {
            kind: StatementKind::Return(expr),
        }
    }

    #[test]
    pub fn declaration() {
        assert_eq!(
            decl("a", Some(ptr(ty("i64"))), None),
            test::parse_one("let a: *i64;")
        );
        assert_eq!(
            decl("n", Some(ty("i32")), Some(unsigned(10))),
            test::parse_one("let n: i32 = 10;")
        );
    }

    #[test]
    pub fn return_() {
        assert_eq!(ret(expr(var("a"))), test::parse_one("return a;"))
    }

    #[test]
    fn all() {
        assert_eq!(
            test::parse(
                "fn a(foo: i64, bar: i64) -> i64 {
                    let m: i64 = foo + bar;
                    let n = 10 + m;
                    return b(n, m + 1);
                }
                fn b(n: i64, m: i64) -> i32 {
                    if n > 10 {
                        n = 10;
                    }
                    return m;
                }"
            ),
            tree(&[
                item(function(
                    "a",
                    &[param("foo", "i64"), param("bar", "i64")],
                    ty("i64"),
                    Some(&[
                        decl(
                            "m",
                            Some(ty("i64")),
                            Some(expr(var("foo")) + expr(var("bar"))),
                        ),
                        decl("n", None, Some(unsigned(10) + expr(var("m")))),
                        ret(call("b", &[expr(var("n")), expr(var("m")) + unsigned(1)]))
                    ]),
                )),
                item(function(
                    "b",
                    &[param("n", "i64"), param("m", "i64")],
                    ty("i32"),
                    Some(&[
                        if_(
                            gt(expr(var("n")), unsigned(10)),
                            &[assign("n", unsigned(10))]
                        ),
                        ret(expr(var("m"))),
                    ])
                )),
            ])
        );
    }
}
