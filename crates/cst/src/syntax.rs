use crate::{green, SyntaxKind};
use std::sync::Arc;

mod builder;
mod node;
mod token;

pub mod traverse;

pub use self::{builder::*, node::*, token::*};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum NodeOrToken {
    Node(Node),
    Token(Token),
}

impl From<Node> for NodeOrToken {
    fn from(n: Node) -> Self {
        NodeOrToken::Node(n)
    }
}

impl From<Token> for NodeOrToken {
    fn from(t: Token) -> Self {
        NodeOrToken::Token(t)
    }
}

impl NodeOrToken {
    #[inline]
    pub fn node(
        offset: usize,
        parent: &Node,
        node: &Arc<green::Node>,
        index: usize,
    ) -> Self {
        Self::Node(Node(Arc::new(NodeData {
            offset,
            parent: Some(Node::clone(parent)),
            green: Arc::clone(node),
            index,
        })))
    }

    #[inline]
    pub fn token(
        offset: usize,
        parent: &Node,
        token: &Arc<green::Token>,
        index: usize,
    ) -> Self {
        Self::Token(Token(Arc::new(TokenData {
            offset,
            parent: Node::clone(parent),
            green: Arc::clone(token),
            index,
        })))
    }

    #[inline]
    pub fn into_token(&self) -> Option<Token> {
        match self {
            Self::Token(tok) => Some(tok.clone()),
            _ => None,
        }
    }

    #[inline]
    pub fn into_node(&self) -> Option<Node> {
        match self {
            Self::Node(node) => Some(node.clone()),
            _ => None,
        }
    }

    #[inline]
    pub fn into_token_ref(&self) -> Option<&Token> {
        match self {
            Self::Token(tok) => Some(tok),
            _ => None,
        }
    }

    #[inline]
    pub fn into_node_ref(&self) -> Option<&Node> {
        match self {
            Self::Node(node) => Some(node),
            _ => None,
        }
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        match self {
            Self::Node(node) => node.kind(),
            Self::Token(token) => token.kind(),
        }
    }

    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        match self {
            Self::Node(node) => node.range(),
            Self::Token(token) => token.range(),
        }
    }

    #[inline]
    pub fn siblings(&self) -> impl Iterator<Item = NodeOrToken> + '_ {
        self.into_node_ref()
            .into_iter()
            .flat_map(|node| node.siblings())
    }

    #[inline]
    pub fn index(&self) -> usize {
        match self {
            Self::Node(node) => node.index,
            Self::Token(token) => token.index,
        }
    }

    #[inline]
    pub fn parent(&self) -> Option<Node> {
        match self {
            Self::Node(node) => node.parent(),
            Self::Token(token) => Some(token.parent.clone()),
        }
    }

    #[inline]
    pub fn ancestors(&self) -> impl Iterator<Item = Node> + '_ {
        std::iter::successors(self.parent(), Node::parent)
    }

    #[inline]
    pub fn children(&self) -> impl Iterator<Item = Node> + '_ {
        self.into_node_ref()
            .into_iter()
            .flat_map(|node| node.children())
    }

    #[inline]
    pub fn children_with_tokens(
        &self,
    ) -> impl Iterator<Item = NodeOrToken> + '_ {
        self.into_node_ref()
            .into_iter()
            .flat_map(|node| node.children_with_tokens())
    }

    pub fn to_string_indented(&self, indent: usize) -> String {
        match self {
            Self::Node(node) => node.to_string_indented(indent),
            Self::Token(token) => token.to_string_indented(indent),
        }
    }
}
