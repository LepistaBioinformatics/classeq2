#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use common::*;
use predicates::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use tempfile::TempDir;

const ALL_KEYS: [&str; 10] = [
    "ID",
    "Name",
    "MinBranchSupport",
    "KmerSize",
    "kmerCount",
    "MinimizerSize",
    "MinimizerCount",
    "MinimizerAvgKmers",
    "LargestMinimizer",
    "SmallestMinimizer",
];

fn all_keys() -> BTreeSet<String> {
    ALL_KEYS.iter().map(|k| k.to_string()).collect()
}

fn describe(db: &Path, format: &str) -> String {
    let output = cls()
        .args(["describe-db", "-f", format, "-d"])
        .arg(db)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

fn describe_json(db: &Path) -> BTreeMap<String, String> {
    serde_json::from_str(&describe(db, "json")).unwrap()
}

fn number(stats: &BTreeMap<String, String>, key: &str) -> usize {
    stats[key].parse().unwrap()
}

#[test]
fn describe_db_emits_all_keys_in_json() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 51);

    let stats = describe_json(&db);

    assert_eq!(stats.keys().cloned().collect::<BTreeSet<_>>(), all_keys());
    assert_eq!(stats["Name"], "tree.nwk");
    assert_eq!(stats["MinBranchSupport"], "70");
    assert_eq!(stats["KmerSize"], K.to_string());
    assert_eq!(stats["MinimizerSize"], M.to_string());

    let count = number(&stats, "kmerCount");
    let minimizers = number(&stats, "MinimizerCount");
    assert!(minimizers > 0 && minimizers <= 4usize.pow(M as u32));
    assert_eq!(number(&stats, "MinimizerAvgKmers"), count / minimizers);
    assert!(
        number(&stats, "SmallestMinimizer")
            <= number(&stats, "MinimizerAvgKmers")
    );
    assert!(
        number(&stats, "MinimizerAvgKmers")
            <= number(&stats, "LargestMinimizer")
    );
}

#[test]
fn describe_db_yaml_and_tsv_match_json() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 52);
    let json = describe_json(&db);

    let yaml: BTreeMap<String, String> =
        serde_saphyr::from_str(&describe(&db, "yaml")).unwrap();

    let tsv: BTreeMap<String, String> = describe(&db, "tsv")
        .lines()
        .map(|line| {
            let (k, v) = line.split_once('\t').expect("tab separated line");
            (k.to_string(), v.to_string())
        })
        .collect();

    assert_eq!(yaml, json);
    assert_eq!(tsv, json);
}

#[test]
fn describe_db_defaults_to_tsv() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 53);

    let output = cls()
        .args(["describe-db", "-d"])
        .arg(&db)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let lines: Vec<_> = std::str::from_utf8(&output).unwrap().lines().collect();
    assert_eq!(lines.len(), ALL_KEYS.len());
    assert!(lines.iter().all(|line| line.split('\t').count() == 2));
}

#[test]
fn describe_db_reads_shipped_fixture() {
    let stats = describe_json(&fixture("bsub-gyrb-k35.cls"));

    assert_eq!(stats.keys().cloned().collect::<BTreeSet<_>>(), all_keys());
    assert_eq!(stats["ID"], "ce47d8bc-2885-3d2c-8247-5b8c8b28fefe");
    assert_eq!(stats["KmerSize"], "35");
    assert_eq!(stats["MinimizerSize"], "4");
    assert_eq!(stats["MinBranchSupport"], "70");
}

/// S-09 / BUG-031: sequences shorter than k produce a database with an empty
/// k-mer map.
#[test]
fn describe_db_reports_zeros_for_empty_kmer_map() {
    let dir = TempDir::new().unwrap();
    let tree_path = dir.path().join("tree.nwk");
    let msa_path = dir.path().join("msa.fasta");
    std::fs::write(&tree_path, "((A:0.1,B:0.1)99:0.1,C:0.1);").unwrap();
    std::fs::write(&msa_path, ">A\nACGTACGT\n>B\nACGAACGA\n>C\nACCAACCA\n")
        .unwrap();
    let db = dir.path().join("short");

    cls()
        .arg("build-db")
        .arg(&tree_path)
        .arg(&msa_path)
        .args(["-k", "15"])
        .arg("-o")
        .arg(&db)
        .assert()
        .success();

    let db = dir.path().join("short.cls");
    let output = cls()
        .args(["describe-db", "-f", "json", "-d"])
        .arg(&db)
        .assert()
        .success()
        .stderr(predicate::str::contains("panicked").not())
        .get_output()
        .stdout
        .clone();

    let stats: BTreeMap<String, String> =
        serde_json::from_slice(&output).unwrap();
    assert_eq!(stats.keys().cloned().collect::<BTreeSet<_>>(), all_keys());
    assert_eq!(stats["KmerSize"], "15");
    for key in [
        "kmerCount",
        "MinimizerCount",
        "MinimizerAvgKmers",
        "LargestMinimizer",
        "SmallestMinimizer",
    ] {
        assert_eq!(stats[key], "0", "{key}");
    }
}

#[test]
fn describe_db_fails_on_missing_database() {
    let dir = TempDir::new().unwrap();

    cls()
        .args(["describe-db", "-d"])
        .arg(dir.path().join("missing.cls"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error loading database"));
}

#[test]
fn describe_db_fails_on_tree_only_export() {
    cls()
        .args(["describe-db", "-d"])
        .arg(fixture("bsub-gyrb-k35.cls.yaml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error loading database"));
}

/// 005-placement-performance FMT-05: the legacy and binary copies of the
/// same database describe identically.
#[test]
fn describe_db_is_identical_for_legacy_and_binary_copies() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, .. } = build_synthetic_db(dir.path(), 55);

    cls()
        .args(["convert", "database", "-f", "zstd", "-d"])
        .arg(&db)
        .arg("-o")
        .arg(dir.path().join("legacy"))
        .assert()
        .success();

    assert_eq!(
        describe_json(&db),
        describe_json(&dir.path().join("legacy.cls"))
    );
}
