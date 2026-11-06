mod common;

use classeq_core::{
    domain::{
        dtos::{
            clade::{Clade, NodeType},
            kmers_map::KmersMap,
            tree::Tree,
        },
        rooting::RootingMethod,
    },
    use_cases::map_kmers_to_tree,
};
use mycelium_base::utils::errors::MappedErrors;
use common::{synth, write_inputs};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    path::PathBuf,
};
use tempfile::TempDir;
use uuid::Uuid;

const DECIMAL: &str = "(((A:0.1,B:0.2)0.7:0.5,(E:0.1,F:0.2)0.95:0.3)0.98:0.1,(C:0.3,D:0.4)0.99:0.5);";
const INTEGER: &str =
    "(((A:0.1,B:0.2)70:0.5,(E:0.1,F:0.2)95:0.3)98:0.1,(C:0.3,D:0.4)99:0.5);";

fn write(dir: &TempDir, file_name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(file_name);
    std::fs::write(&path, content).unwrap();
    path
}

fn load(newick: &str, min_branch_support: f64) -> Tree {
    let dir = TempDir::new().unwrap();
    Tree::init_from_file(&write(&dir, "tree.nwk", newick), min_branch_support)
        .unwrap()
}

fn all_clades(clade: &Clade) -> Vec<&Clade> {
    let mut out = vec![clade];
    for child in clade.children.iter().flatten() {
        out.extend(all_clades(child));
    }
    out
}

fn label(clade: &Clade) -> String {
    clade
        .name
        .clone()
        .unwrap_or_else(|| format!("#{}", clade.id))
}

/// Compact topology string: leaves by name, internal nodes as `#id`.
fn shape(clade: &Clade) -> String {
    match &clade.children {
        None => label(clade),
        Some(children) => format!(
            "{}({})",
            label(clade),
            children.iter().map(shape).collect::<Vec<_>>().join(",")
        ),
    }
}

fn assert_parent_ids_consistent(root: &Clade) {
    assert_eq!(root.parent, None);
    for clade in all_clades(root) {
        for child in clade.children.iter().flatten() {
            assert_eq!(child.parent, Some(clade.id), "child {}", child.id);
        }
    }
}

// ? TREE-01 -------------------------------------------------------------------

#[test]
fn init_from_file_parses_ids_kinds_names_supports_lengths_and_parents() {
    let tree = load(DECIMAL, 0.0);

    type Row = (
        NodeType,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<u64>,
    );
    let leaf = |name: &str| Some(name.to_string());
    let expected: BTreeMap<u64, Row> = BTreeMap::from([
        (0, (NodeType::Root, None, None, Some(0.0), None)),
        (1, (NodeType::Node, None, Some(0.98), Some(0.1), Some(0))),
        (2, (NodeType::Node, None, Some(0.7), Some(0.5), Some(1))),
        (3, (NodeType::Leaf, leaf("A"), None, Some(0.1), Some(2))),
        (4, (NodeType::Leaf, leaf("B"), None, Some(0.2), Some(2))),
        (5, (NodeType::Node, None, Some(0.95), Some(0.3), Some(1))),
        (6, (NodeType::Leaf, leaf("E"), None, Some(0.1), Some(5))),
        (7, (NodeType::Leaf, leaf("F"), None, Some(0.2), Some(5))),
        (8, (NodeType::Node, None, Some(0.99), Some(0.5), Some(0))),
        (9, (NodeType::Leaf, leaf("C"), None, Some(0.3), Some(8))),
        (10, (NodeType::Leaf, leaf("D"), None, Some(0.4), Some(8))),
    ]);

    let actual: BTreeMap<u64, Row> = all_clades(&tree.root)
        .into_iter()
        .map(|c| {
            (
                c.id,
                (
                    c.kind.clone(),
                    c.name.clone(),
                    c.support,
                    c.length,
                    c.parent,
                ),
            )
        })
        .collect();

    assert_eq!(actual, expected);
    assert_eq!(shape(&tree.root), "#0(#1(#2(A,B),#5(E,F)),#8(C,D))");
}

#[test]
fn init_from_file_sets_tree_metadata_from_file_name() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "my-tree.nwk", DECIMAL);
    let tree = Tree::init_from_file(&path, 0.5).unwrap();

    assert_eq!(tree.name, "my-tree.nwk");
    assert_eq!(tree.id, Uuid::new_v3(&Uuid::NAMESPACE_DNS, b"my-tree.nwk"));
    assert_eq!(tree.id.get_version_num(), 3);
    assert_eq!(tree.min_branch_support, 0.5);
    assert!(tree.annotations.is_none());
    assert!(tree.kmers_map.is_none());
}

#[test]
fn tree_id_depends_on_file_name_only() {
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();

    let first =
        Tree::init_from_file(&write(&a, "same.nwk", DECIMAL), 0.0).unwrap();
    let second =
        Tree::init_from_file(&write(&b, "same.nwk", INTEGER), 0.0).unwrap();
    let other =
        Tree::init_from_file(&write(&a, "other.nwk", DECIMAL), 0.0).unwrap();

    assert_eq!(first.id, second.id);
    assert_ne!(first.id, other.id);
}

#[test]
fn init_from_file_assigns_unique_ids_with_root_zero() {
    for newick in [
        DECIMAL,
        INTEGER,
        &synth::write_newick(&synth::default_topology()),
    ] {
        let tree = load(newick, 0.0);
        let ids: Vec<u64> =
            all_clades(&tree.root).iter().map(|c| c.id).collect();
        let unique: HashSet<u64> = ids.iter().copied().collect();

        assert_eq!(ids.len(), unique.len(), "{newick}");
        assert_eq!(tree.root.id, 0);
        assert!(tree.root.is_root());
        assert_parent_ids_consistent(&tree.root);
    }
}

#[test]
fn init_from_file_parses_integer_supports() {
    let tree = load(INTEGER, 0.0);
    let supports: BTreeMap<u64, Option<f64>> = all_clades(&tree.root)
        .iter()
        .filter(|c| c.is_internal())
        .map(|c| (c.id, c.support))
        .collect();

    assert_eq!(
        supports,
        BTreeMap::from([
            (1, Some(98.0)),
            (2, Some(70.0)),
            (5, Some(95.0)),
            (8, Some(99.0))
        ])
    );
}

#[test]
fn init_from_file_ignores_root_label_and_length() {
    let tree = load("((A:0.1,B:0.1)90:0.2,(C:0.1,D:0.1)90:0.2)77:3.5;", 0.0);
    assert_eq!(tree.root.support, None);
    assert_eq!(tree.root.length, Some(0.0));
    assert_eq!(tree.root.name, None);
}

#[test]
fn init_from_file_accepts_all_supported_extensions() {
    let dir = TempDir::new().unwrap();
    for ext in ["nwk", "newick", "tree"] {
        let path = write(&dir, &format!("t.{ext}"), DECIMAL);
        let tree = Tree::init_from_file(&path, 0.0).unwrap();
        assert_eq!(all_clades(&tree.root).len(), 11, "{ext}");
    }
}

#[test]
fn init_from_file_keeps_nodes_without_numeric_support() {
    let tree = load("((A,B)x,(C,D));", 50.0);

    assert_eq!(shape(&tree.root), "#0(#1(A,B),#4(C,D))");
    assert!(all_clades(&tree.root).iter().all(|c| c.support.is_none()));
    assert!(all_clades(&tree.root)
        .iter()
        .filter(|c| c.is_leaf())
        .all(|c| c.length.is_none()));
}

// ? TREE-02 -------------------------------------------------------------------

#[test]
fn sanitize_collapses_low_support_nodes_into_grandparent() {
    let cases = [
        (DECIMAL, 0.0, "#0(#1(#2(A,B),#5(E,F)),#8(C,D))"),
        (DECIMAL, 0.8, "#0(#1(A,B,#5(E,F)),#8(C,D))"),
        // equal to the threshold is kept
        (DECIMAL, 0.95, "#0(#1(A,B,#5(E,F)),#8(C,D))"),
        (DECIMAL, 0.99, "#0(A,B,E,F,#8(C,D))"),
        (DECIMAL, 1.0, "#0(A,B,E,F,C,D)"),
        (INTEGER, 80.0, "#0(#1(A,B,#5(E,F)),#8(C,D))"),
        (INTEGER, 0.8, "#0(#1(#2(A,B),#5(E,F)),#8(C,D))"),
        (INTEGER, 100.0, "#0(A,B,E,F,C,D)"),
    ];

    for (newick, threshold, expected) in cases {
        let tree = load(newick, threshold);
        assert_eq!(shape(&tree.root), expected, "threshold {threshold}");
        assert_parent_ids_consistent(&tree.root);
        assert_eq!(tree.min_branch_support, threshold);
    }
}

#[test]
fn sanitize_preserves_leaf_lengths_and_names() {
    let tree = load(DECIMAL, 1.0);
    let leaves: Vec<(String, Option<f64>)> = tree
        .root
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| (c.name.clone().unwrap(), c.length))
        .collect();

    assert_eq!(
        leaves,
        [
            ("A".to_string(), Some(0.1)),
            ("B".to_string(), Some(0.2)),
            ("E".to_string(), Some(0.1)),
            ("F".to_string(), Some(0.2)),
            ("C".to_string(), Some(0.3)),
            ("D".to_string(), Some(0.4)),
        ]
    );
}

// ? TREE-03 -------------------------------------------------------------------

fn load_err(file_name: &str, newick: &str) -> MappedErrors {
    let dir = TempDir::new().unwrap();
    Tree::init_from_file(&write(&dir, file_name, newick), 0.0).unwrap_err()
}

fn assert_code(err: &MappedErrors, code: &str, text: &str) {
    assert!(err.has_str_code(code), "{err}");
    assert!(err.msg().contains(text), "{err}");
}

#[test]
fn init_from_file_rejects_unsupported_extension() {
    let err = load_err("tree.txt", DECIMAL);
    assert_code(&err, "UCTREE0007", "Tree file format is not supported");
}

#[test]
fn init_from_file_rejects_path_without_extension() {
    let err = load_err("tree", DECIMAL);
    assert_code(&err, "UCTREE0007", "Tree file format is not supported");
}

/// 004-tree-rooting CHECK-01: an unrooted tree is an error with a hint, not
/// a panic.
#[test]
fn init_from_file_rejects_unrooted_tree_with_a_hint() {
    let err = load_err("tree.nwk", "(A:0.1,B:0.1,C:0.1);");
    assert_code(&err, "UCTREE0002", "not rooted (the root has 3 children)");
    assert!(err.msg().contains("cls root-tree"), "{err}");
}

#[test]
fn init_from_file_rejects_tree_marked_unrooted_even_if_binary() {
    let err = load_err("tree.nwk", "[&U]((A:0.1,B:0.1):0.1,C:0.1);");
    assert_code(&err, "UCTREE0001", "[&U]");
}

#[test]
fn init_from_file_accepts_tree_marked_rooted() {
    let tree = load("[&R]((A:0.1,B:0.1)90:0.1,C:0.1);", 0.0);
    assert_eq!(tree.root.children.as_ref().unwrap().len(), 2);
    let leaves: BTreeSet<String> = all_clades(&tree.root)
        .into_iter()
        .filter_map(|c| c.name.clone())
        .collect();
    assert_eq!(leaves, BTreeSet::from(["A", "B", "C"].map(String::from)));
}

#[test]
fn init_from_file_rejects_root_with_a_single_child() {
    let err = load_err("tree.nwk", "((A:0.1,B:0.1)90:0.1);");
    assert_code(&err, "UCTREE0003", "single child");
}

#[test]
fn init_from_file_reports_missing_file() {
    let dir = TempDir::new().unwrap();
    let err =
        Tree::init_from_file(&dir.path().join("missing.nwk"), 0.0).unwrap_err();
    assert_code(&err, "UCTREE0007", "Could not read file");
}

#[test]
fn init_from_file_reports_malformed_newick() {
    let err = load_err("tree.nwk", "((A,B)");
    assert_code(&err, "UCTREE0007", "Could not parse tree");
}

/// 004-tree-rooting CHECK-05: `build-db --root` roots an unrooted tree in
/// memory, keeping supports so that `min_branch_support` still applies.
#[test]
fn init_from_file_with_rooting_roots_an_unrooted_tree() {
    let dir = TempDir::new().unwrap();
    let path = write(
        &dir,
        "tree.nwk",
        "(A:0.1,B:0.2,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)40:0.6)80:0.7);",
    );

    let tree = Tree::init_from_file_with_rooting(
        &path,
        50.0,
        Some(&RootingMethod::Midpoint),
    )
    .unwrap();

    assert_eq!(tree.root.children.as_ref().unwrap().len(), 2);
    let supports: Vec<f64> = all_clades(&tree.root)
        .into_iter()
        .filter_map(|c| c.support)
        .collect();
    assert!(supports.contains(&90.0), "{supports:?}");
    assert!(supports.contains(&80.0), "{supports:?}");
    // The (E,F) node has support 40 < 50, so it is collapsed.
    assert!(!supports.contains(&40.0), "{supports:?}");
}

#[test]
fn init_from_file_with_rooting_keeps_an_already_rooted_tree() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "tree.nwk", INTEGER);

    let rooted = Tree::init_from_file_with_rooting(
        &path,
        0.0,
        Some(&RootingMethod::Midpoint),
    )
    .unwrap();
    let plain = Tree::init_from_file(&path, 0.0).unwrap();

    assert_eq!(rooted.root, plain.root);
}

#[test]
fn init_from_file_with_outgroup_rooting() {
    let dir = TempDir::new().unwrap();
    let path = write(&dir, "tree.nwk", "(A:1,B:1,(C:1,D:1)90:2);");

    let tree = Tree::init_from_file_with_rooting(
        &path,
        0.0,
        Some(&RootingMethod::Outgroup(vec!["C".into(), "D".into()])),
    )
    .unwrap();

    let root_children = tree.root.children.as_ref().unwrap();
    assert_eq!(root_children.len(), 2);
    assert!(root_children.iter().all(|c| c.support == Some(90.0)));
}

/// 004-tree-rooting CHECK-03 (D5): a root that becomes a polytomy after
/// collapsing low-support branches is accepted (with a warning).
#[test]
fn root_polytomy_after_collapsing_is_accepted() {
    let tree = load("((A:0.1,B:0.1)30:0.1,(C:0.1,D:0.1)90:0.1);", 50.0);
    assert_eq!(tree.root.children.as_ref().unwrap().len(), 3);
}

// ? TREE-04 -------------------------------------------------------------------

#[test]
fn clade_get_node_by_id_finds_any_node() {
    let tree = load(DECIMAL, 0.0);

    assert_eq!(tree.root.get_node_by_id(0).unwrap().id, 0);
    assert_eq!(tree.root.get_node_by_id(5).unwrap().support, Some(0.95));
    assert_eq!(
        tree.root.get_node_by_id(10).unwrap().name.as_deref(),
        Some("D")
    );
    assert!(tree.root.get_node_by_id(99).is_none());

    let node_2 = tree.root.get_node_by_id(2).unwrap();
    assert_eq!(node_2.get_node_by_id(4).unwrap().name.as_deref(), Some("B"));
    assert!(node_2.get_node_by_id(9).is_none());
}

#[test]
fn clade_get_path_to_root_includes_self_and_all_ancestors() {
    let tree = load(DECIMAL, 0.0);
    let node = |id| tree.root.get_node_by_id(id).unwrap();

    assert_eq!(
        node(3).get_path_to_root(&tree.root),
        HashSet::from([3, 2, 1, 0])
    );
    assert_eq!(
        node(9).get_path_to_root(&tree.root),
        HashSet::from([9, 8, 0])
    );
    assert_eq!(
        node(5).get_path_to_root(&tree.root),
        HashSet::from([5, 1, 0])
    );
    assert_eq!(tree.root.get_path_to_root(&tree.root), HashSet::from([0]));
}

#[test]
fn clade_get_leaves_with_paths_lists_leaves_in_order_with_root_first_paths() {
    let tree = load(DECIMAL, 0.0);

    let leaves: Vec<(String, Vec<u64>)> = tree
        .root
        .get_leaves_with_paths(None)
        .into_iter()
        .map(|(c, path)| (c.name.unwrap(), path))
        .collect();

    let expected: Vec<(String, Vec<u64>)> = vec![
        ("A".into(), vec![0, 1, 2, 3]),
        ("B".into(), vec![0, 1, 2, 4]),
        ("E".into(), vec![0, 1, 5, 6]),
        ("F".into(), vec![0, 1, 5, 7]),
        ("C".into(), vec![0, 8, 9]),
        ("D".into(), vec![0, 8, 10]),
    ];
    assert_eq!(leaves, expected);

    let subtree = tree.root.get_node_by_id(8).unwrap();
    let leaves: Vec<Vec<u64>> = subtree
        .get_leaves_with_paths(Some(vec![42]))
        .into_iter()
        .map(|(_, path)| path)
        .collect();
    assert_eq!(leaves, vec![vec![42, 8, 9], vec![42, 8, 10]]);
}

#[test]
fn clade_kind_predicates() {
    let tree = load(DECIMAL, 0.0);
    let node = |id| tree.root.get_node_by_id(id).unwrap();

    assert!(
        tree.root.is_root() && !tree.root.is_leaf() && !tree.root.is_internal()
    );
    assert!(node(1).is_internal() && !node(1).is_root() && !node(1).is_leaf());
    assert!(node(3).is_leaf() && !node(3).is_root() && !node(3).is_internal());
    assert_eq!(format!("{}", node(5)), "Clade: 5");
}

// ? TREE-05 -------------------------------------------------------------------

#[test]
fn tree_without_kmers_map_serializes_to_pinned_json_shape() {
    let tree = load("((A:0.1,B:0.2)90:0.3,C:0.4);", 0.0);
    let json = serde_json::to_value(&tree).unwrap();

    let expected = serde_json::json!({
        "id": Uuid::new_v3(&Uuid::NAMESPACE_DNS, b"tree.nwk").to_string(),
        "name": "tree.nwk",
        "minBranchSupport": 0.0,
        "root": {
            "id": 0, "parent": null, "kind": "ROOT", "length": 0.0,
            "children": [
                {
                    "id": 1, "parent": 0, "kind": "NODE", "support": 90.0, "length": 0.3,
                    "children": [
                        { "id": 2, "parent": 1, "kind": "LEAF", "name": "A", "length": 0.1 },
                        { "id": 3, "parent": 1, "kind": "LEAF", "name": "B", "length": 0.2 }
                    ]
                },
                { "id": 4, "parent": 0, "kind": "LEAF", "name": "C", "length": 0.4 }
            ]
        },
        "kmersMap": null
    });

    assert_eq!(json, expected);
}

#[test]
fn tree_without_kmers_map_round_trips_through_yaml_and_json() {
    let tree = load(DECIMAL, 0.9);
    let original = serde_json::to_value(&tree).unwrap();

    let from_json: Tree =
        serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&from_json).unwrap(), original);

    let yaml = serde_saphyr::to_string(&tree).unwrap();
    assert!(yaml.contains("kmersMap: null"));
    assert!(!yaml.contains("annotations"));
    let from_yaml: Tree = serde_saphyr::from_str(&yaml).unwrap();
    assert_eq!(serde_json::to_value(&from_yaml).unwrap(), original);

    let dir = TempDir::new().unwrap();
    let path = write(&dir, "db.yaml", &yaml);
    let loaded = Tree::from_yaml_file(&path).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), original);
}

fn flatten(map: &KmersMap) -> BTreeMap<(u64, u64), BTreeSet<u64>> {
    map.get_map()
        .iter()
        .flat_map(|(key, value)| {
            value.0.iter().map(move |(hash, nodes)| {
                ((key.0, *hash), nodes.iter().copied().collect())
            })
        })
        .collect()
}

fn assert_same_tree(a: &Tree, b: &Tree) {
    assert_eq!(a.id, b.id);
    assert_eq!(a.name, b.name);
    assert_eq!(a.min_branch_support, b.min_branch_support);
    assert_eq!(a.root, b.root);

    let (ka, kb) =
        (a.kmers_map.as_ref().unwrap(), b.kmers_map.as_ref().unwrap());
    assert_eq!(ka.get_kmer_size(), kb.get_kmer_size());
    assert_eq!(ka.get_minimizer_size(), kb.get_minimizer_size());
    assert_eq!(flatten(ka), flatten(kb));
}

#[test]
fn tree_with_kmers_map_round_trips_through_yaml_and_json() {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, 200, 3);

    for m in [0, 4] {
        let dir = TempDir::new().unwrap();
        let (tree_path, msa_path) = write_inputs(
            &dir,
            &synth::write_newick(&topology),
            &synth::write_fasta(&records, 60, false, false),
        );
        let tree =
            map_kmers_to_tree(tree_path, msa_path, Some(15), Some(m), None)
                .unwrap();
        assert!(!flatten(tree.kmers_map.as_ref().unwrap()).is_empty());

        let from_json: Tree =
            serde_json::from_str(&serde_json::to_string(&tree).unwrap())
                .unwrap();
        assert_same_tree(&tree, &from_json);

        let yaml = serde_saphyr::to_string(&tree).unwrap();
        let from_yaml: Tree = serde_saphyr::from_str(&yaml).unwrap();
        assert_same_tree(&tree, &from_yaml);

        let path = write(&dir, "db.yaml", &yaml);
        assert_same_tree(&tree, &Tree::from_yaml_file(&path).unwrap());
    }
}

#[test]
#[should_panic(expected = "Could not read file")]
fn from_yaml_file_panics_on_missing_file() {
    let dir = TempDir::new().unwrap();
    let _ = Tree::from_yaml_file(&dir.path().join("missing.yaml"));
}

#[test]
#[should_panic(expected = "Could not parse tree")]
fn from_yaml_file_panics_on_malformed_content() {
    let dir = TempDir::new().unwrap();
    let _ = Tree::from_yaml_file(&write(&dir, "bad.yaml", "not: [a, tree"));
}

// ? TREE-06 ---------------------------------------------------------------------

/// BUG-013: `inMemorySize` was removed; databases written before the removal
/// still carry the key and must keep loading.
#[test]
fn tree_with_legacy_in_memory_size_key_still_deserializes() {
    let tree = load("((A:0.1,B:0.2)90:0.3,C:0.4);", 0.0);
    let mut value = serde_json::to_value(&tree).unwrap();
    assert!(value.get("inMemorySize").is_none());
    value["inMemorySize"] = serde_json::json!("0.000000 Mb");

    let from_json: Tree = serde_json::from_value(value.clone()).unwrap();
    let from_yaml: Tree =
        serde_saphyr::from_str(&serde_saphyr::to_string(&value).unwrap()).unwrap();

    for loaded in [from_json, from_yaml] {
        assert_eq!(loaded.id, tree.id);
        assert_eq!(loaded.root, tree.root);
        assert!(serde_json::to_value(&loaded)
            .unwrap()
            .get("inMemorySize")
            .is_none());
    }
}
