#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use common::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SCENARIOS: [&str; 4] = [
    "MissingLeafFullLength",
    "MissingLeafReads",
    "MissingCladeFullLength",
    "MissingCladeReads",
];

fn write_reference(dir: &Path, seed: u64) -> (PathBuf, PathBuf) {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, seed);
    let tree_path = dir.join("tree.nwk");
    let msa_path = dir.join("msa.fasta");
    std::fs::write(&tree_path, synth::write_newick(&topology)).unwrap();
    std::fs::write(&msa_path, synth::write_fasta(&records, 60, false, false))
        .unwrap();

    (tree_path, msa_path)
}

#[test]
fn assess_reports_every_scenario_and_writes_no_database() {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_reference(dir.path(), 61);
    let report = dir.path().join("report.tsv");
    let per_query = dir.path().join("queries.tsv");

    cls()
        .current_dir(dir.path())
        .arg("assess")
        .arg(&tree_path)
        .arg(&msa_path)
        .args(["-k", &K.to_string(), "-o"])
        .arg(&report)
        .arg("--per-query")
        .arg(&per_query)
        .assert()
        .success();

    let text = std::fs::read_to_string(&report).unwrap();
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}\t")))
            .unwrap_or_else(|| panic!("{key} missing from {text}"))
            .to_string()
    };

    assert_eq!(value("Leaves"), "8");
    assert_eq!(value("KmerSize"), K.to_string());
    assert_eq!(value("MinBranchSupport"), "70");
    assert_eq!(value("ReadLength"), "150");

    for scenario in SCENARIOS {
        let total = ["Exact", "Under", "Over", "Wrong", "Refused"]
            .iter()
            .map(|class| {
                value(&format!("{scenario}{class}")).parse::<f64>().unwrap()
            })
            .sum::<f64>();

        assert!((total - 1.0).abs() < 1e-3, "{scenario}: {total}");
        assert!(["Good", "Shallow", "Poor"]
            .contains(&value(&format!("{scenario}Verdict")).as_str()));
    }

    assert_eq!(value("MissingLeafFullLengthQueries"), "8");
    assert_eq!(value("MissingLeafReadsQueries"), "40");

    let queries = std::fs::read_to_string(&per_query).unwrap();
    let full = value("MissingLeafFullLengthQueries")
        .parse::<usize>()
        .unwrap()
        + value("MissingCladeFullLengthQueries")
            .parse::<usize>()
            .unwrap();

    assert!(queries.starts_with("leaf\tmissing\tmissing_leaves\tread_start\t"));
    assert_eq!(
        queries
            .lines()
            .filter(|line| line.contains("\tfull\t"))
            .count(),
        full
    );

    assert!(!dir.path().join("classeq-database.cls").exists());
}

#[test]
fn assess_writes_json() {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_reference(dir.path(), 62);
    let report = dir.path().join("report.json");

    cls()
        .arg("assess")
        .arg(&tree_path)
        .arg(&msa_path)
        .args(["-k", &K.to_string(), "--reads-per-leaf", "2", "-f", "json"])
        .arg("-o")
        .arg(&report)
        .assert()
        .success();

    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap())
            .unwrap();

    assert_eq!(value["leaves"], 8);
    assert_eq!(value["missingLeaf"]["reads"]["queries"], 16);
    assert!(value["missingClade"]["fullLength"]["verdict"].is_string());
    assert!(value.get("details").is_none());
}

#[test]
fn assess_fails_on_an_unrooted_tree() {
    let dir = TempDir::new().unwrap();
    let tree_path = dir.path().join("tree.nwk");
    let msa_path = dir.path().join("msa.fasta");
    std::fs::write(&tree_path, "(A:0.1,B:0.1,C:0.1);").unwrap();
    std::fs::write(&msa_path, ">A\nACGT\n>B\nACGT\n>C\nACGT\n").unwrap();

    cls()
        .arg("assess")
        .arg(&tree_path)
        .arg(&msa_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not rooted"));
}
