use crate::util::{Anchor, Source, Span};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier,
    Number,

    Assign,
    Colon,
    Comma,
    Dot,
    Minus,
    Not,
    Plus,
    Question,
    SemiColon,
    Slash,
    Star,
    Tilde,
    BitAnd,
    BitOr,
    BitXor,

    Equals,
    Scope,
    LogicAnd,
    LogicOr,

    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftAngle,
    RightAngle,

    Let,
    Print,

    Invalid,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TokenKind::*;
        let repr = match *self {
            Identifier => "identifier",
            Number => "number",

            Assign => "=",
            Colon => ":",
            Comma => ",",
            Dot => ".",
            Minus => "-",
            Not => "!",
            Plus => "+",
            Question => "?",
            SemiColon => ";",
            Slash => "/",
            Star => "*",
            Tilde => "~",
            BitAnd => "&",
            BitOr => "|",
            BitXor => "^",

            Equals => "==",
            Scope => "::",
            LogicAnd => "||",
            LogicOr => "&&",

            LeftBrace => "{",
            RightBrace => "}",
            LeftParen => "(",
            RightParen => ")",
            LeftBracket => "{",
            RightBracket => "}",
            LeftAngle => "<",
            RightAngle => ">",

            Let => "let",
            Print => "print",

            Invalid => "##INVALID##",
        };
        write!(f, "{}", repr)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct TokenId(usize);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Token {
    pub id: TokenId,
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenMap {
    tokens: Vec<Token>,
    pub idents: HashMap<TokenId, String>,
    pub numbers: HashMap<TokenId, usize>,
}

impl TokenMap {
    pub fn new() -> Self {
        Self {
            tokens: vec![],
            idents: HashMap::new(),
            numbers: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn token(&self, index: usize) -> Token {
        self.tokens[index]
    }

    pub fn add_token(&mut self, kind: TokenKind, span: Span) -> Token {
        let id = TokenId(self.tokens.len());
        let token = Token { id, kind, span };
        self.tokens.push(token);
        token
    }

    pub fn add_ident(&mut self, id: TokenId, identifier: String) {
        self.idents.insert(id, identifier);
    }

    pub fn add_number(&mut self, id: TokenId, value: usize) {
        self.numbers.insert(id, value);
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

type TokenStream<'a> = std::iter::Peekable<std::str::Chars<'a>>;

struct Lexer<'a> {
    source: &'a Source,
    line: usize,
    column: usize,
    map: TokenMap,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a Source) -> Self {
        Self {
            source,
            line: 0,
            column: 0,
            map: TokenMap::new(),
        }
    }

    fn make_token(&mut self, len: usize, kind: TokenKind) -> Token {
        let (line, column) = (self.line, self.column);
        let span = Span {
            start: Anchor { line, column },
            end: Anchor {
                line,
                column: column + len,
            },
        };
        let token = self.map.add_token(kind, span);
        self.column = token.span.end.column;
        token
    }

    fn scan_token(&mut self, first: char, indexed_chars: &mut TokenStream<'_>) {
        macro_rules! special_cases {
            {$($type:ident { $($case:literal => $tokens:expr,)* })+} => {
                match first {
                    $($($case => return $type!($case, $tokens),)*)*
                    _ => {}
                }
            }
        }
        macro_rules! single {
            ($char:literal, $token:expr) => {{
                indexed_chars.next();
                self.make_token(1, $token);
            }};
        }
        macro_rules! double {
            ($char:literal, $tokens:expr) => {{
                let single = $tokens.0;
                let double = $tokens.1;
                indexed_chars.next();
                match indexed_chars.peek() {
                    Some(&c) if c == $char => {
                        indexed_chars.next();
                        self.make_token(2, double);
                    }
                    _ => {
                        self.make_token(1, single);
                    }
                }
            }};
        }
        special_cases! {
            single {
                '^' => TokenKind::BitXor,
                '~' => TokenKind::Tilde,
                '?' => TokenKind::Question,
                '.' => TokenKind::Dot,
                ',' => TokenKind::Comma,
                '!' => TokenKind::Not,
                ';' => TokenKind::SemiColon,
                '*' => TokenKind::Star,
                '+' => TokenKind::Plus,
                '-' => TokenKind::Minus,
                '/' => TokenKind::Slash,
                '(' => TokenKind::LeftParen,
                ')' => TokenKind::RightParen,
                '{' => TokenKind::LeftBrace,
                '}' => TokenKind::RightBrace,
                '[' => TokenKind::LeftBracket,
                ']' => TokenKind::RightBracket,
                '<' => TokenKind::LeftAngle,
                '>' => TokenKind::RightAngle,
            }
            double {
                '=' => (TokenKind::Assign, TokenKind::Equals),
                ':' => (TokenKind::Colon, TokenKind::Scope),
                '|' => (TokenKind::BitOr, TokenKind::LogicOr),
                '&' => (TokenKind::BitAnd, TokenKind::LogicAnd),
            }
        };
        if first.is_numeric() {
            self.scan_number(indexed_chars);
        } else if is_ident(first) {
            self.scan_identifier_or_keyword(indexed_chars);
        } else {
            dbg!(first, self.line, self.column);
            indexed_chars.next();
            self.make_token(1, TokenKind::Invalid);
        }
    }

    fn scan_number(&mut self, indexed_chars: &mut TokenStream<'_>) {
        use itertools::Itertools;
        let nums: String = indexed_chars
            .peeking_take_while(|c| c.is_numeric())
            .collect();

        let len = nums.len();
        let value = nums.parse().unwrap();
        let token = self.make_token(len, TokenKind::Number);
        self.map.add_number(token.id, value);
    }

    fn scan_identifier_or_keyword(&mut self, indexed_chars: &mut TokenStream<'_>) {
        use itertools::Itertools;
        let identifier: String = indexed_chars.peeking_take_while(|&c| is_ident(c)).collect();
        let kind = match identifier.as_str() {
            "let" => TokenKind::Let,
            "print" => TokenKind::Print,
            _ => TokenKind::Identifier,
        };
        let len = identifier.len();
        let token = self.make_token(len, kind);
        if let TokenKind::Identifier = kind {
            self.map.add_ident(token.id, identifier);
        }
    }

    fn lex(mut self) -> TokenMap {
        for line in self.source.lines() {
            self.line += 1;
            self.column = 1;
            let mut chars = line.chars().peekable();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    self.column += 1;
                    chars.next();
                    continue;
                }
                self.scan_token(c, &mut chars);
            }
        }
        self.map
    }
}

pub fn lex(source: &Source) -> TokenMap {
    Lexer::new(source).lex()
}

#[cfg(test)]
#[allow(unused_macros, unused_mut, unused_variables, unused_assignments)]
mod tests {
    use super::*;
    use crate::span;
    use crate::util::SourceBuilder;

    macro_rules! assert_tokens {
        { $input:expr, [
            $($token:tt)*
        ]} => {
            let mut index = 0;
            macro_rules! _token {
                ($kind:ident, $span:expr) => {{
                    let token = Token {
                        kind: TokenKind::$kind,
                        span: $span,
                        id: TokenId(index),
                    };
                    index += 1;
                    token
                }};
            }
            macro_rules! identifier {
                { $ident:expr, $span:expr } => {
                    (_token!(Identifier, $span), Some(String::from($ident)), None)
                }
            }
            macro_rules! number { { $val:expr, $span:expr } => ((_token!(Number, $span), None, Some($val))); }
            macro_rules! token { { $kind:ident, $span:expr } => ((_token!($kind, $span), None, None)); }
            let source = SourceBuilder::new().lines($input).build();
            let tokens = lex(&source);
            [$($token)*].iter().for_each(|expected: &(Token, Option<String>, Option<usize>)| {
                let token = expected.0;
                let actual = tokens.token(token.id.0);
                let ident = tokens.idents.get(&token.id).cloned();
                let number = tokens.numbers.get(&token.id).cloned();
                assert_eq!(actual, expected.0, "token matches");
                assert_eq!(ident, expected.1, "identifier matches");
                assert_eq!(number, expected.2, "number matches");
            });
        }
    }

    #[test]
    fn lex_empty() {
        assert_tokens! { "", [] };
    }

    #[test]
    fn lex_single() {
        assert_tokens! { "identifier", [
            identifier! { "identifier", span!(1:1, 1:11) }
        ]};
        assert_tokens! { "1200", [
            number! { 1200, span!(1:1, 1:5) }
        ]};
    }

    #[test]
    fn lex_mixed() {
        assert_tokens! { "mixed 1200", [
            identifier! { "mixed",       span!(1:01, 1:06) },
            number!     { 1200,          span!(1:07, 1:11) },
        ]};
        assert_tokens! { "1200mixed", [
            number!     { 1200,         span!(1:01, 1:05) },
            identifier! { "mixed",      span!(1:05, 1:10) },
        ]};
        assert_tokens! { "1200 +  mixed;", [
            number!     { 1200,          span!(1:01, 1:05) },
            token!      { Plus,          span!(1:06, 1:07) },
            identifier! { "mixed",       span!(1:09, 1:14) },
            token!      { SemiColon,     span!(1:14, 1:15) }
        ]};
    }
    #[test]
    fn lex_multi_line() {
        assert_tokens! { "some idents\n10 + 20", [
            identifier! { "some",   span!(1:01, 1:05) },
            identifier! { "idents", span!(1:06, 1:12) },
            number!     { 10,       span!(2:01, 2:03) },
            token!      { Plus,     span!(2:04, 2:05) },
            number!     { 20,       span!(2:06, 2:08) },
        ]};
    }
    #[test]
    fn lex_all() {
        assert_tokens! { "ident 1234567890 ===!(){}[]<>:::+*-,./?~&&&|||^", [
            identifier! { "ident",      span!(1:01, 1:06) },
            number!     { 1234567890,   span!(1:07, 1:17) },
            token!      { Equals,       span!(1:18, 1:20) },
            token!      { Assign,       span!(1:20, 1:21) },
            token!      { Not,          span!(1:21, 1:22) },
            token!      { LeftParen,    span!(1:22, 1:23) },
            token!      { RightParen,   span!(1:23, 1:24) },
            token!      { LeftBrace,    span!(1:24, 1:25) },
            token!      { RightBrace,   span!(1:25, 1:26) },
            token!      { LeftBracket,  span!(1:26, 1:27) },
            token!      { RightBracket, span!(1:27, 1:28) },
            token!      { LeftAngle,    span!(1:28, 1:29) },
            token!      { RightAngle,   span!(1:29, 1:30) },
            token!      { Scope,        span!(1:30, 1:32) },
            token!      { Colon,        span!(1:32, 1:33) },
            token!      { Plus,         span!(1:33, 1:34) },
            token!      { Star,         span!(1:34, 1:35) },
            token!      { Minus,        span!(1:35, 1:36) },
            token!      { Comma,        span!(1:36, 1:37) },
            token!      { Dot,          span!(1:37, 1:38) },
            token!      { Slash,        span!(1:38, 1:39) },
            token!      { Question,     span!(1:39, 1:40) },
            token!      { Tilde,        span!(1:40, 1:41) },
            token!      { LogicAnd,     span!(1:41, 1:43) },
            token!      { BitAnd,       span!(1:43, 1:44) },
            token!      { LogicOr,      span!(1:44, 1:46) },
            token!      { BitOr,        span!(1:46, 1:47) },
            token!      { BitXor,       span!(1:47, 1:48) },
        ]};
    }
    #[test]
    fn lex_keyword() {
        assert_tokens! { "let x = 10;", [
            token!      { Let,       span!(1:01, 1:04) },
            identifier! { "x",       span!(1:05, 1:06) },
            token!      { Assign,    span!(1:07, 1:08) },
            number!     { 10,        span!(1:09, 1:11) },
            token!      { SemiColon, span!(1:11, 1:12) },
        ]};
        assert_tokens! { "print 10;", [
            token!  { Print,     span!(1:01, 1:06) },
            number! { 10,        span!(1:07, 1:09) },
            token!  { SemiColon, span!(1:09, 1:10) },
        ]};
    }
}
