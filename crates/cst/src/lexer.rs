use crate::hash::hash;
use crate::{green, SyntaxKind};
use std::collections::HashMap;
use std::iter::Peekable;
use std::str::Chars;
use std::sync::Arc;

pub type Token = Arc<green::Token>;
pub type TokenCache = HashMap<(SyntaxKind, u64), Token>;

pub struct Lexer<'source> {
    offset: usize,
    chars: Peekable<Chars<'source>>,
    pub(crate) token_cache: TokenCache,
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
        ' ' | '\n' | '\r'
    };
}

impl<'source> Lexer<'source> {
    pub fn new(input: &'source str) -> Self {
        Self {
            offset: 0,
            chars: input.chars().peekable(),
            token_cache: Default::default(),
        }
    }
}

impl Lexer<'_> {
    pub fn lex_one(&mut self) -> Token {
        let token = match self.peek() {
            Some(token) => token,
            None => return self.token(SyntaxKind::EOF, "".to_string()),
        };
        match token {
            '(' => self.single(green::SyntaxKind::LEFT_PAREN),
            ')' => self.single(green::SyntaxKind::RIGHT_PAREN),
            '{' => self.single(green::SyntaxKind::LEFT_CURLY),
            '}' => self.single(green::SyntaxKind::RIGHT_CURLY),
            '[' => self.single(green::SyntaxKind::LEFT_SQUARE),
            ']' => self.single(green::SyntaxKind::RIGHT_SQUARE),
            '<' => self.single(green::SyntaxKind::LEFT_ANGLE),
            '>' => self.single(green::SyntaxKind::RIGHT_ANGLE),
            ':' => self.single(green::SyntaxKind::COLON),
            ';' => self.single(green::SyntaxKind::SEMICOLON),
            '=' => self.single(green::SyntaxKind::EQUALS),
            ',' => self.single(green::SyntaxKind::COMMA),
            '-' => self.single(green::SyntaxKind::DASH),
            '+' => self.single(green::SyntaxKind::PLUS),
            '*' => self.single(green::SyntaxKind::STAR),
            '.' => self.single(green::SyntaxKind::DOT),
            '&' => self.single(green::SyntaxKind::AMPERSAND),
            '|' => self.single(green::SyntaxKind::BAR),
            '"' => self.string(),
            '/' => {
                self.advance();
                if self.peek() == Some('/') {
                    self.comment()
                } else {
                    self.token(green::SyntaxKind::SLASH, "/".to_string())
                }
            }
            start_ident!() => self.ident_or_keyword(),
            number!() => self.number(),
            whitespace!() => self.whitespace(),
            _ => self.single(green::SyntaxKind::ERROR),
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next();
        self.offset += 1;
        c
    }

    fn token(&mut self, kind: SyntaxKind, text: String) -> Token {
        self.token_cache
            .entry((kind, hash(&text)))
            .or_insert_with(|| Arc::new(green::Token { kind, text }))
            .clone()
    }

    fn single(&mut self, kind: green::SyntaxKind) -> Token {
        let c = self.advance().unwrap();
        self.token(kind, c.to_string())
    }

    fn matching_range(&mut self, accept: impl Fn(char) -> bool) -> String {
        let mut ret = String::new();
        while let Some(c) = self.peek() {
            if accept(c) {
                ret.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        ret
    }

    fn lex_kind(
        &mut self,
        kind: green::SyntaxKind,
        accept: impl Fn(char) -> bool,
    ) -> Token {
        let text = self.matching_range(accept);
        self.token(kind, text)
    }

    fn ident_or_keyword(&mut self) -> Token {
        let start = self.matching_range(is_start_ident);
        let end = self.matching_range(is_ident);
        let text = format!("{start}{end}");
        let kind = match text.as_str() {
            "mod" => SyntaxKind::MOD_KW,
            "type" => SyntaxKind::TYPE_KW,
            "fn" => SyntaxKind::FN_KW,
            "let" => SyntaxKind::LET_KW,
            "return" => SyntaxKind::RETURN_KW,
            "if" => SyntaxKind::IF_KW,
            "else" => SyntaxKind::ELSE_KW,
            "loop" => SyntaxKind::LOOP_KW,
            "while" => SyntaxKind::WHILE_KW,
            "break" => SyntaxKind::BREAK_KW,
            "continue" => SyntaxKind::CONTINUE_KW,
            "as" => SyntaxKind::AS_KW,
            _ => SyntaxKind::IDENT,
        };
        self.token(kind, text)
    }

    fn whitespace(&mut self) -> Token {
        self.lex_kind(green::SyntaxKind::WHITESPACE, is_whitespace)
    }

    fn number(&mut self) -> Token {
        self.lex_kind(green::SyntaxKind::NUMBER, is_number)
    }

    fn string(&mut self) -> Token {
        let start = self.advance().unwrap();
        let mut end = self.matching_range(|c| c != '"');
        let kind = if let Some('"') = self.peek() {
            end.push(self.advance().unwrap());
            SyntaxKind::STRING
        } else {
            SyntaxKind::ERROR
        };
        self.token(kind, format!("{start}{end}"))
    }

    fn comment(&mut self) -> Token {
        self.advance().unwrap();
        let contents = self.matching_range(|c| c != '\n');
        self.token(SyntaxKind::COMMENT, format!("//{contents}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use green::SyntaxKind::*;

    macro_rules! assert_token {
        ($lexer:expr, $kind:expr, $text:expr) => {{
            let token = $lexer.lex_one();
            assert_eq!(token.kind, $kind);
            assert_eq!(token.text, $text);
            token
        }};
    }

    macro_rules! assert_done {
        ($lexer:expr) => {
            let token = $lexer.lex_one();
            assert_eq!(token.kind, EOF);
        };
    }

    #[test]
    fn ident() {
        let mut lexer = Lexer::new("foo");
        assert_token!(lexer, IDENT, "foo");
        assert_done!(lexer);
    }

    #[test]
    fn whitespace() {
        let mut lexer = Lexer::new("  ");
        assert_token!(lexer, WHITESPACE, "  ");
        assert_done!(lexer);
    }

    #[test]
    fn number() {
        let mut lexer = Lexer::new("123");
        assert_token!(lexer, NUMBER, "123");
    }

    #[test]
    fn string() {
        let mut lexer = Lexer::new("\"foobar\"\"bazbaz\"");
        assert_token!(lexer, STRING, "\"foobar\"");
        assert_token!(lexer, STRING, "\"bazbaz\"");
        assert_done!(lexer);
    }

    #[test]
    fn none() {
        let mut lexer = Lexer::new("");
        assert_done!(lexer);
    }

    #[test]
    fn interning() {
        let mut lexer = Lexer::new("foo foo");
        let foo_1 = assert_token!(lexer, IDENT, "foo");
        let _ = assert_token!(lexer, WHITESPACE, " ");
        let foo_2 = assert_token!(lexer, IDENT, "foo");
        assert!(Arc::ptr_eq(&foo_1, &foo_2));
    }

    #[test]
    fn keywords() {
        let mut lexer = Lexer::new(
            "mod type let fn return if else loop while break continue as",
        );
        assert_token!(lexer, MOD_KW, "mod");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, TYPE_KW, "type");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, LET_KW, "let");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, FN_KW, "fn");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, RETURN_KW, "return");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, IF_KW, "if");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, ELSE_KW, "else");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, LOOP_KW, "loop");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, WHILE_KW, "while");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, BREAK_KW, "break");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, CONTINUE_KW, "continue");
        assert_token!(lexer, WHITESPACE, " ");
        assert_token!(lexer, AS_KW, "as");
        assert_done!(lexer);
    }

    #[test]
    fn single_tokens() {
        let mut lexer = Lexer::new("(){}[]<>:;=,-+*/.&|");
        assert_token!(lexer, LEFT_PAREN, "(");
        assert_token!(lexer, RIGHT_PAREN, ")");
        assert_token!(lexer, LEFT_CURLY, "{");
        assert_token!(lexer, RIGHT_CURLY, "}");
        assert_token!(lexer, LEFT_SQUARE, "[");
        assert_token!(lexer, RIGHT_SQUARE, "]");
        assert_token!(lexer, LEFT_ANGLE, "<");
        assert_token!(lexer, RIGHT_ANGLE, ">");
        assert_token!(lexer, COLON, ":");
        assert_token!(lexer, SEMICOLON, ";");
        assert_token!(lexer, EQUALS, "=");
        assert_token!(lexer, COMMA, ",");
        assert_token!(lexer, DASH, "-");
        assert_token!(lexer, PLUS, "+");
        assert_token!(lexer, STAR, "*");
        assert_token!(lexer, SLASH, "/");
        assert_token!(lexer, DOT, ".");
        assert_token!(lexer, AMPERSAND, "&");
        assert_token!(lexer, BAR, "|");
        assert_done!(lexer);
    }
}
