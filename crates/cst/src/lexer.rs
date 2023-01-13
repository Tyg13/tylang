use crate::hash::hash;
use crate::{green, SyntaxKind};
use std::collections::HashMap;
use std::sync::Arc;

pub type Token = Arc<green::Token>;
pub type TokenCache = HashMap<(SyntaxKind, u64), Token>;

pub trait TextSource {
    fn peek_n(&self, start: usize, n: usize) -> &str;
    fn peek(&self, at: usize) -> Option<char>;
}

impl TextSource for &str {
    fn peek_n(&self, start: usize, n: usize) -> &str {
        &self[start..start + n]
    }

    fn peek(&self, at: usize) -> Option<char> {
        self.chars().nth(at)
    }
}

pub trait TokenSink {
    fn token(&mut self, kind: SyntaxKind, text: &str);
}

pub struct Lexer<'src, 'snk, Src: TextSource, Snk: TokenSink> {
    source: &'src Src,
    sink: &'snk mut Snk,
    offset: usize,
}

macro_rules! start_ident {
    () => { 'a'..='z' | 'A'..='Z' | '_' }
}

macro_rules! number {
    () => {
        '0'..='9'
    };
}

macro_rules! whitespace {
    () => {
        ' '
    };
}

macro_rules! eol {
    () => {
        '\r' | '\n'
    };
}

impl<'src, 'snk, Src, Sink> Lexer<'src, 'snk, Src, Sink>
where
    Src: TextSource,
    Sink: TokenSink,
{
    pub fn new(source: &'src Src, sink: &'snk mut Sink) -> Self {
        Self {
            source,
            sink,
            offset: 0,
        }
    }
}

impl<Src: TextSource, Sink: TokenSink> Lexer<'_, '_, Src, Sink> {
    pub fn lex_one(&mut self) -> bool {
        let token = match self.peek() {
            Some(token) => token,
            None => {
                self.token(SyntaxKind::EOF, 0);
                return false;
            }
        };
        match token {
            start_ident!() => self.ident_or_keyword(),
            number!() => self.number(),
            whitespace!() => self.whitespace(),
            eol!() => self.eol(token),
            '(' => self.single(SyntaxKind::LEFT_PAREN),
            ')' => self.single(SyntaxKind::RIGHT_PAREN),
            '{' => self.single(SyntaxKind::LEFT_CURLY),
            '}' => self.single(SyntaxKind::RIGHT_CURLY),
            '[' => self.single(SyntaxKind::LEFT_SQUARE),
            ']' => self.single(SyntaxKind::RIGHT_SQUARE),
            '<' => self.single(SyntaxKind::LEFT_ANGLE),
            '>' => self.single(SyntaxKind::RIGHT_ANGLE),
            ':' => self.single(SyntaxKind::COLON),
            ';' => self.single(SyntaxKind::SEMICOLON),
            '=' => self.single(SyntaxKind::EQUALS),
            '!' => self.single(SyntaxKind::BANG),
            ',' => self.single(SyntaxKind::COMMA),
            '-' => self.single(SyntaxKind::DASH),
            '+' => self.single(SyntaxKind::PLUS),
            '*' => self.single(SyntaxKind::STAR),
            '.' => self.single(SyntaxKind::DOT),
            '&' => self.single(SyntaxKind::AMPERSAND),
            '|' => self.single(SyntaxKind::BAR),
            '"' => self.string(),
            '/' => {
                if self.source.peek_n(self.offset, 2) == "//" {
                    self.comment();
                } else {
                    self.single(SyntaxKind::SLASH);
                }
            }
            _ => self.single(SyntaxKind::ERROR),
        };
        true
    }

    fn peek(&self) -> Option<char> {
        self.source.peek(self.offset)
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.source.peek(self.offset + n)
    }

    fn token(&mut self, kind: SyntaxKind, n: usize) {
        let text = self.source.peek_n(self.offset, n);
        debug_assert_eq!(text.len(), n);
        self.sink.token(kind, text);
        self.offset += n;
    }

    fn single(&mut self, kind: SyntaxKind) {
        self.token(kind, 1);
    }

    fn matching_range(
        &mut self,
        bias: usize,
        accept: impl Fn(char) -> bool,
    ) -> usize {
        let mut len = bias;
        while let Some(c) = self.peek_ahead(len) {
            if accept(c) {
                len += 1;
            } else {
                break;
            }
        }
        len
    }

    fn lex_kind(&mut self, kind: SyntaxKind, accept: impl Fn(char) -> bool) {
        let len = self.matching_range(0, accept);
        self.token(kind, len);
    }

    fn ident_or_keyword(&mut self) {
        let start = self.matching_range(0, is_start_ident);
        let len = self.matching_range(start, is_ident);
        let kind = match self.source.peek_n(self.offset, len) {
            "as" => SyntaxKind::AS_KW,
            "break" => SyntaxKind::BREAK_KW,
            "continue" => SyntaxKind::CONTINUE_KW,
            "else" => SyntaxKind::ELSE_KW,
            "extern" => SyntaxKind::EXTERN_KW,
            "fn" => SyntaxKind::FN_KW,
            "if" => SyntaxKind::IF_KW,
            "import" => SyntaxKind::IMPORT_KW,
            "let" => SyntaxKind::LET_KW,
            "loop" => SyntaxKind::LOOP_KW,
            "mod" => SyntaxKind::MOD_KW,
            "return" => SyntaxKind::RETURN_KW,
            "type" => SyntaxKind::TYPE_KW,
            "while" => SyntaxKind::WHILE_KW,
            _ => SyntaxKind::IDENT,
        };
        self.token(kind, len);
    }

    fn whitespace(&mut self) {
        self.lex_kind(SyntaxKind::WHITESPACE, is_whitespace);
    }

    fn eol(&mut self, c: char) {
        if c == '\r' && self.peek_ahead(1) == Some('\n') {
            self.token(SyntaxKind::EOL, 2);
            return;
        }
        self.token(SyntaxKind::EOL, 1);
    }

    fn number(&mut self) {
        self.lex_kind(SyntaxKind::NUMBER, is_number);
    }

    fn string(&mut self) {
        let mut len = self.matching_range(1, |c| c != '"');
        let kind = if self.peek_ahead(len) == Some('"') {
            len += 1;
            SyntaxKind::STRING
        } else {
            SyntaxKind::ERROR
        };
        self.token(kind, len);
    }

    fn comment(&mut self) {
        let len = self.matching_range(2, |c| c != '\n');
        self.token(SyntaxKind::COMMENT, len);
    }
}

fn is_start_ident(c: char) -> bool {
    matches!(c, start_ident!())
}

fn is_ident(c: char) -> bool {
    char::is_ascii_alphanumeric(&c)
}

fn is_whitespace(c: char) -> bool {
    matches!(c, whitespace!())
}

fn is_number(c: char) -> bool {
    matches!(c, number!())
}

pub struct Tokens {
    pub stream: Vec<Token>,
    pub cache: TokenCache,
}

impl Tokens {
    pub fn new() -> Self {
        Self {
            stream: Default::default(),
            cache: TokenCache::new(),
        }
    }
}

impl TokenSink for Tokens {
    fn token(&mut self, kind: SyntaxKind, text: &str) {
        let t = self.cache.entry((kind, hash(text))).or_insert_with(|| {
            Arc::new(green::Token {
                kind,
                text: text.to_string(),
            })
        });
        self.stream.push(t.clone());
    }
}

pub fn lex(source: &str) -> Tokens {
    let mut tokens = Tokens::new();
    let mut lexer = Lexer::new(&source, &mut tokens);
    while lexer.lex_one() {}
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use SyntaxKind::*;

    macro_rules! assert_token {
        ($tokens:expr, $kind:expr, $text:expr) => {{
            let token = $tokens.next().unwrap();
            assert_eq!(token.kind, $kind);
            assert_eq!(token.text, $text);
            token
        }};
    }

    macro_rules! assert_done {
        ($tokens:expr) => {
            let token = $tokens.next().unwrap();
            assert_eq!(token.kind, EOF);
            assert_eq!($tokens.next(), None);
        };
    }

    fn check(s: &'static str, ts: &[(SyntaxKind, &'static str)]) {
        let mut tokens = Tokens::new();
        {
            let mut lexer = Lexer::new(&s, &mut tokens);
            while lexer.lex_one() {}
        }
        let mut stream = tokens.stream.iter();
        for (kind, text) in ts {
            assert_token!(stream, *kind, *text);
        }
        assert_done!(stream);
    }

    #[test]
    fn ident() {
        check("foo", &[(IDENT, "foo")]);
    }

    #[test]
    fn whitespace_and_eol() {
        check("  ", &[(WHITESPACE, "  ")]);
        check("\n ", &[(EOL, "\n"), (WHITESPACE, " ")]);
        check("\n  \n", &[(EOL, "\n"), (WHITESPACE, "  "), (EOL, "\n")]);
        check(
            "\r\n   \n",
            &[(EOL, "\r\n"), (WHITESPACE, "   "), (EOL, "\n")],
        );
    }

    #[test]
    fn number() {
        check("123", &[(NUMBER, "123")]);
        check("3", &[(NUMBER, "3")]);
    }

    #[test]
    fn string() {
        check(r#""foo""#, &[(STRING, "\"foo\"")]);
        check(r#""foo"#, &[(ERROR, "\"foo")]);
        check(
            r#""foobar""bazbaz""#,
            &[(STRING, "\"foobar\""), (STRING, "\"bazbaz\"")],
        );
    }

    #[test]
    fn none() {
        check("", &[]);
    }

    #[test]
    fn interning() {
        let mut tokens = Tokens::new();
        let mut lexer = Lexer::new(&"foo foo", &mut tokens);
        while lexer.lex_one() {}
        let mut stream = tokens.stream.iter();
        let foo_1 = assert_token!(stream, IDENT, "foo");
        let _ = assert_token!(stream, WHITESPACE, " ");
        let foo_2 = assert_token!(stream, IDENT, "foo");
        assert!(Arc::ptr_eq(&foo_1, &foo_2));
    }

    #[test]
    fn keywords() {
        check(
            "mod import type let fn return if else loop while break continue as extern",
            &[
                (MOD_KW, "mod"),
                (WHITESPACE, " "),
                (IMPORT_KW, "import"),
                (WHITESPACE, " "),
                (TYPE_KW, "type"),
                (WHITESPACE, " "),
                (LET_KW, "let"),
                (WHITESPACE, " "),
                (FN_KW, "fn"),
                (WHITESPACE, " "),
                (RETURN_KW, "return"),
                (WHITESPACE, " "),
                (IF_KW, "if"),
                (WHITESPACE, " "),
                (ELSE_KW, "else"),
                (WHITESPACE, " "),
                (LOOP_KW, "loop"),
                (WHITESPACE, " "),
                (WHILE_KW, "while"),
                (WHITESPACE, " "),
                (BREAK_KW, "break"),
                (WHITESPACE, " "),
                (CONTINUE_KW, "continue"),
                (WHITESPACE, " "),
                (AS_KW, "as"),
                (WHITESPACE, " "),
                (EXTERN_KW, "extern"),
            ],
        )
    }

    #[test]
    fn single_tokens() {
        check(
            "(){}[]<>:;=!,-+*/.&|",
            &[
                (LEFT_PAREN, "("),
                (RIGHT_PAREN, ")"),
                (LEFT_CURLY, "{"),
                (RIGHT_CURLY, "}"),
                (LEFT_SQUARE, "["),
                (RIGHT_SQUARE, "]"),
                (LEFT_ANGLE, "<"),
                (RIGHT_ANGLE, ">"),
                (COLON, ":"),
                (SEMICOLON, ";"),
                (EQUALS, "="),
                (BANG, "!"),
                (COMMA, ","),
                (DASH, "-"),
                (PLUS, "+"),
                (STAR, "*"),
                (SLASH, "/"),
                (DOT, "."),
                (AMPERSAND, "&"),
                (BAR, "|"),
            ],
        );
    }
}
