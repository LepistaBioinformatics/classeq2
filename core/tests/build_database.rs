mod common;

use classeq_core::{domain::dtos::tree::Tree, use_cases::map_kmers_to_tree};
use common::{synth, write_inputs};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tempfile::TempDir;

const K: u64 = 15;
const L: usize = 800;

fn interleaved(records: &[(String, String)]) -> Vec<(String, String)> {
    let order = ["A1", "C1", "B1", "D1", "A2", "C2", "B2", "D2"];
    order
        .iter()
        .map(|name| records.iter().find(|(n, _)| n == name).unwrap().clone())
        .collect()
}

fn build(
    newick: &str,
    fasta: &str,
    k: Option<u64>,
    m: Option<u64>,
    s: Option<f64>,
) -> Tree {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_inputs(&dir, newick, fasta);
    map_kmers_to_tree(tree_path, msa_path, k, m, s).unwrap()
}

fn build_default_dataset(
    seed: u64,
    fasta: impl Fn(&[(String, String)]) -> String,
) -> (Tree, Vec<(String, String)>) {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, seed);
    let tree = build(
        &synth::write_newick(&topology),
        &fasta(&records),
        Some(K),
        Some(4),
        None,
    );
    (tree, records)
}

fn leaf_paths(tree: &Tree) -> HashMap<String, (u64, HashSet<u64>)> {
    tree.root
        .get_leaves_with_paths(None)
        .into_iter()
        .map(|(clade, path)| {
            (clade.name.unwrap(), (clade.id, path.into_iter().collect()))
        })
        .collect()
}

fn flat_map(tree: &Tree) -> HashMap<u64, HashSet<u64>> {
    let mut flat = HashMap::new();
    for value in tree.kmers_map.as_ref().unwrap().get_map().values() {
        for (hash, nodes) in &value.0 {
            flat.entry(*hash)
                .or_insert_with(HashSet::new)
                .extend(nodes.iter().copied());
        }
    }
    flat
}

fn canonical(tree: &Tree) -> BTreeMap<u64, BTreeMap<u64, BTreeSet<u64>>> {
    tree.kmers_map
        .as_ref()
        .unwrap()
        .get_map()
        .iter()
        .map(|(key, value)| {
            (
                key.0,
                value
                    .0
                    .iter()
                    .map(|(h, n)| (*h, n.iter().copied().collect()))
                    .collect(),
            )
        })
        .collect()
}

fn hashes_of(tree: &Tree, seq: &str) -> HashSet<u64> {
    tree.kmers_map
        .as_ref()
        .unwrap()
        .build_kmer_from_string(seq.to_string(), None)
        .into_iter()
        .map(|(_, hash)| hash)
        .collect()
}

#[test]
fn build_assigns_each_sequence_kmers_to_its_own_leaf() {
    let (tree, records) = build_default_dataset(1, |r| {
        synth::write_fasta(&interleaved(r), 60, false, false)
    });
    let paths = leaf_paths(&tree);
    let flat = flat_map(&tree);

    for (name, seq) in &records {
        let (leaf_id, _) = paths[name];
        let hashes = hashes_of(&tree, seq);
        assert!(!hashes.is_empty());
        for hash in hashes {
            let nodes = flat
                .get(&hash)
                .unwrap_or_else(|| panic!("{name}: k-mer {hash} not indexed"));
            assert!(
                nodes.contains(&leaf_id),
                "{name}: k-mer {hash} not mapped to leaf {leaf_id}"
            );
        }
    }
}

#[test]
fn build_maps_kmers_to_leaf_and_all_ancestors_exactly() {
    let (tree, records) =
        build_default_dataset(2, |r| synth::write_fasta(r, 60, false, false));
    let paths = leaf_paths(&tree);

    let mut expected: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut owners: HashMap<u64, HashSet<String>> = HashMap::new();
    for (name, seq) in &records {
        for hash in hashes_of(&tree, seq) {
            expected
                .entry(hash)
                .or_default()
                .extend(paths[name].1.iter().copied());
            owners.entry(hash).or_default().insert(name.clone());
        }
    }

    assert_eq!(flat_map(&tree), expected);

    for (name, _) in &records {
        let (leaf_id, path) = &paths[name];
        assert!(path.contains(&0) && path.contains(leaf_id) && path.len() == 4);
        let unique = owners
            .iter()
            .find(|(_, o)| o.len() == 1 && o.contains(name))
            .map(|(hash, _)| *hash)
            .unwrap_or_else(|| panic!("{name} has no unique k-mer"));
        assert_eq!(&flat_map(&tree)[&unique], path);
    }
}

#[test]
fn build_indexes_all_records_including_first_and_last() {
    let (tree, records) = build_default_dataset(3, |r| {
        synth::write_fasta(&interleaved(r), 0, false, false)
    });
    let paths = leaf_paths(&tree);
    let indexed: HashSet<u64> =
        flat_map(&tree).into_values().flatten().collect();

    for (name, _) in &records {
        assert!(
            indexed.contains(&paths[name].0),
            "leaf {name} has no k-mers"
        );
    }
}

#[test]
fn build_strips_gaps_and_lowercase_and_breaks_kmers_at_n() {
    let topology = synth::default_topology();
    let newick = synth::write_newick(&topology);
    let records = synth::evolve(&topology, L, 4);

    let clean = build(
        &newick,
        &synth::write_fasta(&records, 60, false, false),
        Some(K),
        Some(4),
        None,
    );

    // BUG-045: an `N` breaks the k-mers that would span it. The map then
    // holds a strict subset of the clean one, with no new k-mer.
    let dirty_records: Vec<(String, String)> = records
        .iter()
        .map(|(name, seq)| {
            (
                name.clone(),
                format!("{}N{}", &seq[..100], &seq[100..]).to_lowercase(),
            )
        })
        .collect();
    let dirty = build(
        &newick,
        &synth::write_fasta(&dirty_records, 60, true, false),
        Some(K),
        Some(4),
        None,
    );

    let (dirty, clean) = (canonical(&dirty), canonical(&clean));
    let count = |map: &BTreeMap<u64, BTreeMap<u64, BTreeSet<u64>>>| {
        map.values().map(|bucket| bucket.len()).sum::<usize>()
    };

    assert!(count(&dirty) < count(&clean));

    for (key, bucket) in &dirty {
        for (hash, nodes) in bucket {
            assert!(nodes.is_subset(&clean[key][hash]));
        }
    }
}

/// BUG-045: RNA input (`U`) builds the same database as its DNA form.
#[test]
fn build_reads_u_as_t() {
    let topology = synth::default_topology();
    let newick = synth::write_newick(&topology);
    let records = synth::evolve(&topology, L, 4);

    let rna_records: Vec<(String, String)> = records
        .iter()
        .map(|(name, seq)| (name.clone(), seq.replace('T', "U")))
        .collect();

    let dna = build(
        &newick,
        &synth::write_fasta(&records, 60, false, false),
        Some(K),
        Some(4),
        None,
    );
    let rna = build(
        &newick,
        &synth::write_fasta(&rna_records, 60, false, false),
        Some(K),
        Some(4),
        None,
    );

    assert_eq!(canonical(&rna), canonical(&dna));
}

#[test]
fn build_uses_default_parameters() {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, 5);
    let tree = build(
        &synth::write_newick(&topology),
        &synth::write_fasta(&records, 60, false, false),
        None,
        None,
        None,
    );
    let map = tree.kmers_map.as_ref().unwrap();

    assert_eq!(map.get_kmer_size(), 35);
    assert_eq!(map.get_minimizer_size(), 4);
    assert_eq!(tree.min_branch_support, 70.0);
}

#[test]
fn build_stores_explicit_parameters() {
    let topology = synth::default_topology();
    let records = synth::evolve(&topology, L, 6);
    let tree = build(
        &synth::write_newick(&topology),
        &synth::write_fasta(&records, 60, false, false),
        Some(21),
        Some(0),
        Some(50.0),
    );
    let map = tree.kmers_map.as_ref().unwrap();

    assert_eq!(map.get_kmer_size(), 21);
    assert_eq!(map.get_minimizer_size(), 0);
    assert_eq!(tree.min_branch_support, 50.0);
    assert_eq!(map.get_map().len(), 1);
}

#[test]
#[should_panic(expected = "The tree file does not exist.")]
fn build_panics_on_missing_tree() {
    let dir = TempDir::new().unwrap();
    let (_, msa_path) = write_inputs(&dir, "", ">A\nACGT\n");
    let _ = map_kmers_to_tree(
        dir.path().join("missing.nwk"),
        msa_path,
        None,
        None,
        None,
    );
}

#[test]
#[should_panic(expected = "The MSA file does not exist.")]
fn build_panics_on_missing_msa() {
    let dir = TempDir::new().unwrap();
    let (tree_path, _) = write_inputs(
        &dir,
        &synth::write_newick(&synth::default_topology()),
        "",
    );
    let _ = map_kmers_to_tree(
        tree_path,
        dir.path().join("missing.fasta"),
        None,
        None,
        None,
    );
}

#[test]
#[should_panic(expected = "The sequence header does not match any tree leaf")]
fn build_panics_on_header_not_in_tree() {
    let topology = synth::default_topology();
    let mut records = synth::evolve(&topology, L, 7);
    records[3].0 = "Unknown".to_string();
    build(
        &synth::write_newick(&topology),
        &synth::write_fasta(&records, 60, false, false),
        Some(K),
        Some(4),
        None,
    );
}

#[test]
fn build_is_independent_of_thread_count() {
    let topology = synth::default_topology();
    let newick = synth::write_newick(&topology);
    let fasta =
        synth::write_fasta(&synth::evolve(&topology, L, 8), 60, false, false);

    let run = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                canonical(&build(&newick, &fasta, Some(K), Some(4), None))
            })
    };

    assert_eq!(run(1), run(4));
}

#[test]
fn build_accepts_crlf_msa() {
    let topology = synth::default_topology();
    let newick = synth::write_newick(&topology);
    let records = synth::evolve(&topology, L, 9);

    let lf = build(
        &newick,
        &synth::write_fasta(&records, 60, false, false),
        Some(K),
        Some(4),
        None,
    );
    let crlf = build(
        &newick,
        &synth::write_fasta(&records, 60, false, true),
        Some(K),
        Some(4),
        None,
    );

    assert_eq!(canonical(&crlf), canonical(&lf));
}
