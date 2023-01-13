use crate::syntax::NodeOrToken;

pub trait Visitor: Sized {
    fn visit(&mut self, node: NodeOrToken);
}

pub fn preorder<V: Visitor>(visitor: &mut V, node: impl Into<NodeOrToken>) {
    let node = node.into();
    visitor.visit(node.clone());
    for child in node.children_with_tokens() {
        preorder(visitor, child);
    }
}

pub fn postorder<V: Visitor>(visitor: &mut V, node: impl Into<NodeOrToken>) {
    let node = node.into();
    for child in node.children_with_tokens() {
        postorder(visitor, child);
    }
    visitor.visit(node);
}

pub enum Step<N, R> {
    Continue(N),
    Terminate(R),
}

pub fn iterate<N, R>(mut cursor: N, mut f: impl FnMut(N) -> Step<N, R>) -> R {
    loop {
        match f(cursor) {
            Step::Continue(next) => cursor = next,
            Step::Terminate(result) => return result,
        }
    }
}
