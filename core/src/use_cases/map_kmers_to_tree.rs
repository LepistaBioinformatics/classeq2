use crate::domain::dtos::{
    kmers_map::KmersMap, sequence::SequenceBody, tree::Tree,
};

use mycelium_base::utils::errors::MappedErrors;
use std::{collections::HashSet, io::BufRead, path::PathBuf};
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
#[tracing::instrument(name = "Building Classeq database")]
pub fn map_kmers_to_tree(
    tree_path: PathBuf,
    msa_path: PathBuf,
    k_size: Option<usize>,
    m_size: Option<usize>,
    min_branch_support: Option<f64>,
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
    let mut tree = Tree::from_file(&tree_path, min_branch_support)?;

    // ? -----------------------------------------------------------------------
    // ? Initialize mappings
    // ? -----------------------------------------------------------------------

    let mut map = KmersMap::new(k_size, m_size);
    let tree_leaves = tree.root.get_leaves(None);

    // ? -----------------------------------------------------------------------
    // ? Read the MSA file and map the kmers to the tree
    //
    // Read MSA line by line collecting headers and kmers
    //
    // ? -----------------------------------------------------------------------

    debug!("Reading the MSA file");
    let mut headers = Vec::<String>::new();
    let mut header = String::new();
    let mut sequence = String::new();

    let reader = match std::fs::File::open(msa_path) {
        Err(err) => panic!("The MSA file could not be opened: {err}"),
        Ok(file) => std::io::BufReader::new(file),
    };

    for line in reader.lines() {
        let line = line.unwrap();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('>') {
            if !header.is_empty() {
                headers.push(header.clone());
                header.clear();
            }

            header.push_str(&line.replace(">", ""));

            let leaf_path = match tree_leaves.iter().find(|(clade, _)| {
                clade.name.as_ref().unwrap().to_owned() == header
            }) {
                None => {
                    panic!("The sequence header does not match any tree leaf: {header}")
                }
                Some((_, path)) => path,
            };

            let kmers = map.build_kmers_from_string(sequence.clone(), None);

            for kmer in kmers {
                map.insert_or_append_kmer_hash(
                    kmer,
                    HashSet::from_iter(leaf_path.iter().cloned()),
                );
            }

            sequence.clear();
        } else {
            sequence.push_str(
                SequenceBody::remove_non_iupac_from_sequence(&line).as_str(),
            );
        }
    }

    // ? -----------------------------------------------------------------------
    // ? Return a positive response
    // ? -----------------------------------------------------------------------

    tree.kmers_map = Some(map);
    tree.update_in_memory_size();

    Ok(tree)
}

#[cfg(test)]
mod tests {
    use crate::use_cases::map_kmers_to_tree;
    use mycelium_base::utils::errors::MappedErrors;
    use std::path::PathBuf;

    #[test]
    fn test_map_kmers_to_tree() -> Result<(), MappedErrors> {
        let tree_path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh-PhyML.nwk");
        let msa_path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh_mafft.fasta");

        let tree = map_kmers_to_tree(tree_path, msa_path, None, None, None)?;

        let content = match serde_yaml::to_string(&tree) {
            Err(err) => panic!("Error: {err}"),
            Ok(content) => content,
        };

        let path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/outputs/Colletotrichum_acutatum_gapdh-PhyML.yaml");

        if let Err(err) = std::fs::write(path.as_path(), content) {
            panic!("Error: {err}")
        }

        Ok(())
    }
}
