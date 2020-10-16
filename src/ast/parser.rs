use crate::ast::error::ErrorHandler;
use crate::ast::{ExpectedError, Tree};
use crate::lexer::{Token, TokenKind, Tokens};

pub struct Parser<'tokens, 'error> {
    tokens: &'tokens Tokens<'tokens>,
    pub(crate) error_handler: &'error mut dyn ErrorHandler,
    token_index: usize,
}

pub trait Parse: Sized {
    fn parse(parser: &mut Parser) -> Option<Self>;
}

impl<'tokens, 'error> Parser<'tokens, 'error> {
    /// Create a new `Parser` from the given `Tokens` and with the given `ErrorHandler`.
    pub fn new(
        tokens: &'tokens Tokens<'tokens>,
        error_handler: &'error mut dyn ErrorHandler,
    ) -> Self {
        Self {
            tokens,
            error_handler,
            token_index: 0,
        }
    }

    /// Parse the entire `ast::Tree`
    pub fn parse(&mut self) -> Tree {
        self.parse_one().unwrap()
    }

    /// Parse a single instance of a given `T`.
    pub fn parse_one<T: Parse>(&mut self) -> Option<T> {
        <T as Parse>::parse(self)
    }

    /// Return the token at the current `token_index`
    pub fn peek(&self) -> Option<Token<'tokens>> {
        self.tokens.get(self.token_index).cloned()
    }

    /// Returns if the parser has consumed all tokens.
    pub fn eof(&self) -> bool {
        self.peek().is_none()
    }

    /// Unconditionally advance to the next token.
    pub fn advance(&mut self) {
        self.token_index += 1;
    }

    /// Check the next token. If it is `kind`, then consume and return it. Otherwise, return
    /// `None`.
    pub fn maybe(&mut self, kind: TokenKind) -> Option<Token<'tokens>> {
        self.peek()
            .filter(|token| token.kind() == kind)
            .map(|token| {
                self.advance();
                token
            })
    }

    /// Check the next token. If it is `kind`, then consume and return it. Otherwise, issue an
    /// error diagnostic and return `None`.
    pub fn expect(&mut self, kind: TokenKind) -> Option<Token<'tokens>> {
        self.maybe(kind).or_else(|| {
            let token = self.peek();
            let advance = token.is_some();
            let error = ExpectedError::new(kind, token);
            self.issue_diagnostic(format!("{}", error));
            if advance {
                self.advance();
            }
            None
        })
    }

    /// Try to perform `action` on `self`, returning an `Option<T>`. If the value returned by
    /// `action` is `None`, backtrack to before `action` was performed.
    pub fn some_or_backtrack<T>(
        &mut self,
        action: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let before = self.token_index;
        action(self).or_else(|| {
            self.token_index = before;
            None
        })
    }

    /// `advance` while `predicate` is satisfied for the current token.
    pub fn advance_while(&mut self, predicate: impl Fn(&Token) -> bool) {
        while let Some(token) = self.peek() {
            if !predicate(&token) {
                break;
            }
            self.advance();
        }
    }

    /// `advance` until the current token is `kind`
    pub fn advance_until(&mut self, kind: TokenKind) {
        self.advance_until_one_of(&[kind]);
    }

    /// `advance` until the current token's kind is one of `kinds`.
    pub fn advance_until_one_of(&mut self, kinds: &[TokenKind]) {
        self.advance_while(|token| !kinds.contains(&token.kind()));
    }

    /// `advance` until the current token's kind is one of `kinds`, and then `advance` past it.
    pub fn advance_past_one_of(&mut self, kinds: &[TokenKind]) {
        self.advance_until_one_of(kinds);
        self.advance();
    }

    /// `advance` until the current token's kind is one of `kinds`, and then `advance` past it.
    pub fn advance_past(&mut self, kind: TokenKind) {
        self.advance_past_one_of(&[kind]);
    }
}

#[cfg(test)]
mod test {
    use crate::ast::*;
    use crate::lexer::TokenKind;

    struct DummyHandler;

    fn leak<'a, T>(t: T) -> &'a mut T {
        Box::leak(Box::new(t))
    }

    impl ErrorHandler for DummyHandler {
        fn issue_diagnostic(&mut self, _error: &Error) {}
    }

    impl<'error> Parser<'_, 'error> {
        fn test_with_source_and_handler(
            source: &Source,
            error_handler: &'error mut dyn ErrorHandler,
        ) -> Self {
            let map = leak(crate::lexer::lex(source));
            let tokens = leak(map.tokens().strip_comments_and_whitespace());
            Self::new(tokens, error_handler)
        }

        pub fn test(text: &str) -> Self {
            let source = leak(crate::util::SourceBuilder::new().source(text).build());
            Self::test_with_source_and_handler(source, leak(DummyHandler))
        }

        pub fn test_with_log(text: &str, log: &'error mut dyn std::io::Write) -> Self {
            let source = leak(crate::util::SourceBuilder::new().source(text).build());
            let handler = leak(StreamHandler::new(source, log));
            Self::test_with_source_and_handler(source, handler)
        }

        fn peek_repr(&self) -> String {
            self.peek().unwrap().repr()
        }
    }

    #[test]
    fn peek_none() {
        let parser = Parser::test("");
        assert!(parser.eof());
    }

    #[test]
    fn peek_one() {
        let parser = Parser::test("1");
        assert_eq!(parser.peek_repr(), "1");
    }

    #[test]
    fn advance_until_normal() {
        let mut parser = Parser::test("let n: 10 = 20;");
        parser.advance_until(TokenKind::SemiColon);
        assert_eq!(parser.peek_repr(), ";");
        assert_eq!(parser.token_index, 6);
    }

    #[test]
    fn advance_until_do_nothing() {
        let mut parser = Parser::test("let m = *");
        let before = parser.token_index;
        parser.advance_until(TokenKind::Let);
        let after = parser.token_index;
        assert_eq!(parser.peek_repr(), "let");
        assert_eq!(before, after);
    }

    #[test]
    fn advance_past_current() {
        let mut parser = Parser::test("this 10");
        parser.advance_past(TokenKind::Identifier);
        assert_eq!(parser.peek_repr(), "10");
    }

    #[test]
    fn advance_past_later() {
        let mut parser = Parser::test("this = 10 + ; wow");
        parser.advance_past(TokenKind::SemiColon);
        assert_eq!(parser.peek_repr(), "wow");
    }

    #[test]
    fn advance_past_none() {
        let mut parser = Parser::test("blue 42");
        parser.advance_past(TokenKind::Let);
        assert!(parser.eof());
    }

    #[test]
    fn maybe_none() {
        let mut parser = Parser::test("if");
        let before = parser.token_index;
        let result = parser.maybe(TokenKind::Let);
        let after = parser.token_index;
        assert!(result.is_none());
        assert_eq!(before, after);
    }

    #[test]
    fn maybe_some() {
        let mut parser = Parser::test("10 &&");
        let repr = parser.maybe(TokenKind::Number).unwrap().repr();
        assert_eq!(repr, "10");
        assert_eq!(parser.peek_repr(), "&&");
    }

    #[test]
    fn maybe_eof() {
        let mut parser = Parser::test("");
        assert!(parser.eof());
        let result = parser.maybe(TokenKind::Identifier);
        assert!(result.is_none());
    }

    #[test]
    fn some_or_backtrack_some() {
        let mut parser = Parser::test("10 + 20");
        let (first, plus, second) = parser
            .some_or_backtrack(|parser| {
                let first = parser.maybe(TokenKind::Number)?;
                let plus = parser.maybe(TokenKind::Plus)?;
                let second = parser.maybe(TokenKind::Number)?;
                Some((first, plus, second))
            })
            .unwrap();
        assert_eq!(first.repr(), "10");
        assert_eq!(plus.repr(), "+");
        assert_eq!(second.repr(), "20");
        assert!(parser.eof());
    }

    #[test]
    fn some_or_backtrack_none() {
        let mut parser = Parser::test("meow + 20");
        let before = parser.token_index;
        let result = parser.some_or_backtrack(|parser| {
            let first = parser.maybe(TokenKind::Identifier)?;
            let plus = parser.maybe(TokenKind::Plus)?;
            let second = parser.maybe(TokenKind::Identifier)?;
            Some((first, plus, second))
        });
        let after = parser.token_index;
        assert!(result.is_none());
        assert_eq!(before, after);
    }
}
