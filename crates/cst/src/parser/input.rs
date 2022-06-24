use super::*;

use crate::lexer::{self, Lexer};
use crate::SyntaxKind;

pub(crate) struct Tokens {
    inner: Vec<lexer::Token>,
}
impl TokenSource for Tokens {
    fn kind_at(&self, index: usize) -> SyntaxKind {
        self.inner
            .get(index)
            .map(|token| token.kind)
            .unwrap_or(SyntaxKind::EOF)
    }
    fn text_at(&self, index: usize) -> &str {
        self.inner
            .get(index)
            .map(|token| token.text.as_ref())
            .unwrap_or("")
    }
}
impl Tokens {
    fn new(tokens: Vec<lexer::Token>) -> Self {
        Self { inner: tokens }
    }
}

pub struct Input<'source> {
    pub(crate) source: &'source str,
    pub(crate) tokens: Tokens,
    pub(crate) token_cache: lexer::TokenCache,
    pub(crate) token_lens: Vec<usize>,
}

impl<'source> Input<'source> {
    pub fn lex(s: &'source str) -> Self {
        let source = s.as_ref();
        let (tokens, token_cache) = {
            let mut lexer = Lexer::new(source);
            let mut tokens = Vec::new();
            loop {
                let token = lexer.lex_one();
                let is_eof = token.kind == SyntaxKind::EOF;
                tokens.push(token);
                if is_eof {
                    break (tokens, lexer.token_cache);
                }
            }
        };
        let token_lens = tokens.iter().map(|token| token.len()).collect();
        let tokens = Tokens::new(tokens);
        Input {
            source,
            tokens,
            token_cache,
            token_lens,
        }
    }

    pub fn tokens(&self) -> &Vec<lexer::Token> {
        &self.tokens.inner
    }
}
