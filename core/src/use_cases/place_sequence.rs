use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    kmers_map::KmersMap,
    placement_response::PlacementStatus::{self, *},
    tree::Tree,
};

use mycelium_base::{dtos::UntaggedParent, utils::errors::MappedErrors};

/// Place a sequence in the tree.
///
/// This function tries to place a sequence in the tree using the overlapping
/// kmers. The function uses a recursive strategy to traverse the tree and
/// evaluate the adherence of the query sequence to the clades.
pub fn place_sequence(
    sequence: String,
    tree: Tree,
    max_iterations: Option<i32>,
) -> Result<PlacementStatus, MappedErrors> {
    let max_iterations = max_iterations.unwrap_or(1000);

    // ? -----------------------------------------------------------------------
    // ? Build and validate query kmers
    // ? -----------------------------------------------------------------------

    let kmers_map = tree
        .kmers_map
        .to_owned()
        .expect("The tree does not have a kmers map.");

    let query_kmers = kmers_map.build_kmers_from_string(sequence, None);

    if query_kmers.is_empty() || query_kmers.len() == 1 {
        panic!("The sequence does not contain enough kmers.");
    }

    // ? -----------------------------------------------------------------------
    // ? Sub-sampling kmers_map from the query_kmers
    //
    // Here the kmers_map is sub-sampled to only contain the kmers present in
    // the query sequence. This is done to reduce the number of kmers to be
    // processed.
    //
    // ? -----------------------------------------------------------------------

    let query_kmers_map =
        kmers_map.get_overlapping_kmers(&query_kmers.into_iter().collect());

    // ? -----------------------------------------------------------------------
    // ? Try to place the sequence
    //
    // Recursive function to traverse the tree and try to place the query
    // sequence using the overlapping kmers.
    //
    // ? -----------------------------------------------------------------------

    let root_kmers = match query_kmers_map.get_kmers_with_node(tree.root.id) {
        Some(kmers) => kmers,
        None => {
            return Ok(Unclassifiable(
                "\
The query sequence does not match any kmers in the tree indicating that the \
sequence is not related to the phylogeny."
                    .to_string(),
            ))
        }
    };

    let mut children = if let Some(children) = tree.root.to_owned().children {
        children
    } else {
        panic!("The root node does not have children. This is unexpected.");
    };

    let mut iteration = 0;
    let mut clade = tree.root;

    loop {
        // ? -------------------------------------------------------------------
        // ? Increment and evaluate the iteration cycle
        // ? -------------------------------------------------------------------

        iteration += 1;
        if iteration > max_iterations {
            panic!("The maximum number of iterations has been reached.");
        }

        // ? -------------------------------------------------------------------
        // ? Start the placement process
        // ? -------------------------------------------------------------------

        //
        // The proposed clades list contains the adherence tests for each child
        // node. The vector should be used to determine the best clade to place
        // the query sequence at the current level of the tree.
        //
        let mut clade_proposals = Vec::<AdherenceTest>::new();

        for child in children.iter() {
            if child.is_leaf() {
                continue;
            }

            //
            // These container executes the one-vs-rest strategy to test the
            // query adherence to the current clade.
            //
            let (one, rest) = {
                //
                // Kmers contained in the child node representing the target
                // clade of the classification. Sucker kmers set should be used
                // to test against the sibling kmers set.
                //
                let child_kmers_map: KmersMap =
                    query_kmers_map.get_overlapping_kmers(&root_kmers);

                //
                // The sibling set represents the `rest` part of the
                // `one-vs-rest` strategy. The sibling kmers set should be used
                // to test against the sucker kmers set.
                //
                let sibling_clades: KmersMap = children
                    .iter()
                    .filter_map(|c| {
                        if c.id != child.id && !c.is_leaf() {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .filter_map(|clade| {
                        if let Some(kmers) =
                            query_kmers_map.get_kmers_with_node(clade.id)
                        {
                            Some((clade.id, kmers))
                        } else {
                            None
                        }
                    })
                    .fold(
                        KmersMap::new(kmers_map.get_k_size()),
                        |mut acc, c| {
                            let (node, kmers) = c;
                            for kmer in kmers.iter() {
                                acc.insert_or_append(
                                    kmer.clone(),
                                    [node].iter().cloned().collect(),
                                );
                            }

                            acc
                        },
                    );

                (child_kmers_map, sibling_clades)
            };

            if one.get_map().is_empty() {
                // TODO:
                // This is a special case where the query kmers do not
                // overlap with the child kmers. This should be handled
                // differently.
            }

            if rest.get_map().is_empty() {
                // TODO:
                // This is a special case where the query kmers do not
                // overlap with the sibling kmers. This should be handled
                // differently.
            }

            clade_proposals.push(AdherenceTest {
                clade: UntaggedParent::Record(child.to_owned()),
                one: one.get_map().keys().len() as i32,
                rest: rest.get_map().keys().len() as i32,
            });
        }

        //
        // Here are filtered the clades that have a higher adherence than the
        // sibling clades.
        //
        let filtered_proposals: Vec<AdherenceTest> = clade_proposals
            .iter()
            .filter_map(|adherence| {
                if adherence.one > adherence.rest {
                    Some(adherence.clone())
                } else {
                    None
                }
            })
            .collect();

        //
        // If none of the proposed clades have a higher adherence than the
        // sibling clades, the search are considered maxed out, than, the query
        // sequence is placed at the current clade.
        //
        if filtered_proposals.is_empty() {
            return Ok(MaxResolutionReached(clade.id));
        }

        //
        // If only one clade has a higher adherence than the sibling clades, the
        // query sequence is placed at the current clade.
        //
        if filtered_proposals.len() == 1 {
            let adherence: AdherenceTest = match filtered_proposals.first() {
                Some(adherence) => adherence.clone(),
                None => {
                    panic!("The filtered proposals list is empty. This is unexpected.");
                }
            };

            clade = match adherence.clade.to_owned() {
                UntaggedParent::Record(clade) => clade,
                UntaggedParent::Id(_) => {
                    panic!(
                        "The adherence test does not contain a clade record."
                    );
                }
            };

            children = match clade.to_owned().children {
                Some(children) => children,
                None => return Ok(IdentityFound(adherence)),
            };
        }

        //
        // If more than one clade has a higher adherence than the sibling
        // clades, the search is considered inconclusive. The query sequence is
        // placed at the current clade.
        //
        if filtered_proposals.len() > 1 {
            return Ok(Inconclusive(
                filtered_proposals
                    .iter()
                    .map(|adherence| AdherenceTest {
                        clade: match &adherence.clade {
                            UntaggedParent::Record(clade) => {
                                UntaggedParent::Id(clade.id.to_owned())
                            }
                            UntaggedParent::Id(id) => {
                                UntaggedParent::Id(id.to_owned())
                            }
                        },
                        one: adherence.one,
                        rest: adherence.rest,
                    })
                    .collect(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_place_sequence() {
        let path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/outputs/Colletotrichum_acutatum_gapdh-PhyML.yaml");

        // Load the tree from file
        let file = std::fs::File::open(path).unwrap();
        let tree: Tree = serde_yaml::from_reader(file).unwrap();

        // Col_orchidophilum
        let query_set = "CCTTCATTGAGACCAAGTACGCTGTGAGTATCACCCCACTTTACCCCTCCATGATGATATCACATCTGTCACGACAATACCAGCCTCATCGGCCACTGGGAAAGAAATGAGCTAGCACTCTCGATCCTGTGACCCAGGATACTGAAGCGGCTCGTCCCAATGGCATGATGTGA";

        // Col_nymphaeae_CBS_52677
        // let query_set = "CCTTCATTGAGACCAAGTACGCTGTGAGTATCACCCCACTTTACCCCTCCATCATGATATCACGTCTGCCACGATAACACCAGCTTCGTCGATATCCACGGGAAAAGAGTCGGAGCTAGCACTCTCAACTCTTTTGCCCCAAGGTTTCGATTGGGCTTGTTGTAACGACACGACGTGACACAATCATGCAGAAACAGCCGAGACAAAACTTGCTGACAGACAATCATCACAGGCCTACATGCTCAAGTAC";

        // A random sequence
        // let invalid_query = "ASDFASDFASDFASDFASDFADSF";

        match place_sequence(query_set.to_string(), tree, None) {
            Err(err) => panic!("Error: {err}"),
            Ok(response) => {
                println!("{:?}", serde_json::to_string(&response).unwrap());
            }
        }
    }
}
