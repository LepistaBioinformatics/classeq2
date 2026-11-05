mod common;

#[path = "../../../core/tests/common/synth.rs"]
mod synth;

use common::*;
use predicates::prelude::*;

#[test]
fn missing_required_arguments_exit_with_usage_error() {
    for args in [
        vec!["build-db"],
        vec!["build-db", "tree.nwk"],
        vec!["place"],
        vec!["place", "-d", "db.cls"],
        vec!["describe-db"],
        vec!["convert"],
        vec!["convert", "tree"],
        vec!["convert", "kmers"],
        vec!["convert", "database"],
    ] {
        cls()
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("Usage"));
    }
}

#[test]
fn unknown_subcommand_exits_with_usage_error() {
    cls()
        .arg("unknown")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn invalid_enum_values_exit_with_usage_error() {
    for args in [
        vec!["describe-db", "-d", "db.cls", "-f", "xml"],
        vec!["convert", "database", "-d", "db.cls", "-f", "xml"],
        vec!["place", "-d", "db.cls", "-o", "out", "--out-format", "csv"],
        vec!["--log-format", "text", "describe-db", "-d", "db.cls"],
    ] {
        cls()
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("invalid value"));
    }
}

#[test]
fn invalid_numeric_arguments_exit_with_usage_error() {
    for args in [
        vec!["build-db", "t.nwk", "m.fasta", "-k", "abc"],
        vec!["build-db", "t.nwk", "m.fasta", "-s", "high"],
        vec!["convert", "kmers", "ACGT", "-k", "-1"],
    ] {
        cls().args(&args).assert().code(2);
    }
}
