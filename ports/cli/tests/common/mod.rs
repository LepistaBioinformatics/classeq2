#![allow(dead_code)]

use crate::synth;

use assert_cmd::Command;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub const K: u64 = 15;
pub const M: u64 = 4;
pub const L: usize = 800;

pub fn cls() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cls"))
}

pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/models")
        .join(name)
}

pub struct SyntheticDb {
    pub db: PathBuf,
    pub records: Vec<(String, String)>,
}

/// Writes the default synthetic dataset into `dir` and builds `db.cls` with
/// `cls build-db -k 15 -m 4`.
pub fn build_synthetic_db(dir: &Path, seed: u64) -> SyntheticDb {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, seed);
    let tree_path = dir.join("tree.nwk");
    let msa_path = dir.join("msa.fasta");
    std::fs::write(&tree_path, synth::write_newick(&topology)).unwrap();
    std::fs::write(&msa_path, synth::write_fasta(&records, 60, false, false))
        .unwrap();

    let db = dir.join("db.cls");
    cls()
        .arg("build-db")
        .arg(&tree_path)
        .arg(&msa_path)
        .args(["-k", &K.to_string(), "-m", &M.to_string()])
        .arg("-o")
        .arg(&db)
        .assert()
        .success();

    assert!(db.exists());
    SyntheticDb { db, records }
}

/// Runs `cls convert database -f json -t` and returns the root clade.
pub fn tree_json(db: &Path, dir: &Path) -> serde_json::Value {
    let out = dir.join("tree-only");
    cls()
        .args(["convert", "database", "-f", "json", "-t", "-d"])
        .arg(db)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let content =
        std::fs::read_to_string(dir.join("tree-only.cls.json")).unwrap();
    serde_json::from_str(&content).unwrap()
}

/// Maps each leaf name to `(leaf id, parent id)`.
pub fn leaf_parents(root: &serde_json::Value) -> HashMap<String, (u64, u64)> {
    fn walk(node: &serde_json::Value, out: &mut HashMap<String, (u64, u64)>) {
        if node["kind"] == "LEAF" {
            out.insert(
                node["name"].as_str().unwrap().to_string(),
                (
                    node["id"].as_u64().unwrap(),
                    node["parent"].as_u64().unwrap(),
                ),
            );
        }
        if let Some(children) = node["children"].as_array() {
            for child in children {
                walk(child, out);
            }
        }
    }

    let mut out = HashMap::new();
    walk(root, &mut out);
    out
}

/// Ids of `parent` and its direct children.
pub fn clade_ids(root: &serde_json::Value, parent: u64) -> Vec<u64> {
    fn find(node: &serde_json::Value, id: u64) -> Option<&serde_json::Value> {
        if node["id"].as_u64() == Some(id) {
            return Some(node);
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|child| find(child, id))
    }

    let node = find(root, parent).expect("clade not found");
    let mut ids = vec![parent];
    if let Some(children) = node["children"].as_array() {
        ids.extend(children.iter().map(|c| c["id"].as_u64().unwrap()));
    }
    ids
}

pub fn write_queries(path: &Path, records: &[(String, String)]) {
    std::fs::write(path, synth::write_fasta(records, 60, false, false))
        .unwrap();
}

/// Parses a multi-document YAML placement output, keyed by query.
pub fn read_yaml_results(path: &Path) -> HashMap<String, serde_json::Value> {
    let content = std::fs::read_to_string(path).unwrap();
    serde_saphyr::from_multiple::<serde_json::Value>(&content)
        .unwrap()
        .into_iter()
        .map(|value| (value["query"].as_str().unwrap().to_string(), value))
        .collect()
}

/// Parses a JSONL placement output, keyed by query.
pub fn read_jsonl_results(path: &Path) -> HashMap<String, serde_json::Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            (value["query"].as_str().unwrap().to_string(), value)
        })
        .collect()
}

/// The node id a placement points to. `None` for Unclassifiable (no
/// `placement` key) and Inconclusive (a list of proposals).
pub fn placed_id(result: &serde_json::Value) -> Option<u64> {
    let placement = result.get("placement")?;

    match placement["status"].as_str()? {
        "IdentityFound" | "MaxResolutionReached" => (),
        _ => return None,
    }

    let clade = placement.get("clade")?;
    clade.as_u64().or_else(|| clade.get("id")?.as_u64())
}
