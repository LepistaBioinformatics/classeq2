use super::newick::{format_number, RootedNode};
use crate::domain::dtos::telemetry_code::TelemetryCode;

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use phylotree::tree::Tree as PhyloTree;
use std::collections::{HashMap, HashSet};

/// An edge of the unrooted tree.
///
/// The support belongs to the edge: it describes the bipartition the edge
/// induces. Newick stores it on the node below the edge, so when the tree is
/// re-rooted it must move with the edge, not with the node.
#[derive(Clone, Debug)]
struct Edge {
    ends: [usize; 2],
    length: Option<f64>,
    support: Option<f64>,
    alive: bool,
}

impl Edge {
    fn other(&self, node: usize) -> usize {
        if self.ends[0] == node {
            self.ends[1]
        } else {
            self.ends[0]
        }
    }
}

#[derive(Clone, Debug)]
struct Node {
    /// Leaf name, or the non-numeric label of an internal node.
    name: Option<String>,
    leaf: bool,
    alive: bool,
    edges: Vec<usize>,
}

/// An unrooted view of a phylogenetic tree with no degree-2 internal nodes.
#[derive(Clone, Debug)]
pub(crate) struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

fn err(code: TelemetryCode, msg: impl ToString) -> MappedErrors {
    use_case_err(msg).with_code(code.to_string().as_str())
}

impl Graph {
    fn add_node(&mut self, name: Option<String>, leaf: bool) -> usize {
        self.nodes.push(Node {
            name,
            leaf,
            alive: true,
            edges: vec![],
        });
        self.nodes.len() - 1
    }

    fn add_edge(
        &mut self,
        a: usize,
        b: usize,
        length: Option<f64>,
        support: Option<f64>,
    ) -> usize {
        self.edges.push(Edge {
            ends: [a, b],
            length,
            support,
            alive: true,
        });
        let id = self.edges.len() - 1;
        self.nodes[a].edges.push(id);
        self.nodes[b].edges.push(id);
        id
    }

    fn remove_edge(&mut self, id: usize) {
        self.edges[id].alive = false;
        for end in self.edges[id].ends {
            self.nodes[end].edges.retain(|edge| *edge != id);
        }
    }

    fn degree(&self, node: usize) -> usize {
        self.nodes[node].edges.len()
    }

    /// Builds the unrooted view of a parsed tree.
    ///
    /// The numeric label of each internal node is moved onto the edge above
    /// it. The old root is dissolved: with one child it is dropped, with two
    /// children its edges are merged into one.
    pub fn from_phylo(tree: &PhyloTree) -> Result<Graph, MappedErrors> {
        let invalid = |msg: String| err(TelemetryCode::UCTREE0007, msg);

        let root = tree
            .get_root()
            .map_err(|e| invalid(format!("Could not get the tree root: {e}")))?;
        let order = tree
            .preorder(&root)
            .map_err(|e| invalid(format!("Could not traverse the tree: {e}")))?;

        let mut graph = Graph {
            nodes: Vec::with_capacity(order.len() + 1),
            edges: Vec::with_capacity(order.len() + 1),
        };
        let mut index = HashMap::<usize, usize>::new();

        for id in &order {
            let node = tree
                .get(id)
                .map_err(|e| invalid(format!("Could not read a tree node: {e}")))?;
            let leaf = node.is_tip();
            let numeric =
                node.name.as_deref().and_then(|n| n.trim().parse::<f64>().ok());

            let name = match (leaf, numeric) {
                (true, _) => node.name.clone(),
                (false, Some(_)) => None,
                (false, None) => node.name.clone().filter(|n| !n.is_empty()),
            };

            let new = graph.add_node(name, leaf);
            index.insert(*id, new);

            if let Some(parent) = node.parent {
                let support = if leaf { None } else { numeric };
                graph.add_edge(index[&parent], new, node.parent_edge, support);
            }
        }

        let old_root = index[&root];
        match graph.degree(old_root) {
            0 => {
                return Err(err(
                    TelemetryCode::UCTREE0005,
                    "The tree has a single node and cannot be rooted",
                ))
            }
            1 => {
                let edge = graph.nodes[old_root].edges[0];
                graph.remove_edge(edge);
                graph.nodes[old_root].alive = false;
            }
            2 => graph.dissolve(old_root),
            _ => (),
        }

        Ok(graph)
    }

    /// Replaces a degree-2 node and its two edges with a single edge.
    fn dissolve(&mut self, node: usize) {
        let (first, second) = (self.nodes[node].edges[0], self.nodes[node].edges[1]);
        let a = self.edges[first].other(node);
        let b = self.edges[second].other(node);

        let length = match (self.edges[first].length, self.edges[second].length) {
            (Some(x), Some(y)) => Some(x + y),
            (x, y) => x.or(y),
        };
        let support = self.edges[first].support.or(self.edges[second].support);

        self.remove_edge(first);
        self.remove_edge(second);
        self.nodes[node].alive = false;
        self.add_edge(a, b, length, support);
    }

    fn leaves(&self) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|n| self.nodes[*n].alive && self.nodes[*n].leaf)
            .collect()
    }

    fn name(&self, node: usize) -> &str {
        self.nodes[node].name.as_deref().unwrap_or("")
    }

    /// Leaf names must exist and be unique: they are the only stable
    /// identity of a leaf and the tie-breaker of the rooting methods.
    fn validate_leaf_names(&self, code: TelemetryCode) -> Result<(), MappedErrors> {
        let mut seen = HashSet::new();
        let mut duplicated = Vec::new();

        for leaf in self.leaves() {
            match &self.nodes[leaf].name {
                None => {
                    return Err(err(code, "Every leaf of the tree must have a name"))
                }
                Some(name) if name.is_empty() => {
                    return Err(err(code, "Every leaf of the tree must have a name"))
                }
                Some(name) => {
                    if !seen.insert(name.clone()) {
                        duplicated.push(name.clone());
                    }
                }
            }
        }

        if !duplicated.is_empty() {
            duplicated.sort();
            duplicated.dedup();
            return Err(err(
                code,
                format!("Leaf names must be unique; duplicated: {duplicated:?}"),
            ));
        }

        Ok(())
    }

    /// Distances from `source` to every node, and the edge each node was
    /// reached through.
    fn distances_from(&self, source: usize) -> (Vec<f64>, Vec<Option<usize>>) {
        let mut distance = vec![f64::NAN; self.nodes.len()];
        let mut via = vec![None; self.nodes.len()];
        let mut stack = vec![source];
        distance[source] = 0.0;

        while let Some(node) = stack.pop() {
            for &edge in &self.nodes[node].edges {
                let next = self.edges[edge].other(node);
                if distance[next].is_nan() {
                    distance[next] =
                        distance[node] + self.edges[edge].length.unwrap_or(0.0);
                    via[next] = Some(edge);
                    stack.push(next);
                }
            }
        }

        (distance, via)
    }

    /// The farthest leaf other than `source`; ties go to the smallest name.
    fn farthest_leaf(&self, distance: &[f64], leaves: &[usize], source: usize) -> usize {
        let leaves = leaves.iter().copied().filter(|leaf| *leaf != source).collect::<Vec<_>>();
        let max = leaves
            .iter()
            .map(|leaf| distance[*leaf])
            .fold(f64::MIN, f64::max);
        let tolerance = 1e-12 * max.abs().max(1e-300);

        *leaves
            .iter()
            .filter(|leaf| (distance[**leaf] - max).abs() <= tolerance)
            .min_by(|a, b| self.name(**a).cmp(self.name(**b)))
            .expect("at least one leaf")
    }

    /// Inserts a new node on `edge`, at `offset` from the end `from`, and
    /// returns it. Both halves keep the edge's support.
    fn split_edge(&mut self, edge: usize, from: usize, offset: Option<f64>) -> usize {
        let to = self.edges[edge].other(from);
        let support = self.edges[edge].support;
        let (near, far) = match (self.edges[edge].length, offset) {
            (Some(length), Some(offset)) => {
                let offset = offset.clamp(0.0, length);
                (Some(offset), Some(length - offset))
            }
            (Some(length), None) => (Some(length / 2.0), Some(length / 2.0)),
            (None, _) => (None, None),
        };

        self.remove_edge(edge);
        let root = self.add_node(None, false);
        self.add_edge(from, root, near, support);
        self.add_edge(root, to, far, support);
        root
    }

    /// Roots the tree at the midpoint of its longest leaf-to-leaf path.
    ///
    /// When the midpoint falls exactly on an internal node, the root is put
    /// on the next edge of the path at distance 0 from that node, so that
    /// the root stays binary.
    pub fn root_at_midpoint(&mut self) -> Result<usize, MappedErrors> {
        let code = TelemetryCode::UCTREE0005;
        let leaves = self.leaves();

        if leaves.len() < 2 {
            return Err(err(code, "Midpoint rooting needs at least two leaves"));
        }

        self.validate_leaf_names(code.clone())?;

        for edge in self.edges.iter().filter(|e| e.alive) {
            match edge.length {
                None => {
                    return Err(err(
                        code,
                        "Midpoint rooting needs branch lengths on every branch",
                    ))
                }
                Some(length) if !length.is_finite() || length < 0.0 => {
                    return Err(err(
                        code,
                        format!("Branch lengths must be finite and non-negative; found {length}"),
                    ))
                }
                _ => (),
            }
        }

        let start = *leaves
            .iter()
            .min_by(|a, b| self.name(**a).cmp(self.name(**b)))
            .expect("at least two leaves");
        let (from_start, _) = self.distances_from(start);
        let a = self.farthest_leaf(&from_start, &leaves, start);
        let (from_a, via) = self.distances_from(a);
        let b = self.farthest_leaf(&from_a, &leaves, a);

        let mut path = vec![b];
        while let Some(edge) = via[*path.last().unwrap()] {
            path.push(self.edges[edge].other(*path.last().unwrap()));
        }
        path.reverse();

        let half = from_a[b] / 2.0;
        let tolerance = 1e-12 * from_a[b].max(1e-300);

        for i in 0..path.len() - 1 {
            let (u, v) = (path[i], path[i + 1]);
            if from_a[v] < half - tolerance {
                continue;
            }

            let on_inner_node = (from_a[v] - half).abs() <= tolerance && i + 2 < path.len();
            if on_inner_node {
                let next = self.edge_between(v, path[i + 2]);
                return Ok(self.split_edge(next, v, Some(0.0)));
            }

            let edge = self.edge_between(u, v);
            return Ok(self.split_edge(edge, u, Some(half - from_a[u])));
        }

        unreachable!("the midpoint lies on the longest path")
    }

    fn edge_between(&self, a: usize, b: usize) -> usize {
        *self.nodes[a]
            .edges
            .iter()
            .find(|edge| self.edges[**edge].other(a) == b)
            .expect("adjacent nodes share an edge")
    }

    /// Roots the tree on the edge that separates the outgroup leaves from all
    /// the others, splitting that edge in half.
    pub fn root_at_outgroup(&mut self, outgroup: &[String]) -> Result<usize, MappedErrors> {
        let code = TelemetryCode::UCTREE0006;
        self.validate_leaf_names(code.clone())?;

        let leaves = self.leaves();
        let by_name: HashMap<&str, usize> =
            leaves.iter().map(|leaf| (self.name(*leaf), *leaf)).collect();

        let mut unknown = outgroup
            .iter()
            .filter(|name| !by_name.contains_key(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            unknown.sort();
            return Err(err(code, format!("Unknown outgroup leaves: {unknown:?}")));
        }

        let members: HashSet<usize> =
            outgroup.iter().map(|name| by_name[name.as_str()]).collect();
        if members.is_empty() {
            return Err(err(code, "The outgroup must name at least one leaf"));
        }
        if members.len() == leaves.len() {
            return Err(err(code, "The outgroup cannot contain every leaf of the tree"));
        }

        //
        // Hang the tree from an ingroup leaf. A clade below a node is then
        // exactly the outgroup iff it holds every outgroup leaf and nothing
        // else.
        //
        let anchor = *leaves
            .iter()
            .filter(|leaf| !members.contains(leaf))
            .min_by(|a, b| self.name(**a).cmp(self.name(**b)))
            .expect("at least one ingroup leaf");

        let mut order = Vec::with_capacity(self.nodes.len());
        let mut via = vec![None; self.nodes.len()];
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![anchor];
        seen[anchor] = true;
        while let Some(node) = stack.pop() {
            order.push(node);
            for &edge in &self.nodes[node].edges {
                let next = self.edges[edge].other(node);
                if !seen[next] {
                    seen[next] = true;
                    via[next] = Some(edge);
                    stack.push(next);
                }
            }
        }

        let mut below_all = vec![0usize; self.nodes.len()];
        let mut below_out = vec![0usize; self.nodes.len()];
        for &node in order.iter().rev() {
            if self.nodes[node].leaf {
                below_all[node] += 1;
                if members.contains(&node) {
                    below_out[node] += 1;
                }
            }
            if let Some(edge) = via[node] {
                let parent = self.edges[edge].other(node);
                below_all[parent] += below_all[node];
                below_out[parent] += below_out[node];
            }
        }

        let target = members.len();
        if let Some(&clade) = order
            .iter()
            .find(|n| **n != anchor && below_out[**n] == target && below_all[**n] == target)
        {
            let edge = via[clade].expect("not the anchor");
            let parent = self.edges[edge].other(clade);
            return Ok(self.split_edge(edge, parent, None));
        }

        //
        // Not monophyletic: report the ingroup leaves inside the smallest
        // clade that holds the whole outgroup.
        //
        let smallest = order
            .iter()
            .filter(|n| below_out[**n] == target)
            .min_by_key(|n| below_all[**n])
            .copied()
            .expect("the anchor holds every leaf");
        let mut intruders = Vec::new();
        let mut stack = vec![smallest];
        while let Some(node) = stack.pop() {
            if self.nodes[node].leaf && !members.contains(&node) {
                intruders.push(self.name(node).to_string());
            }
            for &edge in &self.nodes[node].edges {
                if Some(edge) != via[node] {
                    stack.push(self.edges[edge].other(node));
                }
            }
        }
        intruders.sort();

        Err(err(
            code,
            format!(
                "The outgroup is not monophyletic: the smallest clade containing it also contains {intruders:?}"
            ),
        ))
    }

    /// Orients the tree from `root`. Each edge's support is written on the
    /// node that is below the edge in the new orientation.
    pub fn orient(&self, root: usize) -> RootedNode {
        fn build(graph: &Graph, node: usize, via: Option<usize>) -> RootedNode {
            let edge = via.map(|e| &graph.edges[e]);
            let label = if graph.nodes[node].leaf {
                graph.nodes[node].name.clone()
            } else {
                edge.and_then(|e| e.support)
                    .map(format_number)
                    .or_else(|| graph.nodes[node].name.clone())
            };

            let children = graph.nodes[node]
                .edges
                .iter()
                .filter(|e| Some(**e) != via)
                .map(|e| build(graph, graph.edges[*e].other(node), Some(*e)))
                .collect();

            RootedNode {
                label,
                length: edge.and_then(|e| e.length),
                children,
            }
        }

        let mut rooted = build(self, root, None);
        rooted.label = None;
        rooted.length = None;
        rooted
    }
}
