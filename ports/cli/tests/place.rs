#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use common::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Setup {
    dir: TempDir,
    db: PathBuf,
    root: serde_json::Value,
    queries: Vec<(String, String)>,
    query_path: PathBuf,
}

fn setup(seed: u64) -> Setup {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, records } = build_synthetic_db(dir.path(), seed);
    let root = tree_json(&db, dir.path());
    let queries: Vec<(String, String)> = records
        .iter()
        .filter(|(name, _)| name == "A1" || name == "C2")
        .map(|(name, seq)| {
            (format!("q_{name}"), synth::mutate(seq, 3, seed + 1))
        })
        .collect();
    let query_path = dir.path().join("queries.fasta");
    write_queries(&query_path, &queries);

    Setup {
        dir,
        db,
        root,
        queries,
        query_path,
    }
}

fn place(db: &Path, out: &Path) -> assert_cmd::Command {
    let mut cmd = cls();
    cmd.arg("place").arg("-d").arg(db).arg("-o").arg(out);
    cmd
}

fn assert_placed_results(
    results: &std::collections::HashMap<String, serde_json::Value>,
    queries: &[(String, String)],
) {
    assert_eq!(results.len(), queries.len());
    for (name, _) in queries {
        let result = &results[name];
        assert!(result["code"].is_string(), "{result}");
        assert!(placed_id(result).is_some(), "{name} not placed: {result}");
    }
}

#[test]
fn place_from_file_writes_yaml_documents() {
    let s = setup(11);
    let out = s.dir.path().join("out");

    place(&s.db, &out).arg(&s.query_path).assert().success();

    let path = s.dir.path().join("out.yaml");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.matches("---\n").count(), s.queries.len());
    assert_placed_results(&read_yaml_results(&path), &s.queries);
}

#[test]
fn place_writes_jsonl_lines() {
    let s = setup(12);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .args(["--out-format", "jsonl"])
        .assert()
        .success();

    assert!(!s.dir.path().join("out.yaml").exists());
    let path = s.dir.path().join("out.jsonl");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), s.queries.len());
    assert_placed_results(&read_jsonl_results(&path), &s.queries);
}

#[test]
fn place_reads_query_from_stdin_by_default() {
    let s = setup(13);
    let out = s.dir.path().join("out");
    let fasta = std::fs::read_to_string(&s.query_path).unwrap();

    place(&s.db, &out).write_stdin(fasta).assert().success();

    assert_placed_results(
        &read_yaml_results(&s.dir.path().join("out.yaml")),
        &s.queries,
    );
}

#[test]
fn place_reads_query_from_explicit_stdin_dash() {
    let s = setup(14);
    let out = s.dir.path().join("out");
    let fasta = std::fs::read_to_string(&s.query_path).unwrap();

    place(&s.db, &out)
        .arg("-")
        .args(["--out-format", "jsonl"])
        .write_stdin(fasta)
        .assert()
        .success();

    assert_placed_results(
        &read_jsonl_results(&s.dir.path().join("out.jsonl")),
        &s.queries,
    );
}

#[test]
fn place_refuses_to_overwrite_without_force() {
    let s = setup(15);
    let out = s.dir.path().join("out");
    let path = s.dir.path().join("out.yaml");
    std::fs::write(&path, "sentinel\n").unwrap();

    place(&s.db, &out)
        .arg(&s.query_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Could not overwrite existing file",
        ));

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "sentinel\n");
}

#[test]
fn place_overwrites_with_force() {
    let s = setup(16);
    let out = s.dir.path().join("out");

    place(&s.db, &out).arg(&s.query_path).assert().success();
    place(&s.db, &out)
        .arg(&s.query_path)
        .arg("-f")
        .assert()
        .success();

    let path = s.dir.path().join("out.yaml");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.matches("---\n").count(), s.queries.len());
    assert_placed_results(&read_yaml_results(&path), &s.queries);
}

#[test]
fn place_without_annotations_omits_annotations_key() {
    let s = setup(17);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .args(["--out-format", "jsonl"])
        .assert()
        .success();

    for result in read_jsonl_results(&s.dir.path().join("out.jsonl")).values() {
        assert!(result.get("annotations").is_none(), "{result}");
    }
}

fn write_annotations(s: &Setup) -> (PathBuf, u64, u64) {
    let leaves = leaf_parents(&s.root);
    let a_clade = leaves["A1"].1;
    let b_clade = leaves["B1"].1;
    let path = s.dir.path().join("annotations.yaml");
    std::fs::write(
        &path,
        format!(
            "- clade: 0\n  meta:\n  - !Taxid 1\n  - !SciName root\n\
             - clade: {a_clade}\n  meta:\n  - !Rank species\n  - !Note clade A\n\
             - clade: {b_clade}\n  meta:\n  - !Note clade B\n"
        ),
    )
    .unwrap();
    (path, a_clade, b_clade)
}

fn annotation_clades(result: &serde_json::Value) -> Vec<u64> {
    result["annotations"]
        .as_array()
        .unwrap_or_else(|| panic!("annotations missing: {result}"))
        .iter()
        .map(|a| a["clade"].as_u64().unwrap())
        .collect()
}

#[test]
fn place_adds_annotations_on_path_to_root_in_jsonl() {
    let s = setup(18);
    let (annotations, a_clade, _) = write_annotations(&s);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .arg("-a")
        .arg(&annotations)
        .args(["--out-format", "jsonl"])
        .assert()
        .success();

    let results = read_jsonl_results(&s.dir.path().join("out.jsonl"));
    let a_result = &results["q_A1"];
    assert_eq!(annotation_clades(a_result), vec![0, a_clade]);
    assert_eq!(
        a_result["annotations"][0]["meta"],
        serde_json::json!([{"Taxid": 1}, {"SciName": "root"}])
    );
    assert_eq!(
        a_result["annotations"][1]["meta"],
        serde_json::json!([{"Rank": "species"}, {"Note": "clade A"}])
    );

    assert_eq!(annotation_clades(&results["q_C2"]), vec![0]);
}

#[test]
fn place_adds_annotations_in_yaml() {
    let s = setup(19);
    let (annotations, a_clade, _) = write_annotations(&s);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .arg("-a")
        .arg(&annotations)
        .assert()
        .success();

    let content =
        std::fs::read_to_string(s.dir.path().join("out.yaml")).unwrap();
    assert!(content.contains("Taxid: 1"), "{content}");
    assert!(content.contains("Note: clade A"), "{content}");
    assert!(!content.contains("clade B"), "{content}");

    let results = read_yaml_results(&s.dir.path().join("out.yaml"));
    assert_eq!(annotation_clades(&results["q_A1"]), vec![0, a_clade]);
}

#[test]
fn place_with_empty_annotations_file_omits_annotations_key() {
    let s = setup(20);
    let annotations = s.dir.path().join("annotations.yaml");
    std::fs::write(&annotations, "[]\n").unwrap();
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .arg("-a")
        .arg(&annotations)
        .args(["--out-format", "jsonl"])
        .assert()
        .success();

    for result in read_jsonl_results(&s.dir.path().join("out.jsonl")).values() {
        assert!(result.get("annotations").is_none(), "{result}");
    }
}

#[test]
fn place_fails_on_missing_annotations_file() {
    let s = setup(21);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&s.query_path)
        .arg("-a")
        .arg(s.dir.path().join("missing.yaml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"));
}

#[test]
fn place_fails_on_missing_database() {
    let s = setup(22);
    let out = s.dir.path().join("out");

    place(&s.dir.path().join("missing.cls"), &out)
        .arg(&s.query_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error loading database"));
}

// ? ---------------------------------------------------------------------------
// ? S-08 / BUG-030, BUG-032 and malformed query input
// ? ---------------------------------------------------------------------------

#[test]
fn place_with_zero_queries_succeeds_with_empty_output() {
    let s = setup(23);
    let empty = s.dir.path().join("empty.fasta");
    std::fs::write(&empty, "").unwrap();
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&empty)
        .assert()
        .success()
        .stderr(predicate::str::contains("divide by zero").not());

    assert_eq!(
        std::fs::read_to_string(s.dir.path().join("out.yaml")).unwrap(),
        ""
    );
}

#[test]
fn place_with_empty_stdin_succeeds_with_empty_output() {
    let s = setup(24);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .args(["--out-format", "jsonl"])
        .write_stdin("")
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(s.dir.path().join("out.jsonl")).unwrap(),
        ""
    );
}

#[test]
fn place_fails_on_missing_query_file() {
    let s = setup(25);
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(s.dir.path().join("missing.fasta"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file or directory"))
        .stderr(predicate::str::contains("divide by zero").not());

    assert!(!s.dir.path().join("out.yaml").exists());
}

#[test]
fn place_fails_on_fasta_without_header() {
    let s = setup(26);
    let malformed = s.dir.path().join("malformed.fasta");
    std::fs::write(&malformed, "ACGTACGTACGT\n>q1\nACGTACGT\n").unwrap();
    let out = s.dir.path().join("out");

    place(&s.db, &out)
        .arg(&malformed)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unexpected sequence without header",
        ));
}
