mod common;

use classeq_core::{
    domain::dtos::{file_or_stdin::FileOrStdin, output_format::OutputFormat},
    use_cases::{map_kmers_to_tree, place_sequences},
};
use common::{synth, write_inputs};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tempfile::TempDir;

const K: u64 = 15;
const L: usize = 800;
const SEEDS: [u64; 4] = [1, 2, 3, 4];
const READS_PER_LEAF: usize = 3;
const READ_LEN: usize = 250;
const READ_SUBS: usize = 5;
// 2% substitutions at k=15 keep ~0.98^15 = 74% of k-mers on average, so the
// default min_match_coverage of 0.7 rejects about a sixth of the reads.
const MIN_MATCH_COVERAGE: f64 = 0.6;

#[derive(Default, Debug)]
struct Tally {
    correct: usize,
    ancestor: usize,
    wrong: usize,
    unplaced: usize,
}

impl Tally {
    fn total(&self) -> usize {
        self.correct + self.ancestor + self.wrong + self.unplaced
    }
}

fn reverse_complement(seq: &str) -> String {
    seq.chars()
        .rev()
        .map(|c| match c {
            'A' => 'T',
            'T' => 'A',
            'C' => 'G',
            _ => 'C',
        })
        .collect()
}

fn placed_clade(record: &Value) -> Option<u64> {
    let placement = record.get("placement")?;
    match placement["status"].as_str()? {
        "IdentityFound" | "MaxResolutionReached" => placement["clade"]["id"]
            .as_u64()
            .or_else(|| placement["clade"].as_u64()),
        _ => None,
    }
}

fn run_seed(seed: u64, tally: &mut Tally) {
    let dir = TempDir::new().unwrap();
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, seed);
    let (tree_path, msa_path) = write_inputs(
        &dir,
        &synth::write_newick(&topology),
        &synth::write_fasta(&records, 60, false, false),
    );
    let tree =
        map_kmers_to_tree(tree_path, msa_path, Some(K), Some(4), None).unwrap();

    let leaves: HashMap<String, (u64, HashSet<u64>)> = tree
        .root
        .get_leaves_with_paths(None)
        .into_iter()
        .map(|(clade, path)| {
            (
                clade.name.unwrap(),
                (clade.parent.unwrap(), path.into_iter().collect()),
            )
        })
        .collect();

    let mut rng = synth::Rng::new(seed * 1000);
    let mut queries = Vec::new();
    for (name, seq) in &records {
        for i in 0..READS_PER_LEAF {
            let start = rng.below(L - READ_LEN);
            let read = synth::mutate(
                &synth::fragment(seq, start, READ_LEN),
                READ_SUBS,
                rng.next_u64(),
            );
            let read = if i % 2 == 1 {
                reverse_complement(&read)
            } else {
                read
            };
            queries.push((format!("{name}|{i}"), read));
        }
    }

    let query_path = dir.path().join("queries.fasta");
    std::fs::write(&query_path, synth::write_fasta(&queries, 0, false, false))
        .unwrap();
    let out = dir.path().join("out");

    place_sequences(
        FileOrStdin::from_file(query_path.to_str().unwrap()),
        &tree,
        &out,
        &None,
        &Some(MIN_MATCH_COVERAGE),
        &false,
        &OutputFormat::Jsonl,
        &None,
        &None,
    )
    .unwrap();

    let placed: HashMap<String, Value> =
        std::fs::read_to_string(out.with_extension("jsonl"))
            .unwrap()
            .lines()
            .map(|line| {
                let value: Value = serde_json::from_str(line).unwrap();
                (value["query"].as_str().unwrap().to_string(), value)
            })
            .collect();

    for (query, _) in &queries {
        let leaf = query.split('|').next().unwrap();
        let (clade, path) = &leaves[leaf];
        match placed.get(query).and_then(placed_clade) {
            Some(id) if id == *clade => tally.correct += 1,
            Some(id) if id != tree.root.id && path.contains(&id) => {
                tally.ancestor += 1
            }
            Some(_) => tally.wrong += 1,
            None => tally.unplaced += 1,
        }
    }
}

#[test]
fn accuracy_mutated_reads_are_placed_in_their_source_clade() {
    let mut tally = Tally::default();

    for seed in SEEDS {
        run_seed(seed, &mut tally);
    }

    let rate = tally.correct as f64 / tally.total() as f64;
    eprintln!("ACC-01: {:.2}% correct, {tally:?}", rate * 100.0);

    assert_eq!(tally.total(), SEEDS.len() * 8 * READS_PER_LEAF);
    assert!(rate >= 0.95, "accuracy {rate:.4} below 0.95: {tally:?}");
}
