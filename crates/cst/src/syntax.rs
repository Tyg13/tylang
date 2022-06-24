use crate::green;
use std::sync::Arc;

mod builder;
mod node;
mod token;

pub use self::{builder::*, node::*, token::*};
pub type Element = crate::utils::Element<Node, Token>;

impl Element {
    pub fn node(offset: usize, parent: &Node, node: &Arc<green::Node>) -> Self {
        Self::Node(Node(Arc::new(NodeData {
            offset,
            parent: Some(Node::clone(parent)),
            green: Arc::clone(node),
        })))
    }

    pub fn token(offset: usize, parent: &Node, token: &Arc<green::Token>) -> Self {
        Self::Token(Token(Arc::new(TokenData {
            offset,
            parent: Node::clone(parent),
            green: Arc::clone(token),
        })))
    }

    pub fn into_token(&self) -> Option<Token> {
        match self {
            Self::Token(tok) => Some(tok.clone()),
            _ => None,
        }
    }

    pub fn into_node(&self) -> Option<Node> {
        match self {
            Self::Node(node) => Some(node.clone()),
            _ => None,
        }
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        match self {
            Self::Node(node) => node.range(),
            Self::Token(token) => token.range(),
        }
    }

    pub fn to_string_indented(&self, indent: usize) -> String {
        match self {
            Self::Node(node) => node.to_string_indented(indent),
            Self::Token(token) => token.to_string_indented(indent),
        }
    }
}
