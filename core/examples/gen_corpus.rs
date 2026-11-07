//! Seeded synthetic corpus for the placement benchmarks
//! (005-placement-performance, design D6).
//!
//! cargo run --release -p classeq-core --example gen_corpus -- \
//!     <leaves> <length> <seed> <out-dir> [queries=10000]
//!
//! Writes `ref.nwk`, `ref.fasta` and `queries.fasta` to `<out-dir>`. The same
//! arguments always give the same bytes.

#[path = "../tests/common/synth.rs"]
mod synth;

use std::{fs, path::PathBuf};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.len() < 4 {
        eprintln!(
            "usage: gen_corpus <leaves> <length> <seed> <out-dir> [queries]"
        );
        std::process::exit(2);
    }

    let leaves: usize = args[0].parse().expect("leaves");
    let length: usize = args[1].parse().expect("length");
    let seed: u64 = args[2].parse().expect("seed");
    let out = PathBuf::from(&args[3]);
    let count: usize =
        args.get(4).map_or(10_000, |n| n.parse().expect("queries"));

    let topology = synth::yule(leaves, seed);
    let records = synth::evolve(&topology, length, seed);
    let queries = synth::queries(&records, count, seed + 1);

    fs::create_dir_all(&out).expect("create the output directory");
    fs::write(out.join("ref.nwk"), synth::write_newick(&topology)).unwrap();
    fs::write(
        out.join("ref.fasta"),
        synth::write_fasta(&records, 0, false, false),
    )
    .unwrap();
    fs::write(
        out.join("queries.fasta"),
        synth::write_fasta(&queries, 0, false, false),
    )
    .unwrap();
}
