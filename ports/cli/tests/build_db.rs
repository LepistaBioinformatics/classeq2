#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use classeq_core::domain::dtos::{clade::NodeType, tree::Tree};
use classeq_ports_lib::load_database;
use common::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

fn write_inputs(dir: &Path, seed: u64) -> (PathBuf, PathBuf) {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, seed);
    let tree_path = dir.join("tree.nwk");
    let msa_path = dir.join("msa.fasta");
    std::fs::write(&tree_path, synth::write_newick(&topology)).unwrap();
    std::fs::write(&msa_path, synth::write_fasta(&records, 60, true, false))
        .unwrap();
    (tree_path, msa_path)
}

fn count_kinds(tree: &Tree) -> (usize, usize) {
    fn walk(
        clade: &classeq_core::domain::dtos::clade::Clade,
        acc: &mut (usize, usize),
    ) {
        match clade.kind {
            NodeType::Node => acc.0 += 1,
            NodeType::Leaf => acc.1 += 1,
            NodeType::Root => {}
        }
        for child in clade.children.iter().flatten() {
            walk(child, acc);
        }
    }
    let mut acc = (0, 0);
    walk(&tree.root, &mut acc);
    acc
}

#[test]
fn build_db_writes_zstd_database_with_given_parameters() {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_inputs(dir.path(), 31);

    cls()
        .arg("build-db")
        .arg(&tree_path)
        .arg(&msa_path)
        .args(["-k", "21", "-m", "5", "-s", "80"])
        .arg("-o")
        .arg(dir.path().join("custom.yaml"))
        .assert()
        .success();

    assert!(!dir.path().join("custom.yaml").exists());
    let db = dir.path().join("custom.cls");
    assert_eq!(std::fs::read(&db).unwrap()[..4], ZSTD_MAGIC);

    let tree = load_database(db).unwrap();
    assert_eq!(tree.name, "tree.nwk");
    assert_eq!(tree.min_branch_support, 80.0);
    assert_eq!(count_kinds(&tree), (6, 8));
    let kmers_map = tree.kmers_map.unwrap();
    assert_eq!(kmers_map.get_kmer_size(), 21);
    assert_eq!(kmers_map.get_minimizer_size(), 5);
    assert!(!kmers_map.get_map().is_empty());
}

#[test]
fn build_db_uses_default_parameters_and_output_name() {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_inputs(dir.path(), 32);

    cls()
        .current_dir(dir.path())
        .arg("build-db")
        .arg(&tree_path)
        .arg(&msa_path)
        .assert()
        .success();

    let tree = load_database(dir.path().join("classeq-database.cls")).unwrap();
    assert_eq!(tree.min_branch_support, 70.0);
    let kmers_map = tree.kmers_map.unwrap();
    assert_eq!(kmers_map.get_kmer_size(), 35);
    assert_eq!(kmers_map.get_minimizer_size(), 4);
}

#[test]
fn build_db_min_branch_support_collapses_low_support_nodes() {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_inputs(dir.path(), 33);

    for (support, output, expected) in
        [("99", "kept", (6, 8)), ("100", "collapsed", (0, 8))]
    {
        cls()
            .arg("build-db")
            .arg(&tree_path)
            .arg(&msa_path)
            .args(["-k", "15", "-s", support])
            .arg("-o")
            .arg(dir.path().join(output))
            .assert()
            .success();

        let tree =
            load_database(dir.path().join(format!("{output}.cls"))).unwrap();
        assert_eq!(count_kinds(&tree), expected, "support {support}");
    }
}

#[test]
fn build_db_fails_on_missing_tree() {
    let dir = TempDir::new().unwrap();
    let (_, msa_path) = write_inputs(dir.path(), 34);

    cls()
        .arg("build-db")
        .arg(dir.path().join("missing.nwk"))
        .arg(&msa_path)
        .arg("-o")
        .arg(dir.path().join("db"))
        .assert()
        .failure();

    assert!(!dir.path().join("db.cls").exists());
}

#[test]
fn build_db_fails_on_missing_msa() {
    let dir = TempDir::new().unwrap();
    let (tree_path, _) = write_inputs(dir.path(), 35);

    cls()
        .arg("build-db")
        .arg(&tree_path)
        .arg(dir.path().join("missing.fasta"))
        .arg("-o")
        .arg(dir.path().join("db"))
        .assert()
        .failure();

    assert!(!dir.path().join("db.cls").exists());
}

#[test]
fn build_db_requires_tree_and_msa_arguments() {
    cls()
        .arg("build-db")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("required arguments"));
}
