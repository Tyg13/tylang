use crate::lexer::{Token, TokenKind, TokenMap};
use crate::span;
use crate::util::{Source, Span};
use std::rc::Rc;

#[derive(Debug, PartialEq)]
pub struct Tree {
    pub statements: Vec<Statement>,
}

impl Tree {
    fn new() -> Self {
        Self { statements: vec![] }
    }
}

pub fn parse(source: &Source, map: TokenMap) -> Tree {
    Parser::new(source, map).parse()
}

#[derive(Clone, PartialEq, Debug)]
pub enum Error {
    UnexpectedToken { expected: String, unexpected: Token },
    Internal(String),
    EOF,
}

impl Parser<'_> {
    fn report_err(&self, err: Error) {
        if let Some(token) = self.peek().ok() {
            if let Some(context) = self.source.line(token.span.start.line) {
                const PADDING_LEN: usize = 4;
                let prefix = format!(
                    "[{file}:{line}:{column}] {padding}",
                    file = self.source.file(),
                    line = token.span.start.line,
                    column = token.span.start.column,
                    padding = str::repeat(" ", PADDING_LEN)
                );
                println!("\n{prefix}{context}", prefix = prefix, context = context);
                println!(
                    "{arm}{hand}",
                    arm = str::repeat("-", prefix.len() + token.span.start.column - 1),
                    hand = str::repeat(
                        "^",
                        token
                            .span
                            .end
                            .column
                            .saturating_sub(token.span.start.column)
                    ),
                );
            }
        }
        let message = match err {
            Error::EOF => vec![format!("unexpected EOF")],
            Error::Internal(message) => vec![format!("internal: {}", message)],
            Error::UnexpectedToken {
                expected,
                unexpected,
            } => vec![
                format!("expected {}", expected),
                format!("got `{}`", unexpected.kind),
            ],
        };
        println!("{}", message.join("\n"));
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Parser<'a> {
    source: &'a Source,
    map: TokenMap,
    index: usize,
    backtrack_index: usize,
    precedence: usize,
    tree: Tree,
}

impl<'a> Parser<'a> {
    fn new(source: &'a Source, map: TokenMap) -> Self {
        Self {
            source,
            map,
            index: 0,
            backtrack_index: 0,
            precedence: 0,
            tree: Tree::new(),
        }
    }

    fn parse(self) -> Tree {
        let mut this = self;
        this.parse_statements();
        this.tree
    }

    fn peek(&self) -> Result<Token> {
        if self.index >= self.map.len() {
            return Err(Error::EOF);
        }
        Ok(self.map.token(self.index))
    }

    fn advance(&mut self) -> Result<Token> {
        let token = self.peek();
        self.index = self
            .index
            .checked_add(1)
            .expect("Overflow in parser token index");
        token
    }

    fn sync(&mut self) {
        self.backtrack_index = self.index;
    }

    fn backtrack(&mut self) {
        self.index = self.backtrack_index;
    }

    fn maybe(&mut self, kind: TokenKind) -> Option<Token> {
        self.sync();
        let val = self.expect(kind);
        if val.is_err() {
            self.backtrack();
        }
        val.ok()
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        let token = self.advance()?;
        if token.kind != kind {
            return Err(Error::UnexpectedToken {
                expected: format!(""),
                unexpected: token,
            });
        }
        Ok(token)
    }
}

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
    fn parse_statements(&mut self) {
        loop {
            match self.parse_statement() {
                Ok(Some(statement)) => self.tree.statements.push(statement),
                Ok(None) => break,
                Err(err) => {
                    self.tree.statements.clear();
                    self.report_err(err);
                    break;
                }
            };
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
        let _let = self.expect(TokenKind::Let)?;
        let var = self.parse_variable()?;
        let initializer = if self.maybe(TokenKind::Assign).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let semi = self.expect(TokenKind::SemiColon)?;
        Ok(Statement {
            span: span!(_let.span.start, semi.span.end),
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
    fn parse_expression(&mut self) -> Result<Expression> {
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

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub span: Span,
    pub value: usize,
}

impl std::fmt::Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Constant({span} {value})",
            span = self.span,
            value = self.value
        )
    }
}

impl From<Constant> for Expression {
    fn from(cons: Constant) -> Self {
        Self {
            span: cons.span,
            kind: ExpressionKind::Constant(cons),
        }
    }
}

impl Parser<'_> {
    fn number(&self, token: Token) -> Result<usize> {
        self.map
            .numbers
            .get(&token.id)
            .cloned()
            .ok_or(Error::Internal(format!("{:#?} is not a number", token)))
    }
    fn parse_constant(&mut self) -> Result<Constant> {
        let token = self.expect(TokenKind::Number)?;
        let span = token.span;
        let value = self.number(token)?;
        Ok(Constant { span, value })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub span: Span,
    pub identifier: String,
}

impl std::fmt::Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Variable({span} {identifier})",
            span = self.span,
            identifier = self.identifier,
        )
    }
}

impl From<Variable> for Expression {
    fn from(var: Variable) -> Self {
        Self {
            span: var.span,
            kind: ExpressionKind::Variable(var),
        }
    }
}

impl Parser<'_> {
    fn ident(&self, token: Token) -> Result<String> {
        self.map
            .idents
            .get(&token.id)
            .cloned()
            .ok_or(Error::Internal(format!(
                "{:#?} is not an identifier",
                token
            )))
    }
    fn parse_variable(&mut self) -> Result<Variable> {
        let token = self.expect(TokenKind::Identifier)?;
        let span = token.span;
        let identifier = self.ident(token)?;
        Ok(Variable { span, identifier })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::*;

    macro_rules! token {
        { $map:expr, $kind:ident, $span:expr } => {
            $map.add_token(TokenKind::$kind, $span);
        }
    }

    macro_rules! identifier {
        { $map:expr, $name:expr, $span:expr } => {
            let token = $map.add_token(TokenKind::Identifier, $span);
            $map.add_ident(token.id, String::from($name));
        }
    }
    macro_rules! number {
        { $map:expr, $value:expr, $span:expr } => {
            let token = $map.add_token(TokenKind::Number, $span);
            $map.add_number(token.id, $value);
        }
    }

    macro_rules! tree {
        [$($entry:ident { $($args:tt)* },)*] => {{
            let mut map = TokenMap::new();
            $( $entry! { map, $($args)* } )*;
            println!("{:#?}", map);
            parse(&$crate::util::SourceBuilder::new().build(), map)
        }}
    }

    #[test]
    fn parse_declaration() {
        let tree = tree![
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Assign,    span!(1:07, 1:08) },
            number     { 10,        span!(1:09, 1:11) },
            token      { SemiColon, span!(1:11, 1:12) },
        ];
        assert_eq!(
            tree.statements,
            vec![Statement {
                span: span!(1:01, 1:12),
                kind: StatementKind::Declaration {
                    var: Variable {
                        span: span!(1:05, 1:06),
                        identifier: String::from("x")
                    },
                    initializer: Some(Expression {
                        span: span!(1:09, 1:11),
                        kind: ExpressionKind::Constant(Constant {
                            span: span!(1:09, 1:11),
                            value: 10,
                        }),
                    })
                }
            }],
            "Declaration with constant initializer"
        );
        let tree = tree![
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Assign,    span!(1:07, 1:08) },
            identifier { "x",       span!(1:08, 1:09) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
        assert_eq!(
            tree.statements,
            vec![Statement {
                span: span!(1:01, 1:11),
                kind: StatementKind::Declaration {
                    var: Variable {
                        span: span!(1:05, 1:06),
                        identifier: String::from("x")
                    },
                    initializer: Some(Expression {
                        span: span!(1:08, 1:09),
                        kind: ExpressionKind::Variable(Variable {
                            span: span!(1:08, 1:09),
                            identifier: String::from("x")
                        }),
                    })
                }
            }],
            "Declaration with variable initializer"
        );
        let tree = tree![
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
        assert_eq!(
            tree.statements,
            vec![Statement {
                span: span!(1:01, 1:07),
                kind: StatementKind::Declaration {
                    var: Variable {
                        span: span!(1:05, 1:06),
                        identifier: String::from("x"),
                    },
                    initializer: None,
                }
            }],
            "Declaration with no initializer"
        );
    }

    #[test]
    fn parse_assignment() {
        let tree = tree![
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
        assert_eq!(
            tree.statements,
            vec![Statement {
                span: span!(1:01, 1:07),
                kind: StatementKind::Assignment {
                    dst: Expression {
                        span: span!(1:01, 1:02),
                        kind: ExpressionKind::Variable(Variable {
                            span: span!(1:01, 1:02),
                            identifier: String::from("x"),
                        }),
                    },
                    src: Expression {
                        span: span!(1:05, 1:06),
                        kind: ExpressionKind::Variable(Variable {
                            span: span!(1:05, 1:06),
                            identifier: String::from("x"),
                        }),
                    }
                }
            }],
            "variable assignment to variable"
        );
    }

    #[test]
    fn parse_binary_op() {
        let tree = tree![
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            number     { 1,         span!(1:05, 1:06) },
            token      { Plus,      span!(1:07, 1:08) },
            number     { 1,         span!(1:09, 1:10) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
        assert_eq!(
            tree.statements,
            vec![Statement {
                span: span!(1:01, 1:11),
                kind: StatementKind::Assignment {
                    dst: Expression {
                        span: span!(1:01, 1:02),
                        kind: ExpressionKind::Variable(Variable {
                            span: span!(1:01, 1:02),
                            identifier: String::from("x"),
                        })
                    },
                    src: Expression {
                        span: span!(1:05, 1:10),
                        kind: ExpressionKind::BinaryOp {
                            kind: BinaryOpKind::Add,
                            lhs: Rc::new(Expression {
                                span: span!(1:05, 1:06),
                                kind: ExpressionKind::Constant(Constant {
                                    span: span!(1:05, 1:06),
                                    value: 1,
                                }),
                            }),
                            rhs: Rc::new(Expression {
                                span: span!(1:09, 1:10),
                                kind: ExpressionKind::Constant(Constant {
                                    span: span!(1:09, 1:10),
                                    value: 1,
                                }),
                            }),
                        }
                    }
                }
            }],
            "x = 1 + 1"
        );
    }

    #[test]
    fn parse_precedence() {}
}
