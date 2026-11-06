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

/// Outcome of each read relative to its source leaf.
#[derive(Default, Debug)]
struct Tally {
    /// Placed at the source leaf itself.
    leaf: usize,
    /// Placed at the source leaf's parent clade.
    parent: usize,
    /// `Inconclusive` with proposals that include the source leaf or its
    /// parent clade.
    inconclusive: usize,
    /// Placed at a higher ancestor of the source leaf (not the root).
    ancestor: usize,
    /// Placed outside the source leaf's path, or `Inconclusive` without it.
    wrong: usize,
    unplaced: usize,
}

impl Tally {
    fn total(&self) -> usize {
        self.leaf
            + self.parent
            + self.inconclusive
            + self.ancestor
            + self.wrong
            + self.unplaced
    }

    /// Placed at the source leaf or its parent clade.
    fn correct(&self) -> usize {
        self.leaf + self.parent
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

enum Placed {
    Clade(u64),
    Inconclusive(HashSet<u64>),
    None,
}

fn clade_id(value: &Value) -> Option<u64> {
    value["id"].as_u64().or_else(|| value.as_u64())
}

fn placed(record: Option<&Value>) -> Placed {
    let Some(placement) = record.and_then(|r| r.get("placement")) else {
        return Placed::None;
    };
    match placement["status"].as_str() {
        Some("IdentityFound" | "MaxResolutionReached") => {
            clade_id(&placement["clade"]).map_or(Placed::None, Placed::Clade)
        }
        Some("Inconclusive") => Placed::Inconclusive(
            placement["proposals"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|p| clade_id(&p["clade"]))
                .collect(),
        ),
        _ => Placed::None,
    }
}

/// `((A1,A2),C)` and `(D,(B1,B2))`: leaves next to internal clades, the
/// shape where leaf siblings used to be skipped (BUG-027).
fn mixed_topology() -> synth::Topology {
    let cherry = |p: &str| {
        synth::node(
            vec![
                synth::leaf(&format!("{p}1"), 0.01),
                synth::leaf(&format!("{p}2"), 0.01),
            ],
            Some(99.0),
            0.08,
        )
    };
    synth::node(
        vec![
            synth::node(
                vec![cherry("A"), synth::leaf("C", 0.09)],
                Some(99.0),
                0.08,
            ),
            synth::node(
                vec![synth::leaf("D", 0.09), cherry("B")],
                Some(99.0),
                0.08,
            ),
        ],
        None,
        0.0,
    )
}

fn run_seed(topology: &synth::Topology, seed: u64, tally: &mut Tally) {
    let dir = TempDir::new().unwrap();
    let records = synth::evolve(topology, L, seed);
    let (tree_path, msa_path) = write_inputs(
        &dir,
        &synth::write_newick(topology),
        &synth::write_fasta(&records, 60, false, false),
    );
    let tree =
        map_kmers_to_tree(tree_path, msa_path, Some(K), Some(4), None).unwrap();

    let leaves: HashMap<String, (u64, u64, HashSet<u64>)> = tree
        .root
        .get_leaves_with_paths(None)
        .into_iter()
        .map(|(clade, path)| {
            (
                clade.name.unwrap(),
                (clade.id, clade.parent.unwrap(), path.into_iter().collect()),
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
        let (leaf_id, parent_id, path) = &leaves[leaf];
        match self::placed(placed.get(query)) {
            Placed::Clade(id) if id == *leaf_id => tally.leaf += 1,
            Placed::Clade(id) if id == *parent_id => tally.parent += 1,
            Placed::Clade(id) if id != tree.root.id && path.contains(&id) => {
                tally.ancestor += 1
            }
            Placed::Clade(_) => tally.wrong += 1,
            Placed::Inconclusive(ids)
                if ids.contains(leaf_id) || ids.contains(parent_id) =>
            {
                tally.inconclusive += 1
            }
            Placed::Inconclusive(_) => tally.wrong += 1,
            Placed::None => tally.unplaced += 1,
        }
    }
}

fn report(label: &str, tally: &Tally) -> f64 {
    let rate = tally.correct() as f64 / tally.total() as f64;
    eprintln!(
        "{label}: {:.2}% at leaf or parent clade, {tally:?}",
        rate * 100.0
    );
    rate
}

#[test]
fn accuracy_mutated_reads_are_placed_in_their_source_clade() {
    let mut tally = Tally::default();

    for seed in SEEDS {
        run_seed(&synth::default_topology(), seed, &mut tally);
    }

    let rate = report("ACC-01", &tally);

    assert_eq!(tally.total(), SEEDS.len() * 8 * READS_PER_LEAF);
    assert!(rate >= 0.95, "accuracy {rate:.4} below 0.95: {tally:?}");
}

#[test]
fn accuracy_reads_from_leaves_next_to_clades() {
    let mut tally = Tally::default();

    for seed in SEEDS {
        run_seed(&mixed_topology(), seed, &mut tally);
    }

    let rate = report("ACC-02", &tally);

    assert_eq!(tally.total(), SEEDS.len() * 6 * READS_PER_LEAF);
    // Before 003-placement-algorithm every read from C and D was misplaced
    // (rate 0.67, BUG-027).
    assert!(rate >= 0.95, "accuracy {rate:.4} below 0.95: {tally:?}");
    assert_eq!(tally.wrong, 0, "{tally:?}");
}
