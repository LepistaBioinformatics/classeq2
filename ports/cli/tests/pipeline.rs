#[path = "../../../core/tests/common/synth.rs"]
mod synth;

mod common;

use common::*;
use tempfile::TempDir;

/// CLI-06: build-db then place; every mutated leaf query lands in the clade
/// of its source leaf (the leaf's parent node or one of its children).
#[test]
fn pipeline_places_mutated_leaves_in_source_clade() {
    let dir = TempDir::new().unwrap();
    let SyntheticDb { db, records } = build_synthetic_db(dir.path(), 601);
    let root = tree_json(&db, dir.path());
    let leaves = leaf_parents(&root);

    let queries: Vec<(String, String)> = records
        .iter()
        .enumerate()
        .map(|(i, (name, seq))| {
            (format!("q_{name}"), synth::mutate(seq, 4, 700 + i as u64))
        })
        .collect();
    let query_path = dir.path().join("queries.fasta");
    write_queries(&query_path, &queries);

    let out = dir.path().join("placements");
    cls()
        .arg("place")
        .arg(&query_path)
        .arg("-d")
        .arg(&db)
        .arg("-o")
        .arg(&out)
        .assert()
        .success();

    let results = read_yaml_results(&dir.path().join("placements.yaml"));
    assert_eq!(results.len(), records.len());

    for (name, _) in &records {
        let result = &results[&format!("q_{name}")];
        let (_, parent) = leaves[name];
        let accepted = clade_ids(&root, parent);
        let placed = placed_id(result)
            .unwrap_or_else(|| panic!("{name} was not placed: {result}"));

        assert!(
            accepted.contains(&placed),
            "{name} placed at {placed}, expected one of {accepted:?}: {result}"
        );
    }
}
