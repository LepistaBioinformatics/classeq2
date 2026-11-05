#![allow(dead_code)]

pub mod synth;

use std::path::PathBuf;
use tempfile::TempDir;

/// Writes `tree.nwk` and `msa.fasta` into `dir` and returns their paths.
pub fn write_inputs(
    dir: &TempDir,
    newick: &str,
    fasta: &str,
) -> (PathBuf, PathBuf) {
    let tree_path = dir.path().join("tree.nwk");
    let msa_path = dir.path().join("msa.fasta");
    std::fs::write(&tree_path, newick).unwrap();
    std::fs::write(&msa_path, fasta).unwrap();
    (tree_path, msa_path)
}
