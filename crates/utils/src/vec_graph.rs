use std::collections::HashSet;

#[derive(Debug)]
pub struct Edge<T>(Vertex<T>, Vertex<T>);

impl<T> Clone for Edge<T> {
    #[inline]
    fn clone(&self) -> Self {
        Edge(self.0.clone(), self.1.clone())
    }
}

impl<T> Copy for Edge<T> {}

impl<T> PartialEq for Edge<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T> Eq for Edge<T> {}

impl<T> std::hash::Hash for Edge<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct VecGraph<T> {
    pub(crate) vertices: Vec<T>,
    pub(crate) predecessors: Vec<Vec<Vertex<T>>>,
    pub(crate) successors: Vec<Vec<Vertex<T>>>,
    pub(crate) start: Option<Vertex<T>>,

    unlinked_vertices: Vec<Vertex<T>>,
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
            predecessors: Default::default(),
            successors: Default::default(),
            start: None,
            unlinked_vertices: Default::default(),
        }
    }

    #[inline]
    pub fn add_vertex(&mut self, data: T) -> Vertex<T> {
        let v = Vertex::new(self.vertices.len());
        if let None = &self.start {
            self.start = Some(v);
        }
        self.vertices.push(data);
        self.predecessors.push(Default::default());
        self.successors.push(Default::default());
        v
    }

    #[inline]
    pub fn add_successor(&mut self, v: Vertex<T>, data: T) -> Vertex<T> {
        let succ = self.add_vertex(data);
        self.add_edge(v, succ);
        succ
    }

    #[inline]
    pub fn add_predecessor(&mut self, v: Vertex<T>, data: T) -> Vertex<T> {
        let pred = self.add_vertex(data);
        self.add_edge(pred, v);
        pred
    }

    #[inline]
    pub fn add_edge(&mut self, from: Vertex<T>, to: Vertex<T>) -> bool {
        if self.predecessors[from.idx].contains(&to) {
            debug_assert!(self.successors[to.idx].contains(&from));
            return false;
        }
        self.successors[from.idx].push(to);
        self.predecessors[to.idx].push(from);
        true
    }

    #[inline]
    pub fn unlink(&mut self, vs: &[Vertex<T>]) {
        let mut successors_to_remove = HashSet::new();
        let mut predecessors_to_remove = HashSet::new();
        for v in vs {
            for succ in &self.successors[v.idx] {
                predecessors_to_remove.insert(*succ);
            }
            for pred in &self.predecessors[v.idx] {
                successors_to_remove.insert(*pred);
            }
        }
        for succ in successors_to_remove {
            self.successors[succ.idx].retain(|v| !vs.contains(v));
        }
        for pred in predecessors_to_remove {
            self.predecessors[pred.idx].retain(|v| !vs.contains(v));
        }
        for v in vs {
            self.successors[v.idx].clear();
            self.predecessors[v.idx].clear();
        }
        self.unlinked_vertices.extend(vs);
        if self.start.map_or(false, |v| vs.contains(&v)) {
            self.start = None;
        }
    }

    pub fn start(&self) -> Vertex<T> {
        self.start.unwrap()
    }

    pub fn set_start(&mut self, v: Vertex<T>) {
        self.start = Some(v);
    }

    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertices.len() - self.unlinked_vertices.len()
    }

    #[inline]
    pub fn vertices(&self) -> impl Iterator<Item = Vertex<T>> + '_ {
        (0..self.vertices.len()).map(|i| Vertex::new(i))
    }

    #[inline]
    pub fn edges(&self) -> impl Iterator<Item = Edge<T>> + '_ {
        self.successors.iter().enumerate().flat_map(|(idx, succs)| {
            succs.iter().map(move |s| Edge(Vertex::new(idx), *s))
        })
    }

    #[inline]
    pub fn successors(&self, v: &Vertex<T>) -> &[Vertex<T>] {
        self.successors[v.idx].as_slice()
    }

    #[inline]
    pub fn predecessors(&self, v: &Vertex<T>) -> &[Vertex<T>] {
        self.predecessors[v.idx].as_slice()
    }

    #[inline]
    pub fn out_degree(&self, v: &Vertex<T>) -> usize {
        self.successors[v.idx].len()
    }

    #[inline]
    pub fn in_degree(&self, v: &Vertex<T>) -> usize {
        self.predecessors[v.idx].len()
    }

    #[inline]
    pub fn is_unlinked(&self, v: &Vertex<T>) -> bool {
        self.unlinked_vertices.contains(v)
    }
}

impl<T: PartialEq> VecGraph<T> {
    pub fn find_first(&self, data: &T) -> Option<Vertex<T>> {
        self.vertices
            .iter()
            .enumerate()
            .find_map(|(idx, v)| (v == data).then(|| Vertex::new(idx)))
    }

    pub fn find_all<'this, 'a: 'this>(
        &'this self,
        data: &'a T,
    ) -> impl Iterator<Item = Vertex<T>> + 'this {
        self.vertices
            .iter()
            .enumerate()
            .filter_map(move |(idx, v)| (v == data).then(|| Vertex::new(idx)))
    }

    pub fn collect_all(&self, data: &T) -> Vec<Vertex<T>> {
        self.find_all(data).collect()
    }
}

impl<T: ToString> VecGraph<T> {
    pub fn to_dot_string(&self) -> String {
        let indent = " ".repeat(3);
        let edges = self
            .edges()
            .map(|Edge(start, end)| {
                format!("{indent}{} -> {};", start.idx, end.idx,)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let vertices = self
            .vertices
            .iter()
            .enumerate()
            .map(|(idx, data)| {
                format!("{indent}{idx} [label=\"{}\"];", data.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("digraph g {{\n{edges}\n{vertices}\n}}")
    }
}

pub fn write_to_dot_file<T: ToString>(
    g: &VecGraph<T>,
    path: &str,
) -> std::io::Result<()> {
    std::fs::write(path, g.to_dot_string())
}

#[derive(Debug)]
pub struct Vertex<T> {
    idx: usize,
    _data: std::marker::PhantomData<T>,
}

impl<T> std::hash::Hash for Vertex<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.idx.hash(state)
    }
}

impl<T> PartialEq for Vertex<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.idx == other.idx
    }
}

impl<T> Eq for Vertex<T> {}

impl<T> Clone for Vertex<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new(self.idx)
    }
}

impl<T> Copy for Vertex<T> {}

impl<'graph, T: 'graph> Vertex<T> {
    #[inline]
    fn new(idx: usize) -> Self {
        Vertex {
            idx,
            _data: std::marker::PhantomData,
        }
    }

    #[inline]
    pub fn out_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        graph.out_degree(self)
    }

    #[inline]
    pub fn in_degree(&self, graph: &'graph VecGraph<T>) -> usize {
        graph.in_degree(self)
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
    ) -> &'graph [Vertex<T>] {
        graph.successors(self)
    }

    #[inline]
    pub fn predecessors(
        &self,
        graph: &'graph VecGraph<T>,
    ) -> &'graph [Vertex<T>] {
        graph.predecessors(self)
    }
}

pub mod traversal {
    use super::*;
    use std::collections::HashSet;

    fn pre_post_order_impl<T, Pre, Post, const REVERSE: bool>(
        graph: &VecGraph<T>,
        vertex: Vertex<T>,
        visited: &mut HashSet<Vertex<T>>,
        pre: &mut Pre,
        post: &mut Post,
    ) where
        Pre: FnMut(Vertex<T>),
        Post: FnMut(Vertex<T>),
    {
        if !visited.insert(vertex) {
            return;
        }

        pre(vertex);

        let visit = |successor: &Vertex<_>| {
            pre_post_order_impl::<T, _, _, REVERSE>(
                graph, *successor, visited, pre, post,
            );
        };
        let successors = vertex.successors(graph);
        if REVERSE {
            successors.iter().rev().for_each(visit)
        } else {
            successors.iter().for_each(visit)
        }

        post(vertex);
    }

    fn skip<T>(_: Vertex<T>) {}

    #[inline]
    fn pre_post_order<T, Pre, Post, const REVERSE: bool>(
        graph: &VecGraph<T>,
        pre: &mut Pre,
        post: &mut Post,
    ) where
        Pre: FnMut(Vertex<T>),
        Post: FnMut(Vertex<T>),
    {
        let mut visited = HashSet::new();
        graph.start.map(|start| {
            pre_post_order_impl::<_, _, _, REVERSE>(
                graph,
                start,
                &mut visited,
                pre,
                post,
            );
        });
    }

    #[inline]
    pub fn pre_order<T, F>(graph: &VecGraph<T>, f: &mut F)
    where
        F: FnMut(Vertex<T>),
    {
        pre_post_order::<_, _, _, false>(graph, f, &mut skip)
    }

    #[inline]
    pub fn post_order<T, F>(graph: &VecGraph<T>, f: &mut F)
    where
        F: FnMut(Vertex<T>),
    {
        pre_post_order::<_, _, _, false>(graph, &mut skip, f)
    }

    #[inline]
    pub fn reverse_post_order<T, F>(graph: &VecGraph<T>, f: &mut F)
    where
        F: FnMut(Vertex<T>),
    {
        let mut vertices = Vec::with_capacity(graph.vertices.len());
        pre_post_order::<_, _, _, true>(graph, &mut skip, &mut |v| {
            vertices.push(v)
        });
        vertices.iter().rev().for_each(|v| f(*v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pred_string<T: ToString>(g: &VecGraph<T>, v: Vertex<T>) -> String {
        v.predecessors(&g)
            .iter()
            .map(|v| v.data(&g).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn succ_string<T: ToString>(g: &VecGraph<T>, v: Vertex<T>) -> String {
        v.successors(&g)
            .iter()
            .map(|v| v.data(&g).to_string())
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
    fn unlink_one_vertex() {
        let (g, a, b, c) = {
            let mut g = VecGraph::new();
            let a = g.add_vertex("a");
            let b = g.add_successor(a, "b");
            let c = g.add_successor(b, "c");
            g.unlink(&[b]);
            (g, a, b, c)
        };
        assert_eq!(g.num_vertices(), 2);
        assert_eq!(a.out_degree(&g), 0);
        assert_eq!(c.in_degree(&g), 0);
        for e in g.edges() {
            assert_ne!(e.0, b);
            assert_ne!(e.1, b);
        }
    }

    #[test]
    fn unlink_multiple_vertices() {
        let (g, a, b, c, d) = {
            let mut g = VecGraph::new();
            let a = g.add_vertex("a");

            let b = g.add_successor(a, "b");
            let c = g.add_successor(b, "c");

            let d = g.add_vertex("d");
            g.add_edge(a, d);
            g.add_edge(b, d);
            g.add_edge(c, d);

            g.unlink(&[b, c]);
            (g, a, b, c, d)
        };
        assert_eq!(g.num_vertices(), 2);
        assert_eq!(a.out_degree(&g), 1);
        assert_eq!(b.in_degree(&g), 0);
        assert_eq!(c.in_degree(&g), 0);
        assert_eq!(d.in_degree(&g), 1);
        for e in g.edges() {
            assert_ne!(e.0, b);
            assert_ne!(e.1, b);
            assert_ne!(e.0, c);
            assert_ne!(e.1, c);
        }
    }

    #[test]
    fn unlink_start() {
        let (g, a) = {
            let mut g = VecGraph::new();
            let a = g.add_vertex("a");
            let b = g.add_successor(a, "b");
            g.add_successor(b, "c");
            g.unlink(&[a]);
            (g, a)
        };
        assert_eq!(g.num_vertices(), 2);
        assert_eq!(g.start, None);
        for e in g.edges() {
            assert_ne!(e.0, a);
            assert_ne!(e.1, a);
        }
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
        super::traversal::pre_order(&g, &mut |v| {
            out.push(v.data(&g).to_string());
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
        super::traversal::post_order(&g, &mut |v| {
            out.push(v.data(&g).to_string());
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
        super::traversal::reverse_post_order(&g, &mut |v| {
            out.push(v.data(&g).to_string());
        });
        let rpo_str = out.join(", ");
        debug_assert_eq!(rpo_str, "A, B, C, D");
    }

    #[test]
    fn find_first() {
        let (g, b, b2, c) = {
            let mut g = VecGraph::new();
            let start = g.add_vertex('A');
            let b = g.add_successor(start, 'B');
            let b2 = g.add_successor(start, 'B');
            let c = g.add_successor(start, 'C');
            let d = g.add_vertex('D');
            g.add_edge(b, d);
            g.add_edge(c, d);
            (g, b, b2, c)
        };
        assert_eq!(Some(b), g.find_first(&'B'));
        assert_eq!(Some(c), g.find_first(&'C'));
        assert_eq!(&[b, b2], g.collect_all(&'B').as_slice());
    }
}
