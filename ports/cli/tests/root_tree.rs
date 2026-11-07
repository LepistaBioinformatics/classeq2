//! 004-tree-rooting: `cls root-tree` and the rooting checks of `build-db`.

#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use classeq_core::domain::dtos::clade::Clade;
use classeq_ports_lib::load_database;
use common::*;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const F2: &str =
    "(A:0.1,B:0.2,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0.7);";

/// The default synthetic topology written unrooted: the root's two children
/// are merged into a trifurcation.
const UNROOTED_SYNTHETIC: &str = "((A1:0.01,A2:0.01)99:0.08,(B1:0.01,B2:0.01)99:0.08,\
((C1:0.01,C2:0.01)99:0.08,(D1:0.01,D2:0.01)99:0.08)99:0.16);";

fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn no_panic() -> impl Predicate<str> {
    predicate::str::contains("panicked").not()
}

// ? ---------------------------------------------------------------------------
// ? cls root-tree
// ? ---------------------------------------------------------------------------

#[test]
fn root_tree_reads_stdin_and_writes_only_newick_to_stdout() {
    cls()
        .args(["root-tree", "-"])
        .write_stdin(F2)
        .assert()
        .success()
        .stdout(
            "((A:0.1,B:0.2)80:0.7,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0);\n",
        );
}

#[test]
fn root_tree_writes_to_the_output_file() {
    let dir = TempDir::new().unwrap();
    let input = write(dir.path(), "in.nwk", F2);
    let output = dir.path().join("out.nwk");

    cls()
        .arg("root-tree")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .success()
        .stdout("");

    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        "((A:0.1,B:0.2)80:0.7,((C:0.3,D:0.4)90:0.5,(E:0.05,F:0.05)70:0.6)80:0);\n"
    );
}

#[test]
fn root_tree_with_a_comma_separated_outgroup() {
    cls()
        .args(["root-tree", "-", "--outgroup", "E,F"])
        .write_stdin(F2)
        .assert()
        .success()
        .stdout(
            "(((A:0.1,B:0.2)80:0.7,(C:0.3,D:0.4)90:0.5)70:0.3,(E:0.05,F:0.05)70:0.3);\n",
        );
}

#[test]
fn root_tree_rejects_outgroup_with_the_midpoint_method() {
    cls()
        .args(["root-tree", "-", "--method", "midpoint", "--outgroup", "E"])
        .write_stdin(F2)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be combined").and(no_panic()));
}

#[test]
fn root_tree_reports_unknown_outgroup_leaves() {
    cls()
        .args(["root-tree", "-", "--outgroup", "Z"])
        .write_stdin(F2)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown outgroup leaves").and(no_panic()));
}

#[test]
fn root_tree_reports_missing_branch_lengths() {
    cls()
        .args(["root-tree", "-"])
        .write_stdin("(A,B,(C,D)90);")
        .assert()
        .failure()
        .stderr(predicate::str::contains("branch lengths").and(no_panic()));
}

#[test]
fn root_tree_keep_if_rooted_leaves_a_rooted_tree_alone() {
    cls()
        .args(["root-tree", "-", "--keep-if-rooted"])
        .write_stdin("((A:1,B:1)90:1,C:5);")
        .assert()
        .success()
        .stdout("((A:1,B:1)90:1,C:5);\n");
}

// ? ---------------------------------------------------------------------------
// ? build-db rooting checks and the root-tree -> build-db -> place pipeline
// ? ---------------------------------------------------------------------------

fn write_unrooted_dataset(dir: &Path) -> (PathBuf, PathBuf, Vec<(String, String)>) {
    let records = synth::evolve(&synth::default_topology(), L, 7);
    let tree = write(dir, "unrooted.nwk", UNROOTED_SYNTHETIC);
    let msa = write(dir, "msa.fasta", &synth::write_fasta(&records, 60, false, false));
    (tree, msa, records)
}

fn build_db(tree: &Path, msa: &Path, db: &Path) -> assert_cmd::Command {
    let mut cmd = cls();
    cmd.arg("build-db")
        .arg(tree)
        .arg(msa)
        .args(["-k", &K.to_string(), "-m", &M.to_string()])
        .arg("-o")
        .arg(db);
    cmd
}

fn supports(clade: &Clade) -> Vec<f64> {
    let mut out: Vec<f64> = clade.support.into_iter().collect();
    for child in clade.children.iter().flatten() {
        out.extend(supports(child));
    }
    out
}

#[test]
fn build_db_rejects_an_unrooted_tree_with_a_hint() {
    let dir = TempDir::new().unwrap();
    let (tree, msa, _) = write_unrooted_dataset(dir.path());

    build_db(&tree, &msa, &dir.path().join("db.cls"))
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not rooted (the root has 3 children)")
                .and(predicate::str::contains("cls root-tree"))
                .and(no_panic()),
        );
}

#[test]
fn root_tree_then_build_db_then_place() {
    let dir = TempDir::new().unwrap();
    let (tree, msa, records) = write_unrooted_dataset(dir.path());
    let rooted = dir.path().join("rooted.nwk");
    let db = dir.path().join("db.cls");

    cls()
        .arg("root-tree")
        .arg(&tree)
        .arg("-o")
        .arg(&rooted)
        .assert()
        .success();
    build_db(&rooted, &msa, &db).assert().success();

    let tree = load_database(db.clone()).unwrap();
    assert_eq!(tree.root.children.as_ref().unwrap().len(), 2);
    assert!(supports(&tree.root).contains(&99.0), "supports kept (F-1)");

    let queries = dir.path().join("queries.fasta");
    write_queries(&queries, &records[..1]);
    let out = dir.path().join("out");
    cls()
        .arg("place")
        .arg(&queries)
        .arg("-d")
        .arg(&db)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let results = read_yaml_results(&out.with_extension("yaml"));
    assert_eq!(results.len(), 1);
    assert!(placed_id(&results[&records[0].0]).is_some(), "{results:?}");
}

#[test]
fn build_db_with_root_midpoint_roots_in_memory() {
    let dir = TempDir::new().unwrap();
    let (tree, msa, _) = write_unrooted_dataset(dir.path());
    let db = dir.path().join("db.cls");

    build_db(&tree, &msa, &db)
        .args(["--root", "midpoint"])
        .assert()
        .success();

    let tree = load_database(db).unwrap();
    assert_eq!(tree.root.children.as_ref().unwrap().len(), 2);
}

#[test]
fn build_db_warns_when_collapsing_makes_the_root_a_polytomy() {
    let dir = TempDir::new().unwrap();
    let (_, msa, _) = write_unrooted_dataset(dir.path());
    let tree = write(
        dir.path(),
        "rooted.nwk",
        "(((A1:0.01,A2:0.01)99:0.08,(B1:0.01,B2:0.01)99:0.08)30:0.08,\
         ((C1:0.01,C2:0.01)99:0.08,(D1:0.01,D2:0.01)99:0.08)99:0.08);",
    );

    build_db(&tree, &msa, &dir.path().join("db.cls"))
        .args(["-s", "50"])
        .assert()
        .success()
        .stderr(predicate::str::contains("polytomy"));
}
