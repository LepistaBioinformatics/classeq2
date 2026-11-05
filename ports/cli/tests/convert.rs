#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use classeq_core::domain::dtos::clade::{Clade, NodeType};
use classeq_ports_lib::load_database;
use common::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write_tree(dir: &Path) -> PathBuf {
    let path = dir.join("tree.nwk");
    std::fs::write(&path, synth::write_newick(&synth::default_topology()))
        .unwrap();
    path
}

fn count_nodes(clade: &Clade) -> usize {
    let own = usize::from(clade.kind == NodeType::Node);
    own + clade
        .children
        .iter()
        .flatten()
        .map(count_nodes)
        .sum::<usize>()
}

// ? ---------------------------------------------------------------------------
// ? convert tree
// ? ---------------------------------------------------------------------------

#[test]
fn convert_tree_prints_yaml_to_stdout_by_default() {
    let dir = TempDir::new().unwrap();
    let tree_path = write_tree(dir.path());

    let output = cls()
        .args(["convert", "tree"])
        .arg(&tree_path)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: serde_yaml::Value = serde_yaml::from_slice(&output).unwrap();
    assert_eq!(value["name"].as_str(), Some("tree.nwk"));
    assert_eq!(value["minBranchSupport"].as_f64(), Some(95.0));
    let root: Clade = serde_yaml::from_value(value["root"].clone()).unwrap();
    assert_eq!(root.kind, NodeType::Root);
    assert_eq!(count_nodes(&root), 6);
}

#[test]
fn convert_tree_writes_json_file_with_min_branch_support() {
    let dir = TempDir::new().unwrap();
    let tree_path = write_tree(dir.path());
    let out = dir.path().join("tree.json");

    cls()
        .args(["convert", "tree"])
        .arg(&tree_path)
        .args(["--out-format", "jsonl", "--min-branch-support", "100"])
        .arg("-o")
        .arg(&out)
        .assert()
        .success()
        .stdout("");

    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(value["minBranchSupport"].as_f64(), Some(100.0));
    let root: Clade = serde_json::from_value(value["root"].clone()).unwrap();
    assert_eq!(count_nodes(&root), 0);
    assert_eq!(root.children.unwrap().len(), 8);
}

#[test]
fn convert_tree_fails_on_missing_file() {
    let dir = TempDir::new().unwrap();

    cls()
        .args(["convert", "tree"])
        .arg(dir.path().join("missing.nwk"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Could not read file"));
}

#[test]
fn convert_tree_fails_on_unrooted_tree() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("unrooted.nwk");
    std::fs::write(&path, "(A:0.1,B:0.1,C:0.1);").unwrap();

    cls()
        .args(["convert", "tree"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Tree is not rooted"));
}

#[test]
fn convert_tree_fails_when_output_cannot_be_written() {
    let dir = TempDir::new().unwrap();
    let tree_path = write_tree(dir.path());

    cls()
        .args(["convert", "tree"])
        .arg(&tree_path)
        .arg("-o")
        .arg(dir.path().join("missing-dir/tree.yaml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

// ? ---------------------------------------------------------------------------
// ? convert kmers
// ? ---------------------------------------------------------------------------

#[test]
fn convert_kmers_prints_forward_then_reverse_complement_kmers() {
    cls()
        .args(["convert", "kmers", "ACGTTGCAAC", "-k", "4"])
        .assert()
        .success()
        .stdout(
            "ACGT\nCGTT\nGTTG\nTTGC\nTGCA\nGCAA\nCAAC\n\
             GTTG\nTTGC\nTGCA\nGCAA\nCAAC\nAACG\nACGT\n",
        );
}

#[test]
fn convert_kmers_prints_nothing_for_sequence_shorter_than_k() {
    cls()
        .args(["convert", "kmers", "ACG", "-k", "4"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn convert_kmers_uses_default_k_31() {
    let seq = synth::random_seq(40, 3);

    let output = cls()
        .args(["convert", "kmers", &seq])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let lines: Vec<_> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(lines.len(), 2 * (40 - 31 + 1));
    assert!(lines.iter().all(|kmer| kmer.len() == 31));
    assert_eq!(lines[0], seq[..31]);
}

// ? ---------------------------------------------------------------------------
// ? convert database
// ? ---------------------------------------------------------------------------

fn convert_db(db: &Path, out: &Path, format: &str, only_tree: bool) {
    let mut cmd = cls();
    cmd.args(["convert", "database", "-f", format, "-d"])
        .arg(db)
        .arg("-o")
        .arg(out);
    if only_tree {
        cmd.arg("--only-tree");
    }
    cmd.assert().success();
}

#[test]
fn convert_database_round_trips_full_database_in_all_formats() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 41);
    let original = load_database(db.clone()).unwrap();
    let original_map = original.kmers_map.as_ref().unwrap();

    convert_db(&db, &dir.path().join("as-yaml"), "yaml", false);
    convert_db(&db, &dir.path().join("as-zstd"), "zstd", false);
    convert_db(&db, &dir.path().join("as-json"), "json", false);

    let yaml_path = dir.path().join("as-yaml.cls.yaml");
    assert!(serde_yaml::from_str::<serde_yaml::Value>(
        &std::fs::read_to_string(&yaml_path).unwrap()
    )
    .is_ok());

    for path in [yaml_path, dir.path().join("as-zstd.cls")] {
        let converted = load_database(path.clone()).unwrap();
        assert_eq!(converted.id, original.id, "{path:?}");
        assert_eq!(converted.root, original.root, "{path:?}");
        let map = converted.kmers_map.unwrap();
        assert_eq!(map.get_kmer_size(), original_map.get_kmer_size());
        assert_eq!(map.get_map().len(), original_map.get_map().len());
    }

    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("as-json.cls.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(json["id"].as_str(), Some(original.id.to_string().as_str()));
    assert_eq!(json["kmersMap"]["kSize"].as_u64(), Some(K));
    assert_eq!(json["kmersMap"]["mSize"].as_u64(), Some(M));
    assert_eq!(
        json["kmersMap"]["map"].as_object().unwrap().len(),
        original_map.get_map().len()
    );
}

#[test]
fn convert_database_only_tree_writes_root_clade_in_all_formats() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 42);
    let original = load_database(db.clone()).unwrap();

    convert_db(&db, &dir.path().join("t"), "yaml", true);
    convert_db(&db, &dir.path().join("t"), "json", true);
    convert_db(&db, &dir.path().join("t"), "zstd", true);

    let from_yaml: Clade = serde_yaml::from_str(
        &std::fs::read_to_string(dir.path().join("t.cls.yaml")).unwrap(),
    )
    .unwrap();
    let from_json: Clade = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("t.cls.json")).unwrap(),
    )
    .unwrap();
    let decoded = zstd::decode_all(
        std::fs::read(dir.path().join("t.cls")).unwrap().as_slice(),
    )
    .unwrap();
    let from_zstd: Clade = serde_yaml::from_slice(&decoded).unwrap();

    assert_eq!(from_yaml, original.root);
    assert_eq!(from_json, original.root);
    assert_eq!(from_zstd, original.root);

    assert!(load_database(dir.path().join("t.cls.yaml")).is_err());
}

#[test]
fn convert_database_uses_default_output_name() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 43);

    cls()
        .current_dir(dir.path())
        .args(["convert", "database", "-d"])
        .arg(&db)
        .assert()
        .success();

    assert!(load_database(dir.path().join("classeq-database.cls.yaml")).is_ok());
}

#[test]
fn convert_database_reads_shipped_fixture() {
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("bsub");

    cls()
        .args(["convert", "database", "-t", "-f", "json", "-d"])
        .arg(fixture("bsub-gyrb-k35.cls"))
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let converted: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("bsub.cls.json")).unwrap(),
    )
    .unwrap();
    let shipped: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture("bsub-gyrb-k35.cls.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(converted, shipped);
}

#[test]
fn convert_database_fails_on_missing_database() {
    let dir = TempDir::new().unwrap();

    cls()
        .args(["convert", "database", "-d"])
        .arg(dir.path().join("missing.cls"))
        .arg("-o")
        .arg(dir.path().join("out"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error loading database"));
}
