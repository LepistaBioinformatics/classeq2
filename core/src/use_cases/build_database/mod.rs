use crate::domain::{
    dtos::{
        calibration::Calibration, kmers_map::KmersMap, sequence::SequenceBody,
        tree::Tree,
    },
    rooting::RootingMethod,
};

use mycelium_base::utils::errors::MappedErrors;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{
    collections::HashSet,
    io::{BufRead, Write},
    path::PathBuf,
    sync::mpsc::channel,
    thread,
};
use tracing::debug;

/// Map kmers to nodes in a phylogenetic tree
///
/// Each kmer is mapped to a set of nodes in the tree. The set of nodes is the
/// path from the root to the leaf node that contains the kmer.
///
/// # Returns
/// A tree with the kmers map attached to it. A kmer map is a KmersMap struct
/// that contains a mapping of kmers to a set of nodes along the tree.
///
pub fn map_kmers_to_tree(
    tree_path: PathBuf,
    msa_path: PathBuf,
    k_size: Option<u64>,
    m_size: Option<u64>,
    min_branch_support: Option<f64>,
) -> Result<Tree, MappedErrors> {
    map_kmers_to_tree_with_rooting(
        tree_path,
        msa_path,
        k_size,
        m_size,
        min_branch_support,
        None,
    )
}

/// Same as [`map_kmers_to_tree`], rooting the reference tree in memory with
/// `rooting` when it is not rooted.
#[tracing::instrument(name = "Building Classeq database")]
pub fn map_kmers_to_tree_with_rooting(
    tree_path: PathBuf,
    msa_path: PathBuf,
    k_size: Option<u64>,
    m_size: Option<u64>,
    min_branch_support: Option<f64>,
    rooting: Option<RootingMethod>,
) -> Result<Tree, MappedErrors> {
    // ? -----------------------------------------------------------------------
    // ? Initialize and Validate arguments
    // ? -----------------------------------------------------------------------

    let k_size = k_size.unwrap_or(35);

    let m_size = m_size.unwrap_or(4);

    let min_branch_support = min_branch_support.unwrap_or(70.0);

    if !tree_path.exists() {
        panic!("The tree file does not exist.");
    }

    if !msa_path.exists() {
        panic!("The MSA file does not exist.");
    }

    // ? -----------------------------------------------------------------------
    // ? Read the phylogenetic tree
    // ? -----------------------------------------------------------------------

    debug!("Reading the phylogenetic tree");
    let mut tree = Tree::init_from_file_with_rooting(
        &tree_path,
        min_branch_support,
        rooting.as_ref(),
    )?;

    // ? -----------------------------------------------------------------------
    // ? Initialize mappings
    // ? -----------------------------------------------------------------------

    let mut map = KmersMap::new(k_size, m_size);
    let tree_leaves = tree.root.get_leaves_with_paths(None);

    // ? -----------------------------------------------------------------------
    // ? Read the MSA file and map the kmers to the tree
    //
    // Read MSA line by line collecting headers and kmers
    //
    // ? -----------------------------------------------------------------------

    debug!("Reading the MSA file");
    let mut header = String::new();
    let mut sequence = String::new();

    let reader = match std::fs::File::open(msa_path) {
        Err(err) => panic!("The MSA file could not be opened: {err}"),
        Ok(file) => std::io::BufReader::new(file),
    };

    let (sequence_sender, sequence_receiver) = channel();
    let (kmer_sender, kmer_receiver) = channel();

    let mut i = 0;
    for line in reader.lines() {
        let line = line.unwrap();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('>') {
            if !header.is_empty() {
                i += 1;
                print!("Build kmer for sequence {i}\r");
                std::io::stdout().flush().unwrap();

                let own_sender = sequence_sender.to_owned();
                let cloned_header = header.clone();
                let kmers = map.build_kmer_from_string(sequence.clone(), None);

                let _ = thread::spawn(move || {
                    match own_sender
                        .send((cloned_header.clone(), kmers.clone()))
                    {
                        Err(err) => panic!("Error: {err}"),
                        Ok(_) => (),
                    }
                });

                header.clear();
            }

            header.push_str(&line.replace(">", ""));
            sequence.clear();
        } else {
            sequence.push_str(
                SequenceBody::remove_non_iupac_from_sequence(&line).as_str(),
            );
        }
    }

    if !header.is_empty() {
        let kmers = map.build_kmer_from_string(sequence.clone(), None);
        sequence_sender
            .send((header.clone(), kmers))
            .expect("Error sending kmers to the receiver");
    }

    // Push the last line preventing losing the print value from the kmers map
    // loop which prints the sequence index
    println!();

    // Drop to allow the receiver to finish
    drop(sequence_sender);

    sequence_receiver
        .into_iter()
        .enumerate()
        .par_bridge()
        .for_each(|(i, (header, kmers))| {
            print!("Mapping kmers to nodes {index}\r", index = i + 1);
            std::io::stdout().flush().unwrap();

            let leaf_path = match tree_leaves.iter().find(|(clade, _)| {
                clade.name.as_ref().expect("The clade name is empty").to_owned() == header
            }) {
                None => {
                    panic!("The sequence header does not match any tree leaf: {header}")
                }
                Some((_, path)) => path,
            };

            for (kmer, hash) in kmers {
                kmer_sender
                    .send((leaf_path.clone(), kmer, hash))
                    .expect("Error sending kmer to the receiver");
            }
        });

    // Drop to allow the receiver to finish
    drop(kmer_sender);

    println!();

    for (i, (leaf_path, kmer, hash)) in kmer_receiver.into_iter().enumerate() {
        print!("Indexing kmer {index}\r", index = i + 1);
        std::io::stdout().flush().unwrap();

        map.insert_or_append_kmer_hash(
            kmer,
            hash,
            HashSet::from_iter(leaf_path.iter().cloned()),
        );
    }

    println!();

    // ? -----------------------------------------------------------------------
    // ? Return a positive response
    // ? -----------------------------------------------------------------------

    tree.calibration = Some(Calibration::measure(&tree.root, &map));
    tree.kmers_map = Some(map);

    Ok(tree)
}
