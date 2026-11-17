//! `cls db list` and `cls db fetch` against a local mirror of the release
//! layout: `<root>/databases/index.json` and `<root>/<tag>/<file>`.

#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use common::*;
use predicates::prelude::*;
use std::{fs, path::Path};
use tempfile::TempDir;

fn sha256(content: &str) -> String {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("payload");
    fs::write(&path, content).unwrap();
    let out = std::process::Command::new("sha256sum")
        .arg(&path)
        .output()
        .expect("sha256sum is needed by this test");
    String::from_utf8(out.stdout).unwrap()[..64].to_string()
}

fn file_entry(name: &str, role: &str, content: &str, sum: &str) -> String {
    format!(
        r#"{{"name": "{name}", "role": "{role}", "size": {}, "sha256": "{sum}"}}"#,
        content.len()
    )
}

/// Writes a mirror with two versions of `demo` and one of `other`. Version 2
/// of `demo` is the latest. `bad_sum` gives the database of version 2 a
/// checksum that its content does not have.
fn mirror(root: &Path, bad_sum: bool) {
    let files = [
        ("db-demo-v1", "demo.cls", "database", "database one\n"),
        (
            "db-demo-v1",
            "annotations.yaml",
            "annotations",
            "- clade: 0\n",
        ),
        ("db-demo-v1", "README.md", "doc", "# demo v1\n"),
        ("db-demo-v2", "demo.cls", "database", "database two\n"),
        (
            "db-demo-v2",
            "annotations.yaml",
            "annotations",
            "- clade: 1\n",
        ),
        ("db-demo-v2", "README.md", "doc", "# demo v2\n"),
        ("db-demo-v2", "align.fasta", "extra", ">a\nACGT\n"),
        ("db-other-v1", "other.cls", "database", "another\n"),
    ];
    let mut entries = std::collections::BTreeMap::<&str, Vec<String>>::new();
    for (tag, name, role, content) in files {
        fs::create_dir_all(root.join(tag)).unwrap();
        fs::write(root.join(tag).join(name), content).unwrap();
        let mut sum = sha256(content);
        if bad_sum && tag == "db-demo-v2" && role == "database" {
            sum = "0".repeat(64);
        }
        entries
            .entry(tag)
            .or_default()
            .push(file_entry(name, role, content, &sum));
    }
    let version = |n: u32, tag: &str, cls: &str| {
        format!(
            r#"{{"version": {n}, "tag": "{tag}", "date": "2026-11-1{n}", "cls": "{cls}", "sha256": "x", "files": [{}]}}"#,
            entries[tag].join(", ")
        )
    };
    let index = format!(
        r#"{{"schema": 1, "databases": [
  {{"name": "demo", "title": "Demo group, geneA", "use": "A demonstration", "versions": [{}, {}]}},
  {{"name": "other", "title": "Other group, geneB", "use": "Another one", "versions": [{}]}}
]}}"#,
        version(1, "db-demo-v1", "0.12.0"),
        version(2, "db-demo-v2", "0.12.0"),
        version(1, "db-other-v1", "0.1.0"),
    );
    fs::create_dir_all(root.join("databases")).unwrap();
    fs::write(root.join("databases/index.json"), index).unwrap();
}

fn index_of(root: &Path) -> String {
    root.join("databases/index.json").display().to_string()
}

#[test]
fn list_prints_every_version_and_marks_the_latest() {
    let dir = TempDir::new().unwrap();
    mirror(dir.path(), false);

    let out = cls()
        .args(["db", "list", "--index", &index_of(dir.path())])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(out).unwrap();
    let lines = out.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 4, "a header and three versions:\n{out}");
    assert!(
        lines[0].starts_with("name\tversion\tlatest\tdate\tcls\tsize\ttitle")
    );
    assert!(lines[1].starts_with("demo\t2\tyes\t2026-11-12\t0.12.0\t"));
    assert!(lines[1].ends_with("\tDemo group, geneA"));
    assert!(lines[2].starts_with("demo\t1\t\t2026-11-11\t"));
    assert!(lines[3].starts_with("other\t1\tyes\t"));
}

#[test]
fn fetch_takes_the_latest_database_annotations_and_readme() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .success();

    let got = out.path().join("demo");
    assert_eq!(
        fs::read_to_string(got.join("demo.cls")).unwrap(),
        "database two\n"
    );
    assert_eq!(
        fs::read_to_string(got.join("annotations.yaml")).unwrap(),
        "- clade: 1\n"
    );
    assert_eq!(
        fs::read_to_string(got.join("README.md")).unwrap(),
        "# demo v2\n"
    );
    assert!(!got.join("align.fasta").exists(), "extra files need --all");
}

#[test]
fn fetch_all_takes_every_file() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--all",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(out.path().join("demo/align.fasta")).unwrap(),
        ">a\nACGT\n"
    );
}

#[test]
fn fetch_takes_the_version_asked_for() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--version",
            "1",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(out.path().join("demo/demo.cls")).unwrap(),
        "database one\n"
    );
}

#[test]
fn unknown_database_and_unknown_version_fail_and_name_what_exists() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);

    cls()
        .args([
            "db",
            "fetch",
            "nothing",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("demo")
                .and(predicate::str::contains("other")),
        );

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--version",
            "9",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("1, 2"));

    assert!(!out.path().join("demo").exists());
}

#[test]
fn a_file_that_fails_its_checksum_is_not_left_on_disk() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), true);

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("checksum"));

    let left = fs::read_dir(out.path().join("demo"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "nothing of a failed download may stay");
}

#[test]
fn an_existing_file_is_kept_unless_forced() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);
    fs::create_dir_all(out.path().join("demo")).unwrap();
    fs::write(out.path().join("demo/demo.cls"), "mine\n").unwrap();

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        fs::read_to_string(out.path().join("demo/demo.cls")).unwrap(),
        "mine\n"
    );
    assert!(!out.path().join("demo/README.md").exists());

    cls()
        .args([
            "db",
            "fetch",
            "demo",
            "--force",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(out.path().join("demo/demo.cls")).unwrap(),
        "database two\n"
    );
}

#[test]
fn a_database_built_by_another_minor_version_gets_a_warning() {
    let dir = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    mirror(dir.path(), false);

    cls()
        .args([
            "db",
            "fetch",
            "other",
            "--index",
            &index_of(dir.path()),
            "-o",
        ])
        .arg(out.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.1.0"));
}

#[test]
fn a_missing_index_fails_with_its_location() {
    cls()
        .args(["db", "list", "--index", "/no/such/place/index.json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("/no/such/place/index.json"));
}
