use classeq_core::{
    domain::dtos::{
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::{map_kmers_to_tree, place_sequences},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::PathBuf;

const TREE_PATH: &str =
    "src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh-PhyML.nwk";
const MSA_PATH: &str =
    "src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh_mafft.fasta";

fn build_test_tree() -> Tree {
    map_kmers_to_tree(
        PathBuf::from(TREE_PATH),
        PathBuf::from(MSA_PATH),
        Some(35),
        Some(4),
        Some(70.0),
    )
    .expect("Failed to build test tree")
}

fn bench_build_kmers(c: &mut Criterion) {
    let tree = build_test_tree();
    let kmers_map = tree.kmers_map.as_ref().unwrap();
    let sequence = "ATGGTCAAGGAGGACAAGTACGCTGTGAGTATCACCCCA\
                    CTTTACCCCTCCAATGATGATATCACATCTGTCACGAC"
        .to_string();

    c.bench_function("build_kmer_from_string k=35", |b| {
        b.iter(|| {
            black_box(kmers_map.build_kmer_from_string(sequence.clone(), None))
        })
    });
}

fn bench_full_placement(c: &mut Criterion) {
    let tree = build_test_tree();
    let out_file = PathBuf::from("/tmp/bench_placement_out");

    c.bench_function("place_sequences (all MSA leaves as queries)", |b| {
        b.iter(|| {
            black_box(
                place_sequences(
                    FileOrStdin::from_file(MSA_PATH),
                    &tree,
                    &out_file,
                    &None,
                    &None,
                    &true,
                    &OutputFormat::Jsonl,
                    &None,
                    &None,
                )
                .expect("Placement failed"),
            )
        })
    });
}

criterion_group!(benches, bench_build_kmers, bench_full_placement);
criterion_main!(benches);
