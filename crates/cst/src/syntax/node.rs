use std::sync::Arc;

use crate::syntax::NodeOrToken;
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

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct NodeData {
    pub offset: usize,
    pub parent: Option<Node>,
    pub green: Arc<green::Node>,
    pub index: usize,
}

impl std::fmt::Debug for NodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("Data");
        if let Some(ref parent) = self.parent {
            f.field("parent", &format!("{:?}", parent.green.kind));
        }
        f.field("offset", &self.offset);
        f.field("green", &self.green);
        f.field("index", &self.index);
        f.finish()
    }
}

impl Node {
    #[inline]
    pub fn clone(other: &Node) -> Self {
        Self(Arc::clone(&other.0))
    }

    #[inline]
    pub fn root(green: Arc<green::Node>) -> Self {
        Self(Arc::new(NodeData {
            offset: 0,
            parent: None,
            green,
            index: 0,
        }))
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        self.green.kind
    }

    #[inline]
    pub fn text(&self) -> String {
        self.green.text()
    }

    #[inline]
    pub fn parent(&self) -> Option<Node> {
        self.parent.clone()
    }

    #[inline]
    pub fn parents(&self) -> impl Iterator<Item = Node> + '_ {
        std::iter::successors(Some(self.clone()), Self::parent)
    }

    #[inline]
    pub fn siblings(&self) -> impl Iterator<Item = NodeOrToken> + '_ {
        self.parent.as_ref().into_iter().flat_map(|parent| {
            parent
                .children_with_tokens()
                .filter(|child| child.index() != self.index)
        })
    }

    #[inline]
    pub fn prev(&self) -> Option<NodeOrToken> {
        self.parent.as_ref().and_then(|parent| {
            parent.child_by_index(self.index.saturating_sub(1))
        })
    }

    #[inline]
    pub fn next(&self) -> Option<NodeOrToken> {
        self.parent.as_ref().and_then(|parent| {
            parent.child_by_index(self.index.saturating_add(1))
        })
    }

    #[inline]
    fn construct_child(
        &self,
        index: usize,
        green: &green::Child,
    ) -> NodeOrToken {
        match green {
            green::Child::Node {
                relative_offset,
                node,
            } => NodeOrToken::node(
                self.offset + relative_offset,
                &self,
                node,
                index,
            ),
            green::Child::Token {
                relative_offset,
                token,
            } => NodeOrToken::token(
                self.offset + relative_offset,
                &self,
                token,
                index,
            ),
        }
    }

    #[inline]
    fn child_by_index(&self, index: usize) -> Option<NodeOrToken> {
        self.green
            .children
            .get(index)
            .map(|child| self.construct_child(index, child))
    }

    #[inline]
    pub fn children_with_tokens(
        &self,
    ) -> impl Iterator<Item = NodeOrToken> + '_ {
        self.green
            .children
            .iter()
            .enumerate()
            .map(|(idx, child)| self.construct_child(idx, child))
    }

    #[inline]
    pub fn children(&self) -> impl Iterator<Item = Node> + '_ {
        self.children_with_tokens()
            .filter_map(|child| child.into_node())
    }

    #[inline]
    pub fn child(&self, i: usize) -> Node {
        self.children().nth(i).unwrap()
    }

    #[inline]
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
                .map(|child| {
                    format!("\n{}", child.to_string_indented(indent + 2))
                })
                .collect::<String>()
        )
    }

    #[inline]
    pub fn as_node_or_token(self) -> NodeOrToken {
        NodeOrToken::Node(self)
    }
}
