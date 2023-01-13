use super::*;

use cst::lexer::{self, Token, TokenCache};
use cst::SyntaxKind;

impl TokenSource for TokenStream {
    fn kind_at(&self, index: usize) -> SyntaxKind {
        self.data
            .get(index)
            .map(|token| token.kind)
            .unwrap_or(SyntaxKind::EOF)
    }
    fn text_at(&self, index: usize) -> &str {
        self.data
            .get(index)
            .map(|token| token.text.as_ref())
            .unwrap_or_default()
    }
}

pub struct TokenStream {
    pub(crate) data: Vec<Token>,
}

impl From<Vec<Token>> for TokenStream {
    fn from(data: Vec<Token>) -> Self {
        Self { data }
    }
}

pub struct Input<'text> {
    pub(crate) text: &'text str,
    pub(crate) tokens: TokenStream,
    pub(crate) token_cache: TokenCache,
    pub(crate) token_lens: Vec<usize>,
}

impl<'text> Input<'text> {
    pub fn lex(text: &'text str) -> Self {
        let tokens = lexer::lex(text);
        let token_lens =
            tokens.stream.iter().map(|token| token.len()).collect();
        Input {
            text,
            tokens: tokens.stream.into(),
            token_cache: tokens.cache,
            token_lens,
        }
    }

    pub fn tokens(&self) -> &Vec<lexer::Token> {
        &self.tokens.data
    }
}
