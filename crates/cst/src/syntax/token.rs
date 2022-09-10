use std::sync::Arc;

use crate::syntax::{Node, NodeOrToken};
use crate::{green, SyntaxKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token(pub(crate) Arc<TokenData>);

impl Token {
    pub fn as_node_or_token(self) -> NodeOrToken {
        NodeOrToken::Token(self)
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_indented(0))
    }
}

impl std::fmt::Debug for TokenData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("offset", &self.offset)
            .field("parent", &self.parent.green.kind)
            .field("green", &self.green)
            .field("index", &self.index)
            .finish()
    }
}

impl std::ops::Deref for Token {
    type Target = TokenData;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenData {
    pub offset: usize,
    pub parent: Node,
    pub green: Arc<green::Token>,
    pub index: usize,
}

impl TokenData {
    #[inline]
    pub fn text(&self) -> &str {
        self.green.text.as_str()
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        self.green.kind
    }

    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.offset..self.offset + self.text().len()
    }

    #[inline]
    pub fn parent(&self) -> Node {
        self.parent.clone()
    }

    #[inline]
    pub fn parents(&self) -> impl Iterator<Item = Node> + '_ {
        std::iter::successors(Some(self.parent()), Node::parent)
    }

    pub fn to_string_indented(&self, indent: usize) -> String {
        let (start, end) = (self.range().start, self.range().end);
        format!(
            "{indent}{kind:?} @ {start}..{end}: '{text}' ",
            indent = str::repeat(" ", indent),
            kind = self.kind(),
            text = self.text().escape_debug().collect::<String>(),
        )
    }
}
