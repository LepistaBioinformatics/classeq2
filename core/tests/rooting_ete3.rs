//! 004-tree-rooting T9: `cls root-tree` (midpoint) against the outputs of
//! the former `scripts/root_tree.py` (ete3 3.1.3), kept as fixtures in
//! `tests/data/rooting/`.
//!
//! The rooted topologies are compared as sets of clades (the leaf set below
//! every non-root node). Branch lengths are not compared: ete3 splits the
//! root edge 50/50 instead of at the length midpoint (finding F-2), and it
//! drops supports (finding F-1).

use classeq_core::domain::rooting::{root_newick, RootingMethod};
use phylotree::tree::Tree as PhyloTree;
use std::{collections::BTreeSet, path::PathBuf};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/data/rooting")
            .join(name),
    )
    .unwrap()
}

fn rooted_clades(newick: &str) -> BTreeSet<BTreeSet<String>> {
    let tree = PhyloTree::from_newick(newick.trim()).unwrap();
    let root = tree.get_root().unwrap();
    tree.preorder(&root)
        .unwrap()
        .into_iter()
        .filter(|id| *id != root)
        .map(|id| {
            tree.get_subtree_leaves(&id)
                .unwrap()
                .into_iter()
                .map(|leaf| tree.get(&leaf).unwrap().name.clone().unwrap())
                .collect()
        })
        .collect()
}

fn compare(name: &str) -> (BTreeSet<BTreeSet<String>>, BTreeSet<BTreeSet<String>>) {
    let ours = root_newick(
        &fixture(&format!("{name}.input.nwk")),
        &RootingMethod::Midpoint,
        false,
    )
    .unwrap();
    (rooted_clades(&ours), rooted_clades(&fixture(&format!("{name}.ete3.nwk"))))
}

const FIXTURES: [&str; 5] = [
    "synthetic-8",
    "synthetic-20",
    "synthetic-50",
    "gapdh-phyml",
    "f2-midpoint-on-node",
];

#[test]
fn midpoint_rooting_gives_the_same_rooted_topology_as_ete3() {
    for name in FIXTURES {
        let (ours, ete3) = compare(name);
        assert_eq!(ours, ete3, "{name}");
    }
}

/// F-2: on the same rooted topology, the root edges differ. ete3 split the
/// 0.7 edge 50/50; the true midpoint lies on the node above (C,D) and (E,F).
#[test]
fn root_edge_lengths_follow_the_true_midpoint_not_ete3() {
    let ours = root_newick(
        &fixture("f2-midpoint-on-node.input.nwk"),
        &RootingMethod::Midpoint,
        false,
    )
    .unwrap();

    assert!(fixture("f2-midpoint-on-node.ete3.nwk").contains(":0.35"));
    assert_eq!(
        ours,
        "((A:0.1,B:0.2)80:0.7,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0);"
    );
}

/// F-1: the ete3 script dropped every support; the native command keeps
/// them, so `--min-branch-support` works on trees rooted with it.
#[test]
fn supports_are_kept_unlike_the_ete3_script() {
    let supports = |newick: &str| {
        let tree = PhyloTree::from_newick(newick.trim()).unwrap();
        let root = tree.get_root().unwrap();
        tree.preorder(&root)
            .unwrap()
            .into_iter()
            .filter(|id| !tree.get(id).unwrap().is_tip())
            .filter_map(|id| tree.get(&id).unwrap().name.clone())
            .filter(|name| name.parse::<f64>().is_ok())
            .count()
    };

    let input = fixture("gapdh-phyml.input.nwk");
    let ours =
        root_newick(&input, &RootingMethod::Midpoint, false).unwrap();

    assert_eq!(supports(&fixture("gapdh-phyml.ete3.nwk")), 0);
    assert!(supports(&input) > 100);
    // One more than the input: the split root edge carries its support on
    // both new root edges.
    assert!(supports(&ours) >= supports(&input), "{ours}");
}
