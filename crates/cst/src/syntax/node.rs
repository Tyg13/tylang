use std::sync::Arc;

use crate::syntax::Element;
use crate::{green, SyntaxKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node(pub(crate) Arc<NodeData>);

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_indented(0))
    }
}

impl std::ops::Deref for Node {
    type Target = NodeData;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeData {
    pub offset: usize,
    pub parent: Option<Node>,
    pub green: Arc<green::Node>,
}

impl std::fmt::Debug for NodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parent = if let Some(ref parent) = self.parent {
            format!("{:?}", parent.green.kind)
        } else {
            "".to_string()
        };
        f.debug_struct("Data")
            .field("offset", &self.offset)
            .field("parent", &parent)
            .field("green", &self.green)
            .finish()
    }
}

impl Node {
    pub fn clone(other: &Node) -> Self {
        Self(Arc::clone(&other.0))
    }

    pub fn root(green: Arc<green::Node>) -> Self {
        Self(Arc::new(NodeData {
            offset: 0,
            parent: None,
            green,
        }))
    }

    pub fn kind(&self) -> SyntaxKind {
        self.green.kind
    }

    pub fn text(&self) -> String {
        self.green.text()
    }

    pub fn parent(&self) -> Option<Node> {
        self.parent.clone()
    }

    pub fn parents(&self) -> impl Iterator<Item = Node> + '_ {
        std::iter::successors(Some(self.clone()), Self::parent)
    }

    pub fn children_with_tokens(&self) -> impl Iterator<Item = Element> + '_ {
        self.green.children.iter().map(move |child| match child {
            green::Child::Node {
                relative_offset,
                node,
            } => Element::node(self.offset + *relative_offset, &self, node),
            green::Child::Token {
                relative_offset,
                token,
            } => Element::token(self.offset + *relative_offset, &self, token),
        })
    }

    pub fn children(&self) -> impl Iterator<Item = Node> + '_ {
        self.children_with_tokens().filter_map(|child| match child {
            Element::Node(node) => Some(node),
            _ => None,
        })
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        let (start, mut end) = (self.offset, self.offset);
        for child in self.children_with_tokens() {
            end = child.range().end;
        }
        start..end
    }

    pub fn to_string_indented(&self, indent: usize) -> String {
        let (start, end) = (self.range().start, self.range().end);
        format!(
            "{indent}{kind:?} @ {start}..{end}:{children}",
            indent = str::repeat(" ", indent),
            kind = self.kind(),
            children = self
                .children_with_tokens()
                .map(|child| { format!("\n{}", child.to_string_indented(indent + 2)) })
                .collect::<String>()
        )
    }
}
