mod annotation;
mod graph;
mod newick;

pub use annotation::{split_rooting_annotation, RootingFlag};

use crate::domain::dtos::telemetry_code::TelemetryCode;
use graph::Graph;
use newick::{format_number, write_newick, RootedNode};

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use phylotree::tree::Tree as PhyloTree;

/// How to root a tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootingMethod {
    /// Root at the midpoint of the longest leaf-to-leaf path.
    Midpoint,

    /// Root on the edge that separates these leaves from all the others.
    Outgroup(Vec<String>),
}

/// Parses Newick text that no longer carries a rooting annotation.
pub(crate) fn parse_newick(text: &str) -> Result<PhyloTree, MappedErrors> {
    PhyloTree::from_newick(text).or_else(|e| {
        use_case_err(format!("Could not parse tree: {e:?}"))
            .with_code(TelemetryCode::UCTREE0007.to_string().as_str())
            .as_error()
    })
}

fn rooted_from_phylo(tree: &PhyloTree, id: usize) -> RootedNode {
    let node = tree.get(&id).expect("node ids come from the tree");
    let label = match (node.is_tip(), &node.name) {
        (true, name) => name.clone(),
        (false, Some(name)) => match name.trim().parse::<f64>() {
            Ok(support) => Some(format_number(support)),
            Err(_) if name.is_empty() => None,
            Err(_) => Some(name.clone()),
        },
        (false, None) => None,
    };

    RootedNode {
        label,
        length: node.parent_edge,
        children: node
            .children
            .iter()
            .map(|child| rooted_from_phylo(tree, *child))
            .collect(),
    }
}

/// Roots a Newick tree and returns it as a single Newick line.
///
/// Leaf names, branch lengths and supports are preserved; supports follow
/// their edges when the tree is re-rooted. Children are written in a
/// deterministic order (by the smallest leaf name below each child).
///
/// With `keep_if_rooted`, a tree whose root is already binary, and that is
/// not marked `[&U]`, is written back without re-rooting.
pub fn root_newick(
    newick: &str,
    method: &RootingMethod,
    keep_if_rooted: bool,
) -> Result<String, MappedErrors> {
    let (flag, text) = split_rooting_annotation(newick);
    let tree = parse_newick(&text)?;

    let root = tree.get_root().or_else(|e| {
        use_case_err(format!("Could not get the tree root: {e}"))
            .with_code(TelemetryCode::UCTREE0007.to_string().as_str())
            .as_error()
    })?;
    let binary_root = tree
        .get(&root)
        .map(|node| node.children.len() == 2)
        .unwrap_or(false);

    let mut rooted = if keep_if_rooted && binary_root && flag != Some(RootingFlag::Unrooted) {
        let mut rooted = rooted_from_phylo(&tree, root);
        rooted.label = None;
        rooted.length = None;
        rooted
    } else {
        let mut graph = Graph::from_phylo(&tree)?;
        let new_root = match method {
            RootingMethod::Midpoint => graph.root_at_midpoint()?,
            RootingMethod::Outgroup(names) => graph.root_at_outgroup(names)?,
        };
        graph.orient(new_root)
    };

    rooted.sort();
    Ok(write_newick(&rooted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn midpoint(newick: &str) -> String {
        root_newick(newick, &RootingMethod::Midpoint, false).unwrap()
    }

    fn outgroup(newick: &str, names: &[&str]) -> Result<String, MappedErrors> {
        root_newick(
            newick,
            &RootingMethod::Outgroup(names.iter().map(|n| n.to_string()).collect()),
            false,
        )
    }

    // ? -----------------------------------------------------------------------
    // ? Midpoint
    // ? -----------------------------------------------------------------------

    #[test]
    fn midpoint_inside_an_edge_splits_it_at_the_exact_position() {
        // Diameter C..A = 5; the midpoint is 2.5 from C on the edge C-X (4).
        assert_eq!(midpoint("(A:1,B:1,C:4);"), "((A:1,B:1):1.5,C:2.5);");
    }

    #[test]
    fn midpoint_on_an_internal_node_gives_a_binary_root_with_a_zero_edge() {
        // Diameter D..B = 1.8; the midpoint (0.9 from D) is the node above
        // (C,D) and (E,F). ete3 splits the 0.7 edge 50/50 instead (F-2).
        assert_eq!(
            midpoint("(A:0.1,B:0.2,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0.7);"),
            "((A:0.1,B:0.2)80:0.7,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0);"
        );
    }

    #[test]
    fn midpoint_of_a_star_tree() {
        assert_eq!(midpoint("(A:1,B:2,C:3,D:4);"), "((A:1,B:2,C:3):0.5,D:3.5);");
    }

    #[test]
    fn midpoint_of_a_two_leaf_tree() {
        assert_eq!(midpoint("(A:1,B:3);"), "(A:2,B:2);");
    }

    #[test]
    fn midpoint_ties_are_broken_by_leaf_name() {
        let one = midpoint("(A:1,B:1,C:1,D:1);");
        let two = midpoint("(D:1,C:1,B:1,A:1);");
        assert_eq!(one, two);
        // Sweeps start at A; B is the farthest leaf by name, then A from B.
        // The midpoint is the central node, so the root goes on the edge to
        // A at distance 0 from that node.
        assert_eq!(one, "(A:1,(B:1,C:1,D:1):0);");
    }

    #[test]
    fn midpoint_with_zero_length_branches() {
        // Every point is a midpoint; the rule still gives one fixed answer.
        assert_eq!(midpoint("(A:0,B:0,(C:0,D:0):0);"), "(A:0,(B:0,(C:0,D:0):0):0);");
    }

    #[test]
    fn midpoint_re_roots_an_already_rooted_tree() {
        // The old root is dissolved: A..C = 7, so the midpoint is 3.5 from C.
        assert_eq!(midpoint("((A:1,B:1)90:1,C:5);"), "((A:1,B:1)90:2.5,C:3.5);");
    }

    fn assert_code(result: Result<String, MappedErrors>, code: &str) {
        let err = result.unwrap_err();
        assert!(err.has_str_code(code), "{err}");
    }

    #[test]
    fn midpoint_requires_branch_lengths() {
        assert_code(
            root_newick("(A,B,(C,D)90);", &RootingMethod::Midpoint, false),
            "UCTREE0005",
        );
    }

    #[test]
    fn midpoint_rejects_negative_lengths() {
        assert_code(
            root_newick("(A:1,B:-1,C:1);", &RootingMethod::Midpoint, false),
            "UCTREE0005",
        );
    }

    #[test]
    fn duplicated_leaf_names_are_rejected() {
        assert_code(
            root_newick("(A:1,A:1,C:1);", &RootingMethod::Midpoint, false),
            "UCTREE0005",
        );
    }

    #[test]
    fn malformed_newick_is_rejected() {
        assert_code(
            root_newick("((A,B", &RootingMethod::Midpoint, false),
            "UCTREE0007",
        );
    }

    // ? -----------------------------------------------------------------------
    // ? Outgroup
    // ? -----------------------------------------------------------------------

    #[test]
    fn outgroup_clade_is_split_from_the_rest() {
        assert_eq!(
            outgroup("(A:1,B:1,(C:1,D:1)90:2);", &["C", "D"]).unwrap(),
            "((A:1,B:1)90:1,(C:1,D:1)90:1);"
        );
    }

    #[test]
    fn outgroup_of_a_single_leaf() {
        assert_eq!(
            outgroup("(A:1,B:1,(C:1,D:1)90:2);", &["A"]).unwrap(),
            "(A:0.5,(B:1,(C:1,D:1)90:2):0.5);"
        );
    }

    #[test]
    fn non_monophyletic_outgroup_lists_the_intruders() {
        let err = outgroup("(A:1,B:1,(C:1,D:1)90:2);", &["A", "C"]).unwrap_err();
        assert!(err.has_str_code("UCTREE0006"), "{err}");
        assert!(err.msg().contains("[\"D\"]"), "{err}");
    }

    #[test]
    fn unknown_outgroup_leaves_are_rejected() {
        let err = outgroup("(A:1,B:1,C:1);", &["Z"]).unwrap_err();
        assert!(err.has_str_code("UCTREE0006"), "{err}");
        assert!(err.msg().contains("Z"), "{err}");
    }

    #[test]
    fn outgroup_cannot_be_every_leaf() {
        assert_code(outgroup("(A:1,B:1,C:1);", &["A", "B", "C"]), "UCTREE0006");
    }

    #[test]
    fn outgroup_works_without_branch_lengths() {
        assert_eq!(
            outgroup("(A,B,(C,D)90);", &["C", "D"]).unwrap(),
            "((A,B)90,(C,D)90);"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? Annotations and keep-if-rooted
    // ? -----------------------------------------------------------------------

    #[test]
    fn keep_if_rooted_writes_a_rooted_tree_back_unchanged() {
        assert_eq!(
            root_newick("[&R]((B:1,A:1)90:1,C:2);", &RootingMethod::Midpoint, true)
                .unwrap(),
            "((A:1,B:1)90:1,C:2);"
        );
    }

    #[test]
    fn keep_if_rooted_still_roots_a_tree_marked_unrooted() {
        assert_eq!(
            root_newick("[&U]((A:1,B:1)90:1,C:5);", &RootingMethod::Midpoint, true)
                .unwrap(),
            "((A:1,B:1)90:2.5,C:3.5);"
        );
    }

    // ? -----------------------------------------------------------------------
    // ? Property: supports follow their bipartitions
    // ? -----------------------------------------------------------------------

    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// A random unrooted tree (trifurcating root) with lengths and, on most
    /// internal nodes, a support.
    fn random_unrooted(leaves: usize, seed: u64) -> String {
        let mut rng = Lcg(seed);
        let mut nodes = (0..leaves)
            .map(|i| format!("L{i:02}:{}", 1 + rng.below(200)))
            .collect::<Vec<_>>();

        while nodes.len() > 3 {
            let a = nodes.remove(rng.below(nodes.len() as u64) as usize);
            let b = nodes.remove(rng.below(nodes.len() as u64) as usize);
            let support = match rng.below(5) {
                0 => String::new(),
                _ => rng.below(101).to_string(),
            };
            nodes.push(format!("({a},{b}){support}:{}", 1 + rng.below(200)));
        }

        format!("({});", nodes.join(","))
    }

    /// Non-trivial bipartitions with their supports, each written as the
    /// side that does not contain the smallest leaf name.
    fn bipartitions(newick: &str) -> BTreeSet<(Vec<String>, Option<String>)> {
        let tree = PhyloTree::from_newick(newick).unwrap();
        let root = tree.get_root().unwrap();
        let all = tree
            .get_leaf_names()
            .into_iter()
            .map(Option::unwrap)
            .collect::<BTreeSet<_>>();
        let smallest = all.iter().next().unwrap().clone();

        tree.preorder(&root)
            .unwrap()
            .into_iter()
            .filter(|id| *id != root && !tree.get(id).unwrap().is_tip())
            .filter_map(|id| {
                let below = tree
                    .get_subtree_leaves(&id)
                    .unwrap()
                    .into_iter()
                    .map(|leaf| tree.get(&leaf).unwrap().name.clone().unwrap())
                    .collect::<BTreeSet<_>>();
                if below.len() < 2 || below.len() > all.len() - 2 {
                    return None;
                }
                let side = if below.contains(&smallest) {
                    all.difference(&below).cloned().collect()
                } else {
                    below.into_iter().collect()
                };
                let support = tree
                    .get(&id)
                    .unwrap()
                    .name
                    .clone()
                    .filter(|n| !n.is_empty())
                    .map(|n| format_number(n.parse::<f64>().unwrap()));
                Some((side, support))
            })
            .collect()
    }

    #[test]
    fn re_rooting_keeps_every_bipartition_with_its_support() {
        for leaves in [3, 4, 5, 8, 13, 21, 34, 50] {
            for seed in 0..5 {
                let input = random_unrooted(leaves, seed * 31 + leaves as u64);
                let expected = bipartitions(&input);

                let by_midpoint = midpoint(&input);
                assert_eq!(bipartitions(&by_midpoint), expected, "{input}");

                let by_outgroup = outgroup(&input, &["L00"]).unwrap();
                assert_eq!(bipartitions(&by_outgroup), expected, "{input}");

                // Writing is deterministic and stable under re-rooting.
                assert_eq!(midpoint(&by_outgroup), by_midpoint, "{input}");
            }
        }
    }
}
