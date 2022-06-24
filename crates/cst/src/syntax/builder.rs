use crate::green;
use crate::syntax::Node;

#[derive(Debug, Clone, Default)]
pub struct Builder {
    green: green::NodeBuilder,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_cache(cache: crate::lexer::TokenCache) -> Self {
        Self {
            green: green::NodeBuilder::new_with_cache(cache),
        }
    }

    pub fn token(&mut self, kind: green::SyntaxKind, text: &str) {
        self.green.token(kind, text);
    }

    pub fn start_node(&mut self, kind: green::SyntaxKind) {
        self.green.start_node(kind);
    }

    pub fn finish_node(&mut self) {
        self.green.finish_node();
    }

    pub fn finish(self) -> Node {
        Node::root(self.green.finish())
    }
}
