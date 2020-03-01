use crate::util::{Anchor, Source, Span};
use itertools::Itertools;
use std::collections::HashMap;

macro_rules! declare_tokens {
    (keywords => { $($keyword:ident => $keyword_repr:literal,)* }
     single   => { $($single:ident => $single_repr:literal,)* }
     double   => { $($double:ident => $double_repr:literal,)* }) => {
        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum TokenKind {
            Identifier,
            Number,
            String,

            $($single,)*
            $($double,)*
            $($keyword,)*

            Invalid,
        }
        impl std::fmt::Display for TokenKind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", Into::<&'static str>::into(*self))
            }
        }
        impl From<TokenKind> for &'static str {
            fn from(token: TokenKind) -> Self {
                use TokenKind::*;
                match token {
                    Identifier => "identifier",
                    Number => "number",
                    String => "string literal",

                    $($keyword => $keyword_repr,)*
                    $($single => $single_repr,)*
                    $($double => $double_repr,)*

                    Invalid => "##INVALID##",
                }
            }
        }
        fn keyword<T: AsRef<str>>(identifier: T) -> Option<TokenKind> {
            match identifier.as_ref() {
                $($keyword_repr => Some(TokenKind::$keyword),)*
                _ => None,
            }
        }
    }
}

declare_tokens!(
    keywords => {
        Else => "else",
        If => "if",
        Let => "let",
        Return => "return",
    }
    single => {
        Equals => "=",
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
        Amp => "&",
        Bar => "|",
        Caret => "^",

        LeftBrace => "{",
        RightBrace => "}",
        LeftParen => "(",
        RightParen => ")",
        LeftBracket => "{",
        RightBracket => "}",
        LeftAngle => "<",
        RightAngle => ">",
    }
    double => {
        EqualsEquals => "==",
        ColonColon => "::",
        BarBar => "||",
        AmpAmp => "&&",
        LeftAngleEquals => "<=",
        RightAngleEquals => ">=",
    }
);

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
    pub strings: HashMap<TokenId, String>,
    pub idents: HashMap<TokenId, String>,
    pub numbers: HashMap<TokenId, usize>,
}

impl TokenMap {
    pub fn new() -> Self {
        Self {
            tokens: vec![],
            strings: HashMap::new(),
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

    pub fn add_string(&mut self, literal: String, span: Span) -> Token {
        let token = self.add_token(TokenKind::String, span);
        self.strings.insert(token.id, literal);
        token
    }

    pub fn add_ident(&mut self, identifier: String, span: Span) -> Token {
        let token = self.add_token(TokenKind::Identifier, span);
        self.idents.insert(token.id, identifier);
        token
    }

    pub fn add_number(&mut self, value: usize, span: Span) -> Token {
        let token = self.add_token(TokenKind::Number, span);
        self.numbers.insert(token.id, value);
        token
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

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
            line: 1,
            column: 1,
            map: TokenMap::new(),
        }
    }

    fn span(&self, len: usize) -> Span {
        let (line, column) = (self.line, self.column);
        Span {
            start: Anchor { line, column },
            end: Anchor {
                line,
                column: column + len,
            },
        }
    }

    fn scan_token(&mut self, first: char, chars: &mut Chars<'_>) {
        use TokenKind::*;
        macro_rules! scan {
            {$($type:ident { $($case:literal => $tokens:expr,)* }),+, { $rest:expr }} => {
                match first {
                    $($($case => {
                        let (kind, len) = $type!($case, $tokens);
                        self.map.add_token(kind, self.span(len))
                    })*)*
                    _ => $rest,
                }
            }
        }
        macro_rules! single {
            ($char:literal, $token:expr) => {{
                chars.next();
                ($token, 1)
            }};
        }
        macro_rules! double {
            ($char:literal, $tokens:expr) => {{
                let single = $tokens.0;
                let double = $tokens.1;
                chars.next();
                match chars.peek() {
                    Some(&c) if c == $char => {
                        chars.next();
                        (double, 2)
                    }
                    _ => (single, 1),
                }
            }};
        }
        let token = scan! {
            single {
                '^' => Caret,
                '~' => Tilde,
                '?' => Question,
                '.' => Dot,
                ',' => Comma,
                '!' => Not,
                ';' => SemiColon,
                '*' => Star,
                '+' => Plus,
                '-' => Minus,
                '/' => Slash,
                '(' => LeftParen,
                ')' => RightParen,
                '{' => LeftBrace,
                '}' => RightBrace,
                '[' => LeftBracket,
                ']' => RightBracket,
                '<' => LeftAngle,
                '>' => RightAngle,
            },
            double {
                '=' => (Equals, EqualsEquals),
                ':' => (Colon, ColonColon),
                '|' => (Bar, BarBar),
                '&' => (Amp, AmpAmp),
            },
            {
                if first == '"' {
                    self.scan_string(chars)
                } else if first.is_numeric() {
                    self.scan_number(chars)
                } else if is_ident(first) {
                    self.scan_identifier_or_keyword(chars)
                } else {
                    dbg!(first, self.line, self.column);
                    chars.next();
                    self.map.add_token(TokenKind::Invalid, self.span(1))
                }
            }
        };
        self.column = token.span.end.column;
    }

    fn scan_string(&mut self, chars: &mut Chars<'_>) -> Token {
        let _start_of_literal = chars.next();
        let literal: String = chars.take_while(|&c| c != '"').collect();
        // Length of the literal, plus the two quote characters
        let len = literal.len() + 2;
        self.map.add_string(literal, self.span(len))
    }

    fn scan_number(&mut self, chars: &mut Chars<'_>) -> Token {
        let nums: String = chars.peeking_take_while(|c| c.is_numeric()).collect();

        let len = nums.len();
        let value = nums.parse().unwrap();
        self.map.add_number(value, self.span(len))
    }

    fn scan_identifier_or_keyword(&mut self, chars: &mut Chars<'_>) -> Token {
        let identifier: String = chars.peeking_take_while(|&c| is_ident(c)).collect();
        let len = identifier.len();
        let span = self.span(len);
        let kind = keyword(&identifier).unwrap_or(TokenKind::Identifier);
        match kind {
            TokenKind::Identifier => self.map.add_ident(identifier, span),
            kind => self.map.add_token(kind, span),
        }
    }

    fn lex(mut self) -> TokenMap {
        for line in self.source.lines() {
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
            self.line += 1;
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
            macro_rules! ident {
                { $ident:expr, $span:expr } => {
                    (_token!(Identifier, $span), Some(String::from($ident)), None, None)
                }
            }
            macro_rules! string {
                { $literal:literal, $span:expr } => {
                    (_token!(String, $span), None, None, Some(String::from($literal)))
                }
            }
            macro_rules! number {
                { $val:expr, $span:expr } => {
                    (_token!(Number, $span), None, Some($val), None)
                }
            }
            macro_rules! token {
                { $kind:ident, $span:expr } => {
                    (_token!($kind, $span), None, None, None)
                }
            }
            let source = SourceBuilder::new().source($input).build();
            let tokens = lex(&source);
            [$($token)*].iter().for_each(|expected: &(Token, Option<String>, Option<usize>, Option<String>)| {
                let token = expected.0;
                let actual = tokens.token(token.id.0);
                let ident = tokens.idents.get(&token.id).cloned();
                let number = tokens.numbers.get(&token.id).cloned();
                let string = tokens.strings.get(&token.id).cloned();
                assert_eq!(actual, expected.0, "token matches");
                assert_eq!(ident, expected.1, "identifier matches");
                assert_eq!(number, expected.2, "number matches");
                assert_eq!(string, expected.3, "string matches");
            });
        }
    }

    #[test]
    fn empty() {
        assert_tokens! { "", [] };
    }

    #[test]
    fn single() {
        assert_tokens! { "identifier", [
            ident! { "identifier", span!(1:1, 1:11) }
        ]};
        assert_tokens! { "1200", [
            number! { 1200, span!(1:1, 1:5) }
        ]};
    }

    #[test]
    fn mixed() {
        assert_tokens! { "mixed 1200", [
            ident!  { "mixed", span!(1:01, 1:06) },
            number! { 1200,    span!(1:07, 1:11) },
        ]};
        assert_tokens! { "1200mixed", [
            number! { 1200,    span!(1:01, 1:05) },
            ident!  { "mixed", span!(1:05, 1:10) },
        ]};
        assert_tokens! { "1200 +  mixed;", [
            number! { 1200,      span!(1:01, 1:05) },
            token!  { Plus,      span!(1:06, 1:07) },
            ident!  { "mixed",   span!(1:09, 1:14) },
            token!  { SemiColon, span!(1:14, 1:15) }
        ]};
    }
    #[test]
    fn multi_line() {
        assert_tokens! { "some idents\n10 + 20", [
            ident!  { "some",   span!(1:01, 1:05) },
            ident!  { "idents", span!(1:06, 1:12) },
            number! { 10,       span!(2:01, 2:03) },
            token!  { Plus,     span!(2:04, 2:05) },
            number! { 20,       span!(2:06, 2:08) },
        ]};
    }
    #[test]
    fn all() {
        assert_tokens! { "ident 1234567890 ===!(){}[]<>:::+*-,./?~&&&|||^", [
            ident!  { "ident",      span!(1:01, 1:06) },
            number! { 1234567890,   span!(1:07, 1:17) },
            token!  { EqualsEquals, span!(1:18, 1:20) },
            token!  { Equals,       span!(1:20, 1:21) },
            token!  { Not,          span!(1:21, 1:22) },
            token!  { LeftParen,    span!(1:22, 1:23) },
            token!  { RightParen,   span!(1:23, 1:24) },
            token!  { LeftBrace,    span!(1:24, 1:25) },
            token!  { RightBrace,   span!(1:25, 1:26) },
            token!  { LeftBracket,  span!(1:26, 1:27) },
            token!  { RightBracket, span!(1:27, 1:28) },
            token!  { LeftAngle,    span!(1:28, 1:29) },
            token!  { RightAngle,   span!(1:29, 1:30) },
            token!  { ColonColon,   span!(1:30, 1:32) },
            token!  { Colon,        span!(1:32, 1:33) },
            token!  { Plus,         span!(1:33, 1:34) },
            token!  { Star,         span!(1:34, 1:35) },
            token!  { Minus,        span!(1:35, 1:36) },
            token!  { Comma,        span!(1:36, 1:37) },
            token!  { Dot,          span!(1:37, 1:38) },
            token!  { Slash,        span!(1:38, 1:39) },
            token!  { Question,     span!(1:39, 1:40) },
            token!  { Tilde,        span!(1:40, 1:41) },
            token!  { AmpAmp,       span!(1:41, 1:43) },
            token!  { Amp,          span!(1:43, 1:44) },
            token!  { BarBar,       span!(1:44, 1:46) },
            token!  { Bar,          span!(1:46, 1:47) },
            token!  { Caret,        span!(1:47, 1:48) },
        ]};
    }
    #[test]
    fn keyword() {
        assert_tokens! { "let x = 10;", [
            token!  { Let,       span!(1:01, 1:04) },
            ident!  { "x",       span!(1:05, 1:06) },
            token!  { Equals,    span!(1:07, 1:08) },
            number! { 10,        span!(1:09, 1:11) },
            token!  { SemiColon, span!(1:11, 1:12) },
        ]};
        assert_tokens! { "return 10;", [
            token!  { Return,    span!(1:01, 1:07) },
            number! { 10,        span!(1:08, 1:10) },
            token!  { SemiColon, span!(1:10, 1:11) },
        ]};
        assert_tokens! { "if n > 10 { return n; }", [
            token!  { If,         span!(1:01, 1:03) },
            ident!  { "n",        span!(1:04, 1:05) },
            token!  { RightAngle, span!(1:06, 1:07) },
            number! { 10,         span!(1:08, 1:10) },
            token!  { LeftBrace,  span!(1:11, 1:12) },
            token!  { Return,     span!(1:13, 1:19) },
            ident!  { "n",        span!(1:20, 1:21) },
            token!  { SemiColon,  span!(1:21, 1:22) },
            token!  { RightBrace, span!(1:23, 1:24) },
        ]};
    }
    #[test]
    fn string_literal() {
        assert_tokens! { r#" "this should be a whole literal" "#, [
            string! { "this should be a whole literal", span!(1:02, 1:34) },
        ]};
    }
}
