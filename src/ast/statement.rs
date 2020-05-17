use super::*;
use crate::lexer::{TokenKind, TokenTree, TreeKind};

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
    pub kind: StatementKind,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Statement({kind})", kind = self.kind)
    }
}

impl Parser {
    pub(super) fn parse_scope(&self, cursor: &mut Cursor<'_>) -> Scope {
        let mut statements = Vec::new();
        while let Some(statement) = self.parse_statement(cursor) {
            statements.push(statement);
        }
        Scope { statements }
    }

    pub(super) fn parse_statement(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        match cursor.peek()? {
            TokenTree::Tree(tree) => {
                if tree.kind != TreeKind::Braces {
                    panic!("Unexpected {} in scope", tree.kind);
                }
                let mut cursor = Cursor::new(&tree.children);
                self.parse_local_scope(&mut cursor)
            }
            TokenTree::Token(token) => match token.kind {
                TokenKind::Let => self.parse_declaration(cursor),
                TokenKind::Return => self.parse_print(cursor),
                TokenKind::Identifier => self.parse_assignment(cursor),
                TokenKind::SemiColon => self.parse_null(cursor),
                TokenKind::If => self.parse_if_statement(cursor),
                _ => self.parse_expr_statement(cursor),
            },
            _ => None,
        }
    }

    fn parse_declaration(&self, cursor: &mut Cursor) -> Option<Statement> {
        cursor.expect_token(TokenKind::Let);
        let var = self.parse_variable(cursor)?;
        cursor.expect_token(TokenKind::Colon);
        let type_ = self
            .parse_type(cursor)
            .expect("couldn't parse type in declaration");
        let initializer = if cursor.maybe_token(TokenKind::Equals).is_some() {
            Some(
                self.parse_expression(cursor)
                    .expect("couldn't parse expression in declaration"),
            )
        } else {
            None
        };
        cursor.expect_token(TokenKind::SemiColon);
        Some(Statement {
            kind: StatementKind::Declaration {
                var,
                type_,
                initializer,
            },
        })
    }

    fn parse_assignment(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        let dst = self.parse_variable(cursor).map(Expression::from)?;
        cursor.expect_token(TokenKind::Equals);
        let src = self.parse_expression(cursor)?;
        cursor.expect_token(TokenKind::SemiColon);
        Some(Statement {
            kind: StatementKind::Assignment { src, dst },
        })
    }

    fn parse_print(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        cursor.expect_token(TokenKind::Return);
        let expr = self.parse_expression(cursor)?;
        cursor.expect_token(TokenKind::SemiColon);
        Some(Statement {
            kind: StatementKind::Return(expr),
        })
    }

    fn parse_local_scope(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        let scope = self.parse_scope(cursor);
        Some(Statement {
            kind: StatementKind::Scope(scope),
        })
    }

    fn parse_expr_statement(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        let expr = self.parse_expression(cursor)?;
        cursor.expect_token(TokenKind::SemiColon);
        Some(Statement {
            kind: StatementKind::Expression(expr),
        })
    }

    fn parse_if_statement(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        cursor.expect_token(TokenKind::If);
        let condition = Rc::new(self.parse_expression(cursor)?);
        let block = Rc::new(self.parse_scope(cursor));
        Some(Statement {
            kind: StatementKind::If { condition, block },
        })
    }

    fn parse_null(&self, cursor: &mut Cursor<'_>) -> Option<Statement> {
        cursor.expect_token(TokenKind::SemiColon);
        Some(Statement {
            kind: StatementKind::Null,
        })
    }
}
