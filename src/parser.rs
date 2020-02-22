use crate::lexer::{Token, TokenKind, TokenMap};
use crate::span;
use crate::util::{ArmPosition, Source, Span};
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

pub fn parse(source: &Source, map: TokenMap, out: &mut dyn std::io::Write) -> Tree {
    Parser::new(source, map, out).parse()
}

#[derive(Clone, PartialEq, Debug)]
pub enum Error {
    UnexpectedToken { expected: String, unexpected: Token },
    Internal(String),
    EOF,
}

impl Parser<'_> {
    fn report_err(&mut self, err: Error) {
        || -> std::io::Result<()> {
            write!(self.out, "\nParseError: ")?;
            match err {
                Error::EOF => writeln!(self.out, "unexpected EOF")?,
                Error::Internal(ref message) => writeln!(self.out, "internal: {}", message)?,
                Error::UnexpectedToken {
                    ref expected,
                    unexpected,
                } => writeln!(
                    self.out,
                    "expected `{}`, got `{}`",
                    expected, unexpected.kind
                )?,
            }
            if let Some(token) = self.peek().ok() {
                if let Some(context) = self.source.give_context(
                    token.span,
                    match err {
                        Error::EOF => ArmPosition::End,
                        _ => ArmPosition::Begin,
                    },
                ) {
                    writeln!(self.out, "{}", context)?;
                }
            }
            Ok(())
        }()
        .expect("couldn't write to parser out");
    }
}

type Result<T> = std::result::Result<T, Error>;

pub struct Parser<'a> {
    source: &'a Source,
    out: &'a mut dyn std::io::Write,
    map: TokenMap,
    index: usize,
    backtrack_index: usize,
    precedence: usize,
    tree: Tree,
}

impl<'a> Parser<'a> {
    fn new(source: &'a Source, map: TokenMap, out: &'a mut dyn std::io::Write) -> Self {
        Self {
            source,
            out,
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
        let token = self.peek()?;
        self.sync();
        self.index = self
            .index
            .checked_add(1)
            .expect("Overflow in parser token index");
        Ok(token)
    }

    fn advance_until(&mut self, kind: TokenKind) -> Option<Token> {
        loop {
            match self.advance() {
                Ok(token) => {
                    if token.kind == TokenKind::SemiColon {
                        return Some(token);
                    }
                }
                Err(Error::EOF) => return None,
                Err(_) => continue,
            }
        }
    }

    fn sync(&mut self) {
        self.backtrack_index = self.index;
    }

    fn backtrack(&mut self) {
        self.index = self.backtrack_index;
    }

    fn maybe(&mut self, kind: TokenKind) -> Option<Token> {
        self.expect(kind).ok()
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        let token = self.advance()?;
        if token.kind != kind {
            self.backtrack();
            return Err(Error::UnexpectedToken {
                expected: kind.to_string(),
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
    use crate::util::SourceBuilder;

    #[test]
    fn parser_stops_on_eof() {
        let source = SourceBuilder::new().build();
        let mut out = Vec::new();
        let mut parser = Parser::new(&source, TokenMap::new(), &mut out);
        assert_eq!(parser.advance(), Err(Error::EOF));
        let old_index = parser.index;
        assert_eq!(parser.advance(), Err(Error::EOF));
        let new_index = parser.index;
        assert_eq!(old_index, new_index);
    }

    macro_rules! token {
        { $map:expr, $kind:ident, $span:expr } => {
            $map.add_token(TokenKind::$kind, $span);
        }
    }
    macro_rules! identifier {
        { $map:expr, $name:expr, $span:expr } => {
            $map.add_ident(String::from($name), $span);
        }
    }
    macro_rules! number {
        { $map:expr, $value:expr, $span:expr } => {
            $map.add_number($value, $span);
        }
    }
    macro_rules! tree {
        [$source:literal, $($entry:ident { $($args:tt)* },)*] => {{
            let mut map = TokenMap::new();
            $( $entry! { map, $($args)* } )*;
            println!("{:#?}", map);
            let mut out = Vec::new();
            parse(&SourceBuilder::new().lines($source).build(), map, &mut out)
        }}
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
    macro_rules! var {
        ($span:expr, $ident:expr) => {
            $crate::parser::Variable {
                span: $span,
                identifier: String::from($ident),
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
    macro_rules! con {
        ($span:expr, $value:expr) => {
            $crate::parser::Constant {
                span: $span,
                value: $value,
            }
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

    #[test]
    fn parse_declaration() {
        let tree = tree![
            "let x = 10;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Assign,    span!(1:07, 1:08) },
            number     { 10,        span!(1:09, 1:11) },
            token      { SemiColon, span!(1:11, 1:12) },
        ];
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
        let tree = tree![
            "let x = x;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { Assign,    span!(1:07, 1:08) },
            identifier { "x",       span!(1:08, 1:09) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
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
        let tree = tree![
            "let x;",
            token      { Let,       span!(1:01, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
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
    fn parse_assignment() {
        let tree = tree![
            "x = x;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            identifier { "x",       span!(1:05, 1:06) },
            token      { SemiColon, span!(1:06, 1:07) },
        ];
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

    #[test]
    fn parse_binary_op() {
        let tree = tree![
            "x = 1 + 1;",
            identifier { "x",       span!(1:01, 1:02) },
            token      { Assign,    span!(1:03, 1:04) },
            number     { 1,         span!(1:05, 1:06) },
            token      { Plus,      span!(1:07, 1:08) },
            number     { 1,         span!(1:09, 1:10) },
            token      { SemiColon, span!(1:10, 1:11) },
        ];
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
    fn parse_precedence() {
        let tree = tree![
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
        let tree = tree![
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
        let tree = tree![
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
    #[test]
    fn parse_error() {
        let tree = tree![
            "let=;",
            token      { Let,       span!(1:01, 1:04) },
            token      { Assign,    span!(1:04, 1:05) },
            token      { SemiColon, span!(1:05, 1:06) },
        ];
    }
}
