use crate::syntax::NodeOrToken;

pub trait Visitor: Sized {
    fn visit(&mut self, node: NodeOrToken);
}

pub fn preorder<V: Visitor>(visitor: &mut V, node: NodeOrToken) {
    visitor.visit(node.clone());
    for child in node.children_with_tokens() {
        preorder(visitor, child);
    }
}

pub fn postorder<V: Visitor>(visitor: &mut V, node: NodeOrToken) {
    for child in node.children_with_tokens() {
        postorder(visitor, child);
    }
    visitor.visit(node);
}

pub enum Step<R> {
    Continue(NodeOrToken),
    Terminate(R),
}

pub fn iterate<R>(node: NodeOrToken, mut f: impl FnMut(NodeOrToken) -> Step<R>) -> R {
    match f(node) {
        Step::Continue(node) => iterate(node, f),
        Step::Terminate(result) => result,
    }
}
