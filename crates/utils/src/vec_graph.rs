use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub struct Edge<T>(NodeRef<T>, NodeRef<T>);

impl<T> PartialEq for Edge<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T> Eq for Edge<T> {}

#[derive(Debug, Clone)]
pub struct VecGraph<T> {
    pub(crate) vertices: Vec<T>,
    pub(crate) edges: Vec<Edge<T>>,
    pub(crate) predecessors: Vec<Vec<NodeRef<T>>>,
    pub(crate) successors: Vec<Vec<NodeRef<T>>>,
    pub(crate) start: Option<NodeRef<T>>,
}

impl<T> std::default::Default for VecGraph<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> VecGraph<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            vertices: Default::default(),
            edges: Default::default(),
            predecessors: Default::default(),
            successors: Default::default(),
            start: None,
        }
    }

    #[inline]
    pub fn add_vertex(&mut self, vertex: T) -> NodeRef<T> {
        let node = NodeRef::new(self.vertices.len());
        if self.start.is_none() {
            self.set_start(node);
        }
        self.vertices.push(vertex);
        self.predecessors.push(Vec::new());
        self.predecessors.push(Vec::new());
        self.successors.push(Vec::new());
        node
    }

    #[inline]
    pub fn add_successor(&mut self, node: NodeRef<T>, vertex: T) -> NodeRef<T> {
        let succ = self.add_vertex(vertex);
        self.add_edge(node, succ);
        succ
    }

    #[inline]
    pub fn add_predecessor(
        &mut self,
        node: NodeRef<T>,
        vertex: T,
    ) -> NodeRef<T> {
        let pred = self.add_vertex(vertex);
        self.add_edge(pred, node);
        pred
    }

    #[inline]
    pub fn add_edge(&mut self, from: NodeRef<T>, to: NodeRef<T>) {
        let edge = Edge(from, to);
        if self.edges.contains(&edge) {
            return;
        }
        self.edges.push(edge);
        self.successors[from.idx].push(to);
        self.predecessors[to.idx].push(from);
    }

    pub fn start(&self) -> NodeRef<T> {
        self.start.unwrap()
    }

    pub fn set_start(&mut self, node: NodeRef<T>) {
        self.start = Some(node);
    }

    #[inline]
    pub fn last(&self) -> Option<NodeRef<T>> {
        self.vertices().last()
    }

    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    #[inline]
    pub fn vertices(&self) -> impl Iterator<Item = NodeRef<T>> + '_ {
        (0..self.vertices.len()).map(|i| NodeRef::new(i))
    }

    #[inline]
    pub fn successors(&self, node: &NodeRef<T>) -> &[NodeRef<T>] {
        self.successors[node.idx].as_slice()
    }

    #[inline]
    pub fn predecessors(&self, node: &NodeRef<T>) -> &[NodeRef<T>] {
        self.predecessors[node.idx].as_slice()
    }
}

impl<T: PartialEq> VecGraph<T> {
    pub fn find_vertex(&self, data: &T) -> Option<NodeRef<T>> {
        self.vertices
            .iter()
            .enumerate()
            .find_map(|(idx, v)| (v == data).then(|| NodeRef::new(idx)))
    }
}

impl<T: ToString> VecGraph<T> {
    pub fn to_dot_string(&self) -> String {
        let indent = " ".repeat(3);
        let edges = self
            .edges
            .iter()
            .map(|Edge(start, end)| {
                format!("{indent}{} -> {};", start.idx, end.idx,)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let nodes = self
            .vertices
            .iter()
            .enumerate()
            .map(|(idx, data)| {
                format!("{indent}{idx} [label=\"{}\"];", data.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("digraph g {{\n{edges}\n{nodes}\n}}")
    }
}

pub fn write_to_dot_file<T: ToString>(
    g: &VecGraph<T>,
    path: &str,
) -> std::io::Result<()> {
    std::fs::write(path, g.to_dot_string())
}

#[derive(Debug)]
pub struct NodeRef<T> {
    idx: usize,
    _data: std::marker::PhantomData<T>,
}

impl<T> std::hash::Hash for NodeRef<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.idx.hash(state)
    }
}

impl<T> PartialEq for NodeRef<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<T> Eq for NodeRef<T> {}

impl<T> Clone for NodeRef<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(self.idx)
    }
}

impl<T> Copy for NodeRef<T> {}

impl<'graph, T: 'graph> NodeRef<T> {
    #[inline]
    fn new(idx: usize) -> Self {
        NodeRef {
            idx,
            _data: std::marker::PhantomData,
        }
    }

    #[inline]
    pub fn in_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        self.predecessors(graph).len()
    }

    #[inline]
    pub fn out_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        self.successors(graph).len()
    }

    #[inline]
    pub fn data(&self, graph: &'graph VecGraph<T>) -> &'graph T {
        graph.vertices.get(self.idx).unwrap()
    }

    #[inline]
    pub fn data_mut(&self, graph: &'graph mut VecGraph<T>) -> &'graph mut T {
        graph.vertices.get_mut(self.idx).unwrap()
    }

    #[inline]
    pub fn successors(
        &self,
        graph: &'graph VecGraph<T>,
    ) -> &'graph [NodeRef<T>] {
        graph.successors(self)
    }

    #[inline]
    pub fn predecessors(
        &self,
        graph: &'graph VecGraph<T>,
    ) -> &'graph [NodeRef<T>] {
        graph.predecessors(self)
    }
}

mod detail {
    use super::*;
    use std::collections::HashSet;

    pub fn pre_post_order_impl<T, Pre, Post, const REVERSE: bool>(
        graph: &VecGraph<T>,
        node: NodeRef<T>,
        visited: &mut HashSet<NodeRef<T>>,
        pre: &mut Pre,
        post: &mut Post,
    ) where
        Pre: FnMut(NodeRef<T>),
        Post: FnMut(NodeRef<T>),
    {
        if !visited.insert(node) {
            return;
        }

        pre(node);

        let visit = |successor: &NodeRef<_>| {
            pre_post_order_impl::<T, _, _, REVERSE>(
                graph, *successor, visited, pre, post,
            );
        };
        let successors = node.successors(graph).iter();
        if REVERSE {
            successors.rev().for_each(visit)
        } else {
            successors.for_each(visit)
        }

        post(node);
    }

    pub fn skip<T>(_: NodeRef<T>) {}

    #[inline]
    pub fn pre_post_order<T, Pre, Post, const REVERSE: bool>(
        graph: &VecGraph<T>,
        pre: &mut Pre,
        post: &mut Post,
    ) where
        Pre: FnMut(NodeRef<T>),
        Post: FnMut(NodeRef<T>),
    {
        let mut visited = HashSet::new();
        let start = graph.start();
        pre_post_order_impl::<_, _, _, REVERSE>(
            graph,
            start,
            &mut visited,
            pre,
            post,
        );
    }
}

#[inline]
pub fn pre_order<T, F>(graph: &VecGraph<T>, f: &mut F)
where
    F: FnMut(NodeRef<T>),
{
    detail::pre_post_order::<_, _, _, false>(graph, f, &mut detail::skip)
}

#[inline]
pub fn post_order<T, F>(graph: &VecGraph<T>, f: &mut F)
where
    F: FnMut(NodeRef<T>),
{
    detail::pre_post_order::<_, _, _, false>(graph, &mut detail::skip, f)
}

#[inline]
pub fn reverse_post_order<T, F>(graph: &VecGraph<T>, f: &mut F)
where
    F: FnMut(NodeRef<T>),
{
    let mut nodes = Vec::with_capacity(graph.vertices.len());
    detail::pre_post_order::<_, _, _, true>(
        graph,
        &mut detail::skip,
        &mut |node| nodes.push(node),
    );
    nodes.iter().rev().for_each(|node| f(*node));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pred_string<T: ToString>(g: &VecGraph<T>, n: NodeRef<T>) -> String {
        n.predecessors(&g)
            .iter()
            .map(|node| node.data(&g).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn succ_string<T: ToString>(g: &VecGraph<T>, n: NodeRef<T>) -> String {
        n.successors(&g)
            .iter()
            .map(|node| node.data(&g).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn start() {
        let g = {
            let mut g = VecGraph::new();
            g.add_vertex("foo");
            g
        };
        assert_eq!("foo", g.start().data(&g).to_string());
    }

    #[test]
    fn successors() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("foo");
            g.add_successor(start, "succ1");
            g.add_successor(start, "succ2");
            g
        };
        let succs = succ_string(&g, g.start());
        assert_eq!(succs, "succ1 succ2");
    }

    #[test]
    fn predecessors() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("foo");
            g.add_predecessor(start, "pred1");
            g.add_predecessor(start, "pred2");
            g
        };
        let preds = pred_string(&g, g.start());
        assert_eq!(preds, "pred1 pred2");
    }

    #[test]
    fn successors_and_predecessors() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("foo");
            g.add_predecessor(start, "pred1");
            g.add_successor(start, "succ1");
            g
        };
        let preds = pred_string(&g, g.start());
        let succs = succ_string(&g, g.start());
        assert_eq!(preds, "pred1");
        assert_eq!(succs, "succ1");
    }

    #[test]
    fn dot_string() {
        let g = {
            let mut g = VecGraph::new();
            let a = g.add_vertex("A");
            let _b = g.add_successor(a, "B");
            let c = g.add_successor(a, "C");
            let _d = g.add_successor(c, "D");
            g
        };
        assert_eq!(
            g.to_dot_string(),
            r#"digraph g {
   0 -> 1;
   0 -> 2;
   2 -> 3;
   0 [label="A"];
   1 [label="B"];
   2 [label="C"];
   3 [label="D"];
}"#
        );
    }

    #[test]
    fn preorder() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("A");
            let b = g.add_successor(start, "B");
            let c = g.add_successor(start, "C");
            let d = g.add_vertex("D");
            g.add_edge(b, d);
            g.add_edge(c, d);
            g
        };
        let mut out = Vec::new();
        super::pre_order(&g, &mut |node| {
            out.push(node.data(&g).to_string());
        });
        let pre_order_str = out.join(", ");
        debug_assert_eq!(pre_order_str, "A, B, D, C");
    }

    #[test]
    fn postorder() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("A");
            let b = g.add_successor(start, "B");
            let c = g.add_successor(start, "C");
            let d = g.add_vertex("D");
            g.add_edge(b, d);
            g.add_edge(c, d);
            g
        };
        let mut out = Vec::new();
        super::post_order(&g, &mut |node| {
            out.push(node.data(&g).to_string());
        });
        let postorder_str = out.join(", ");
        debug_assert_eq!(postorder_str, "D, B, C, A");
    }

    #[test]
    fn reverse_postorder() {
        let g = {
            let mut g = VecGraph::new();
            let start = g.add_vertex("A");
            let b = g.add_successor(start, "B");
            let c = g.add_successor(start, "C");
            let d = g.add_vertex("D");
            g.add_edge(b, d);
            g.add_edge(c, d);
            g
        };
        let mut out = Vec::new();
        super::reverse_post_order(&g, &mut |node| {
            out.push(node.data(&g).to_string());
        });
        let rpo_str = out.join(", ");
        debug_assert_eq!(rpo_str, "A, B, C, D");
    }
}
