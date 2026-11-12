//! Placement micro-benchmark (005-placement-performance, MEAS-05).
//!
//! It goes through the public `place_sequences` on a seeded synthetic
//! database built in memory, with one rayon thread, so it measures the
//! per-query path and not the machine's core count.
//!
//! cargo bench -p classeq-core --bench placement

#[path = "../tests/common/synth.rs"]
mod synth;

use classeq_core::{
    domain::dtos::{
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::{map_kmers_to_tree, place_sequences},
};
use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::TempDir;

const LEAVES: usize = 200;
const LENGTH: usize = 1_500;
const QUERIES: usize = 100;

fn dataset(dir: &TempDir) -> (Tree, std::path::PathBuf) {
    let topology = synth::yule(LEAVES, 1);
    let records = synth::evolve(&topology, LENGTH, 1);
    let queries = synth::queries(&records, QUERIES, 2);

    let tree_path = dir.path().join("ref.nwk");
    let msa_path = dir.path().join("ref.fasta");
    let query_path = dir.path().join("queries.fasta");
    std::fs::write(&tree_path, synth::write_newick(&topology)).unwrap();
    std::fs::write(&msa_path, synth::write_fasta(&records, 0, false, false))
        .unwrap();
    std::fs::write(&query_path, synth::write_fasta(&queries, 0, false, false))
        .unwrap();

    let mut tree =
        map_kmers_to_tree(tree_path, msa_path, Some(35), Some(4), None)
            .unwrap();
    tree.freeze_kmers_map();

    (tree, query_path)
}

fn placement(c: &mut Criterion) {
    let dir = TempDir::new().unwrap();
    let (tree, query_path) = dataset(&dir);
    let out = dir.path().join("out");
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("place_sequences");
    group.sample_size(20);

    for (name, remove_intersection) in [("default", false), ("ri", true)] {
        group.bench_function(format!("yule{LEAVES}_{QUERIES}q_{name}"), |b| {
            b.iter(|| {
                pool.install(|| {
                    place_sequences(
                        FileOrStdin::from_file(query_path.to_str().unwrap()),
                        &tree,
                        &out,
                        &None,
                        &None,
                        &true,
                        &OutputFormat::Jsonl,
                        &Some(remove_intersection),
                        &None,
                        &None,
                    )
                    .unwrap()
                })
            })
        });
    }

    group.finish();
}

criterion_group!(benches, placement);
criterion_main!(benches);
