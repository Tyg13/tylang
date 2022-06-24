#[derive(Debug, Clone)]
pub struct VecGraph<T> {
    vertices: Vec<T>,
    edges: Vec<(NodeRef<T>, NodeRef<T>)>,
    start: Option<NodeRef<T>>,
}
impl<T> std::default::Default for VecGraph<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> VecGraph<T> {
    pub fn new() -> Self {
        Self {
            vertices: Default::default(),
            edges: Default::default(),
            start: None,
        }
    }

    pub fn add_vertex(&mut self, vertex: T) -> NodeRef<T> {
        let node = NodeRef::new(self.vertices.len());
        if self.start.is_none() {
            self.set_start(node);
        }
        self.vertices.push(vertex);
        node
    }

    pub fn add_successor(&mut self, node: NodeRef<T>, vertex: T) -> NodeRef<T> {
        let succ = self.add_vertex(vertex);
        self.add_edge(node, succ);
        succ
    }

    pub fn add_predecessor(&mut self, node: NodeRef<T>, vertex: T) -> NodeRef<T> {
        let pred = self.add_vertex(vertex);
        self.add_edge(pred, node);
        pred
    }

    pub fn add_edge(&mut self, from: NodeRef<T>, to: NodeRef<T>) {
        let edge = (from, to);
        if self.edges.contains(&edge) {
            return;
        }
        self.edges.push(edge);
    }

    pub fn start(&self) -> Option<NodeRef<T>> {
        self.start
    }

    pub fn set_start(&mut self, node: NodeRef<T>) {
        self.start = Some(node);
    }

    pub fn last(&self) -> Option<NodeRef<T>> {
        self.vertices().last()
    }

    pub fn vertices(&self) -> impl Iterator<Item = NodeRef<T>> + '_ {
        (0..self.vertices.len()).map(|i| NodeRef::new(i))
    }

    pub fn edges(&self) -> impl Iterator<Item = &(NodeRef<T>, NodeRef<T>)> + '_ {
        self.edges.iter()
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
            .map(|(start, end)| format!("{indent}{} -> {};", start.idx, end.idx,))
            .collect::<Vec<_>>()
            .join("\n");
        let nodes = self
            .vertices
            .iter()
            .enumerate()
            .map(|(idx, data)| format!("{indent}{idx} [label=\"{}\"];", data.to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        format!("digraph g {{\n{edges}\n{nodes}\n}}")
    }
}

pub fn write_to_dot_file<T: ToString>(g: &VecGraph<T>, path: &str) -> std::io::Result<()> {
    std::fs::write(path, g.to_dot_string())
}

#[derive(Debug)]
pub struct NodeRef<T> {
    idx: usize,
    _data: std::marker::PhantomData<T>,
}

impl<T> std::hash::Hash for NodeRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.idx.hash(state)
    }
}

impl<T> PartialEq for NodeRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<T> Eq for NodeRef<T> {}

impl<T> Clone for NodeRef<T> {
    fn clone(&self) -> Self {
        Self::new(self.idx)
    }
}

impl<T> Copy for NodeRef<T> {}

impl<'graph, T: 'graph> NodeRef<T> {
    fn new(idx: usize) -> Self {
        NodeRef {
            idx,
            _data: std::marker::PhantomData,
        }
    }

    pub fn in_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        self.predecessors(graph).count()
    }

    pub fn out_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        self.successors(graph).count()
    }

    pub fn data(&self, graph: &'graph VecGraph<T>) -> &'graph T {
        graph.vertices.get(self.idx).unwrap()
    }

    pub fn successors<'this: 'graph>(
        &'this self,
        graph: &'graph VecGraph<T>,
    ) -> impl Iterator<Item = NodeRef<T>> + 'graph {
        graph
            .edges
            .iter()
            .filter_map(|(start, end)| (*start == *self).then(|| *end))
    }

    pub fn predecessors<'this: 'graph>(
        &'this self,
        graph: &'graph VecGraph<T>,
    ) -> impl Iterator<Item = NodeRef<T>> + 'graph {
        graph
            .edges
            .iter()
            .filter_map(|(start, end)| (*end == *self).then(|| *start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pred_string<T: ToString>(g: &VecGraph<T>, n: NodeRef<T>) -> String {
        n.predecessors(&g)
            .map(|node| node.data(&g).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn succ_string<T: ToString>(g: &VecGraph<T>, n: NodeRef<T>) -> String {
        n.successors(&g)
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
        assert_eq!("foo", g.start().unwrap().data(&g).to_string());
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
        let succs = succ_string(&g, g.start().unwrap());
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
        let preds = pred_string(&g, g.start().unwrap());
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
        let preds = pred_string(&g, g.start().unwrap());
        let succs = succ_string(&g, g.start().unwrap());
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
}
