use crate::hash::hash;
use std::sync::Arc;

mod kinds;
pub use kinds::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Child {
    Node {
        relative_offset: usize,
        node: Arc<Node>,
    },
    Token {
        relative_offset: usize,
        token: Arc<Token>,
    },
}

impl Child {
    pub fn len(&self) -> usize {
        match self {
            Self::Node { node, .. } => node.len,
            Self::Token { token, .. } => token.text.len(),
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        match self {
            Self::Node { node, .. } => node.kind,
            Self::Token { token, .. } => token.kind,
        }
    }

    pub fn text(&self) -> String {
        match self {
            Self::Node { node, .. } => node.text(),
            Self::Token { token, .. } => token.text.to_string(),
        }
    }

    fn to_string_indented(&self, indent: usize) -> String {
        match self {
            Self::Node { node, .. } => node.to_string_indented(indent),
            Self::Token { token, .. } => token.to_string_indented(indent),
        }
    }

    pub fn into_node(&self) -> Option<&Arc<Node>> {
        match self {
            Self::Node { node, .. } => Some(node),
            _ => None,
        }
    }

    pub fn into_token(&self) -> Option<&Arc<Token>> {
        match self {
            Self::Token { token, .. } => Some(token),
            _ => None,
        }
    }

    pub fn as_node(&self) -> &Arc<Node> {
        self.into_node().unwrap()
    }

    pub fn as_token(&self) -> &Arc<Token> {
        self.into_token().unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node {
    pub kind: SyntaxKind,
    pub len: usize,
    pub children: Vec<Child>,
}

impl Node {
    pub fn text(&self) -> String {
        self.children.iter().map(|child| child.text()).collect()
    }

    fn to_string_indented(&self, indent: usize) -> String {
        format!(
            "{indent}{kind:?}:{children}",
            indent = str::repeat(" ", indent),
            kind = self.kind,
            children = self
                .children
                .iter()
                .map(|child| { format!("\n{}", child.to_string_indented(indent + 2)) })
                .collect::<String>()
        )
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_indented(0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: SyntaxKind,
    pub text: String,
}

impl Token {
    pub fn len(&self) -> usize {
        self.text.len()
    }

    fn to_string_indented(&self, indent: usize) -> String {
        format!(
            "{indent}{kind:?}: '{text}'",
            indent = str::repeat(" ", indent),
            kind = self.kind,
            text = self.text
        )
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_indented(0))
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeBuilder {
    relative_offset: usize,
    token_cache: crate::lexer::TokenCache,
    parents: Vec<(SyntaxKind, usize, usize)>,
    children: Vec<Child>,
}

impl NodeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_cache(token_cache: crate::lexer::TokenCache) -> Self {
        let mut this = Self::new();
        this.token_cache = token_cache;
        this
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        let token = self
            .token_cache
            .entry((kind, hash(text)))
            .or_insert_with(|| {
                Arc::new(Token {
                    kind,
                    text: text.to_string(),
                })
            })
            .clone();
        let relative_offset = self.relative_offset;
        self.relative_offset += token.len();
        self.children.push(Child::Token {
            relative_offset,
            token,
        });
    }

    pub fn start_node(&mut self, kind: SyntaxKind) {
        let index = self.children.len();
        self.parents.push((kind, index, self.relative_offset));
        self.relative_offset = 0;
    }

    pub fn finish_node(&mut self) {
        let (kind, index, relative_offset) = self.parents.pop().unwrap();
        let children: Vec<_> = self.children.drain(index..).collect();
        let len = children.iter().map(|child| child.len()).sum();
        self.children.push(Child::Node {
            relative_offset,
            node: Arc::new(Node {
                kind,
                len,
                children,
            }),
        });
        self.relative_offset = relative_offset + len;
    }

    pub fn finish(mut self) -> Arc<Node> {
        assert_eq!(self.children.len(), 1);
        match self.children.pop().unwrap() {
            Child::Node { node, .. } => node,
            Child::Token { .. } => panic!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_node() {
        let node = {
            let mut builder = NodeBuilder::new();
            builder.start_node(SyntaxKind::LET_ITEM);
            builder.token(SyntaxKind::LET_KW, "let");
            builder.token(SyntaxKind::WHITESPACE, " ");
            builder.token(SyntaxKind::IDENT, "foo");
            builder.finish_node();
            builder.finish()
        };
        assert_eq!(node.len, 7);
        assert_eq!(node.kind, SyntaxKind::LET_ITEM);
    }

    #[test]
    fn node_printer() {
        let node = {
            let mut builder = NodeBuilder::new();
            builder.start_node(SyntaxKind::LET_ITEM);
            builder.token(SyntaxKind::LET_KW, "let");
            builder.token(SyntaxKind::WHITESPACE, " ");
            builder.start_node(SyntaxKind::NAME);
            builder.token(SyntaxKind::IDENT, "foo");
            builder.finish_node();
            builder.token(SyntaxKind::COLON, ":");
            builder.start_node(SyntaxKind::BASIC_TYPE);
            builder.token(SyntaxKind::IDENT, "i64");
            builder.finish_node();
            builder.finish_node();
            builder.finish()
        };
        assert_eq!(node.len, 11);
        assert_eq!(node.children[0].as_token().text, "let");
        assert_eq!(node.children[2].as_node().len, 3);
        assert_eq!(
            node.to_string(),
            r#"
LET_ITEM:
  LET_KW: 'let'
  WHITESPACE: ' '
  NAME:
    IDENT: 'foo'
  COLON: ':'
  BASIC_TYPE:
    IDENT: 'i64'
"#
            .trim()
        );
    }
}
