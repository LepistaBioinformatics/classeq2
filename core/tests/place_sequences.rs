mod common;

use classeq_core::{
    domain::dtos::{
        annotation::{Annotation, Tag},
        clade::Clade,
        file_or_stdin::FileOrStdin,
        output_format::OutputFormat,
        tree::Tree,
    },
    use_cases::{map_kmers_to_tree, place_sequences, PlacementTime},
};
use common::{synth, write_inputs};
use mycelium_base::utils::errors::MappedErrors;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const K: u64 = 15;
const L: usize = 800;

struct Dataset {
    dir: TempDir,
    tree: Tree,
    records: Vec<(String, String)>,
}

impl Dataset {
    fn new(seed: u64) -> Self {
        let dir = TempDir::new().unwrap();
        let topology = synth::default_topology();
        let records = synth::evolve(&topology, L, seed);
        let (tree_path, msa_path) = write_inputs(
            &dir,
            &synth::write_newick(&topology),
            &synth::write_fasta(&records, 60, false, false),
        );
        let tree =
            map_kmers_to_tree(tree_path, msa_path, Some(K), Some(4), None)
                .unwrap();
        Dataset { dir, tree, records }
    }

    fn seq(&self, name: &str) -> &str {
        &self.records.iter().find(|(n, _)| n == name).unwrap().1
    }

    fn leaf(&self, name: &str) -> Clade {
        self.tree
            .root
            .get_leaves_with_paths(None)
            .into_iter()
            .find(|(clade, _)| clade.name.as_deref() == Some(name))
            .unwrap()
            .0
    }

    fn leaf_id(&self, name: &str) -> u64 {
        self.leaf(name).id
    }

    fn leaf_parent(&self, name: &str) -> u64 {
        self.leaf(name).parent.unwrap()
    }

    fn write_queries(&self, queries: &[(&str, String)]) -> PathBuf {
        let path = self.dir.path().join("queries.fasta");
        let records: Vec<(String, String)> = queries
            .iter()
            .map(|(name, seq)| (name.to_string(), seq.clone()))
            .collect();
        std::fs::write(&path, synth::write_fasta(&records, 70, false, false))
            .unwrap();
        path
    }

    /// A1 and C1 with mutations, an unrelated sequence and a too-short one.
    fn mixed_queries(&self) -> PathBuf {
        self.write_queries(&[
            ("qA1", synth::mutate(self.seq("A1"), 8, 11)),
            ("qC1", synth::mutate(self.seq("C1"), 8, 12)),
            ("qRandom", synth::random_seq(L, 13)),
            ("qShort", "ACGTACGT".to_string()),
        ])
    }

    fn place(
        &self,
        queries: &Path,
        out: &Path,
        overwrite: bool,
        format: OutputFormat,
    ) -> Result<Vec<PlacementTime>, MappedErrors> {
        place_sequences(
            FileOrStdin::from_file(queries.to_str().unwrap()),
            &self.tree,
            &out.to_path_buf(),
            &None,
            &None,
            &overwrite,
            &format,
            &None,
            &None,
            &None,
        )
    }
}

fn yaml_records(path: &Path) -> HashMap<String, Value> {
    let content = std::fs::read_to_string(path).unwrap();
    content
        .split("---\n")
        .filter(|doc| !doc.trim().is_empty())
        .map(|doc| {
            let value: Value = serde_saphyr::from_str(doc).unwrap();
            (value["query"].as_str().unwrap().to_string(), value)
        })
        .collect()
}

fn jsonl_records(path: &Path) -> HashMap<String, Value> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            (value["query"].as_str().unwrap().to_string(), value)
        })
        .collect()
}

fn annotation_clades(record: &Value) -> Option<Vec<u64>> {
    record.get("annotations").map(|annotations| {
        annotations
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["clade"].as_u64().unwrap())
            .collect()
    })
}

fn annotation(clade: u32, name: &str) -> Annotation {
    Annotation {
        clade,
        meta: Some(vec![Tag::SciName(name.to_string())]),
    }
}

// ? ---------------------------------------------------------------------------
// ? BATCH-01
// ? ---------------------------------------------------------------------------

fn assert_mixed_records(data: &Dataset, records: &HashMap<String, Value>) {
    assert_eq!(
        records.keys().cloned().collect::<HashSet<_>>(),
        HashSet::from(["qA1", "qC1", "qRandom"].map(String::from))
    );

    for (query, leaf) in [("qA1", "A1"), ("qC1", "C1")] {
        let record = &records[query];
        assert_eq!(record["code"], "IdentityFound");
        assert_eq!(record["placement"]["status"], "IdentityFound");
        assert_eq!(
            record["placement"]["clade"]["id"].as_u64(),
            Some(data.leaf_id(leaf)),
            "{record}"
        );
    }

    let random = &records["qRandom"];
    assert!(random["code"]
        .as_str()
        .unwrap()
        .starts_with("Unclassifiable"));
    assert!(random.get("placement").is_none());
    assert!(random.get("annotations").is_none());
}

#[test]
fn batch_writes_one_yaml_document_per_placed_query() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");

    let times = data
        .place(&data.mixed_queries(), &out, false, OutputFormat::Yaml)
        .unwrap();

    let content = std::fs::read_to_string(out.with_extension("yaml")).unwrap();
    assert!(content.starts_with("---\n"));
    assert_eq!(content.matches("---\n").count(), 3);
    assert_mixed_records(&data, &yaml_records(&out.with_extension("yaml")));
    assert_eq!(
        times
            .iter()
            .map(|t| t.sequence.clone())
            .collect::<HashSet<_>>(),
        HashSet::from(["qA1", "qC1", "qRandom", "qShort"].map(String::from))
    );
}

#[test]
fn batch_writes_one_json_line_per_placed_query() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");

    let times = data
        .place(&data.mixed_queries(), &out, false, OutputFormat::Jsonl)
        .unwrap();

    let path = out.with_extension("jsonl");
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), 3);
    assert!(content.ends_with('\n'));
    assert_mixed_records(&data, &jsonl_records(&path));
    assert_eq!(times.len(), 4);
    assert!(!out.with_extension("yaml").exists());
}

#[test]
fn batch_with_empty_query_file_writes_nothing_and_succeeds() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.dir.path().join("empty.fasta");
    std::fs::write(&queries, "").unwrap();

    let times = data
        .place(&queries, &out, false, OutputFormat::Jsonl)
        .unwrap();

    assert!(times.is_empty());
    assert_eq!(
        std::fs::read_to_string(out.with_extension("jsonl")).unwrap(),
        ""
    );
}

// ? ---------------------------------------------------------------------------
// ? BATCH-02
// ? ---------------------------------------------------------------------------

#[test]
fn batch_existing_output_without_overwrite_returns_err() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    std::fs::write(out.with_extension("yaml"), "previous").unwrap();

    let result =
        data.place(&data.mixed_queries(), &out, false, OutputFormat::Yaml);

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(out.with_extension("yaml")).unwrap(),
        "previous"
    );
}

#[test]
fn batch_overwrite_replaces_existing_output() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.mixed_queries();

    data.place(&queries, &out, false, OutputFormat::Jsonl)
        .unwrap();
    data.place(&queries, &out, true, OutputFormat::Jsonl)
        .unwrap();

    let content = std::fs::read_to_string(out.with_extension("jsonl")).unwrap();
    assert_eq!(content.lines().count(), 3);
}

#[test]
fn batch_overwrite_does_not_accumulate_errors_from_previous_runs() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.mixed_queries();

    data.place(&queries, &out, false, OutputFormat::Jsonl)
        .unwrap();
    data.place(&queries, &out, true, OutputFormat::Jsonl)
        .unwrap();

    let errors = std::fs::read_to_string(out.with_extension("error")).unwrap();
    assert_eq!(errors.matches("UCPLACE0005").count(), 1, "{errors:?}");
}

// ? ---------------------------------------------------------------------------
// ? BATCH-03
// ? ---------------------------------------------------------------------------

#[test]
fn batch_error_file_has_one_line_per_failed_query_with_its_header() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.write_queries(&[
        ("short1", "ACGTACGT".to_string()),
        ("qA1", synth::mutate(data.seq("A1"), 8, 11)),
        ("short2", "TTTTGGGG".to_string()),
    ]);

    data.place(&queries, &out, false, OutputFormat::Yaml)
        .unwrap();

    let errors = std::fs::read_to_string(out.with_extension("error")).unwrap();
    let lines: Vec<&str> = errors.lines().collect();
    assert_eq!(lines.len(), 2, "{errors:?}");
    assert!(errors.ends_with('\n'));
    let headers: HashSet<&str> = lines
        .iter()
        .map(|l| l.split('\t').next().unwrap())
        .collect();
    assert_eq!(headers, HashSet::from(["short1", "short2"]));
    assert!(lines.iter().all(|l| l.contains("UCPLACE0005")));
    assert_eq!(yaml_records(&out.with_extension("yaml")).len(), 1);
}

#[test]
fn batch_missing_query_file_returns_err() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");

    let result = data.place(
        &data.dir.path().join("missing.fasta"),
        &out,
        false,
        OutputFormat::Yaml,
    );

    assert!(result.is_err());
    assert!(!out.with_extension("yaml").exists());
}

#[test]
fn batch_query_sequence_before_first_header_returns_err() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.dir.path().join("headerless.fasta");
    std::fs::write(
        &queries,
        format!("{}\n>q1\n{}\n", data.seq("A1"), data.seq("A2")),
    )
    .unwrap();

    let result = data.place(&queries, &out, false, OutputFormat::Yaml);

    assert!(result.is_err());
    assert!(!out.with_extension("yaml").exists());
}

#[test]
fn batch_query_file_without_any_header_returns_err() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    let queries = data.dir.path().join("no-header.fasta");
    std::fs::write(&queries, format!("{}\n", data.seq("A1"))).unwrap();

    let result = data.place(&queries, &out, false, OutputFormat::Yaml);

    assert!(result.is_err());
    assert!(!out.with_extension("yaml").exists());
}

#[test]
fn batch_unreadable_queries_with_overwrite_keep_previous_outputs() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");
    data.place(&data.mixed_queries(), &out, false, OutputFormat::Yaml)
        .unwrap();
    let previous = std::fs::read_to_string(out.with_extension("yaml")).unwrap();

    let result = data.place(
        &data.dir.path().join("missing.fasta"),
        &out,
        true,
        OutputFormat::Yaml,
    );

    assert!(result.is_err());
    assert_eq!(
        std::fs::read_to_string(out.with_extension("yaml")).unwrap(),
        previous
    );
    assert!(out.with_extension("error").exists());
}

// ? ---------------------------------------------------------------------------
// ? BATCH-04
// ? ---------------------------------------------------------------------------

#[test]
fn batch_annotations_include_placed_clade_and_ancestors_sorted() {
    let mut data = Dataset::new(1);
    let a = data.leaf_parent("A1") as u32;
    let c = data.leaf_parent("C1") as u32;
    let ab = data
        .tree
        .root
        .get_node_by_id(a as u64)
        .unwrap()
        .parent
        .unwrap() as u32;
    data.tree.annotations = Some(vec![
        annotation(c, "clade C"),
        annotation(a, "clade A"),
        annotation(0, "root"),
        annotation(ab, "clade AB"),
    ]);
    let out = data.dir.path().join("out");

    data.place(&data.mixed_queries(), &out, false, OutputFormat::Jsonl)
        .unwrap();

    let records = jsonl_records(&out.with_extension("jsonl"));
    let mut expected = vec![0, ab as u64, a as u64];
    expected.sort();
    assert_eq!(annotation_clades(&records["qA1"]), Some(expected));
    assert_eq!(
        records["qA1"]["annotations"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["meta"],
        serde_json::json!([{ "SciName": "clade A" }])
    );
    assert_eq!(annotation_clades(&records["qRandom"]), None);
}

#[test]
fn batch_annotations_absent_when_none_on_path() {
    let mut data = Dataset::new(1);
    let c = data.leaf_parent("C1") as u32;
    data.tree.annotations = Some(vec![annotation(c, "clade C")]);
    let out = data.dir.path().join("out");

    data.place(&data.mixed_queries(), &out, false, OutputFormat::Jsonl)
        .unwrap();

    let records = jsonl_records(&out.with_extension("jsonl"));
    assert_eq!(annotation_clades(&records["qA1"]), None);
    assert_eq!(annotation_clades(&records["qC1"]), Some(vec![c as u64]));
}

#[test]
fn batch_without_tree_annotations_has_no_annotations() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("out");

    data.place(&data.mixed_queries(), &out, false, OutputFormat::Yaml)
        .unwrap();

    let records = yaml_records(&out.with_extension("yaml"));
    assert!(records.values().all(|r| r.get("annotations").is_none()));
}

// ? ---------------------------------------------------------------------------
// ? BATCH-06
// ? ---------------------------------------------------------------------------

#[test]
fn batch_creates_missing_output_dir() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("results").join("out");

    data.place(&data.mixed_queries(), &out, false, OutputFormat::Yaml)
        .unwrap();

    assert_eq!(yaml_records(&out.with_extension("yaml")).len(), 3);
}

#[test]
fn batch_creates_nested_missing_output_dirs() {
    let data = Dataset::new(1);
    let out = data.dir.path().join("a").join("b").join("out");

    data.place(&data.mixed_queries(), &out, false, OutputFormat::Yaml)
        .unwrap();

    assert_eq!(yaml_records(&out.with_extension("yaml")).len(), 3);
}
