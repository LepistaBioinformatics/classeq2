use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    placement_response::PlacementStatus::{self, *},
    sequence::{SequenceBody, SequenceHeader},
    tree::Tree,
};

use mycelium_base::{
    dtos::UntaggedParent,
    utils::errors::{use_case_err, MappedErrors},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

/// Place a sequence in the tree.
///
/// This function tries to place a sequence in the tree using the overlapping
/// kmers. The function uses a recursive strategy to traverse the tree and
/// evaluate the adherence of the query sequence to the clades.
#[tracing::instrument(name = "Place a sequence", skip(sequence, tree))]
pub(super) fn place_sequence(
    header: &SequenceHeader,
    sequence: &SequenceBody,
    tree: &Tree,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
) -> Result<PlacementStatus, MappedErrors> {
    let max_iterations = max_iterations.unwrap_or(1000);

    let min_match_coverage = if let Some(value) = min_match_coverage {
        match value.to_owned() {
            value if value > 1.0 => 1.0,
            value if value < 0.0 => 0.0,
            value => value,
        }
    } else {
        0.7
    };

    let mut kmers_map = tree
        .kmers_map
        .to_owned()
        .expect("The tree does not have a kmers map.");

    // ? -----------------------------------------------------------------------
    // ? Build and validate query kmers
    // ? -----------------------------------------------------------------------

    debug!("Building kmers from the query sequence.");

    let query_kmers =
        kmers_map.build_kmers_from_string(sequence.seq().to_string(), None);

    if query_kmers.len() < 2 {
        return use_case_err("The sequence does not contain enough kmers.")
            .as_error();
    }

    // ? -----------------------------------------------------------------------
    // ? Sub-sampling kmers_map from the query_kmers
    //
    // Here the kmers_map is sub-sampled to only contain the kmers present in
    // the query sequence. This is done to reduce the number of kmers to be
    // processed.
    //
    // ? -----------------------------------------------------------------------

    debug!("Sub-sampling kmers map from the query kmers.");

    let mut query_kmers_map = kmers_map
        .get_overlapping_kmers(&query_kmers.to_owned().into_iter().collect());

    let query_kmers_len = query_kmers_map
        .get_map()
        .values()
        .into_iter()
        .map(|i| i.0.len())
        .sum::<usize>();

    if query_kmers_len == 0 {
        warn!("Query sequence may not be related to the phylogeny");
    }

    // ? -----------------------------------------------------------------------
    // ? Try to place the sequence
    //
    // Recursive function to traverse the tree and try to place the query
    // sequence using the overlapping kmers.
    //
    // ? -----------------------------------------------------------------------

    debug!("Starting the placement process.");

    let root_kmers = match query_kmers_map.get_kmers_with_node(tree.root.id) {
        None => return Ok(Unclassifiable),
        Some(kmers) => query_kmers_map
            .get_overlapping_kmers(&kmers.into_iter().map(|i| *i).collect()),
    };

    // ? -----------------------------------------------------------------------
    // ? Start the children clades with the root
    //
    // This object should be updated during the search process. The symbol 🌳
    // indicate wether this object is updated.
    //
    // ? -----------------------------------------------------------------------

    let mut children = if let Some(children) = tree.root.to_owned().children {
        children
    } else {
        return use_case_err(
            "The root node does not have children. This is unexpected.",
        )
        .as_error();
    };

    let mut iteration = 0;
    let mut clade = tree.root.to_owned();

    loop {
        // ? -------------------------------------------------------------------
        // ? Increment and evaluate the iteration cycle
        // ? -------------------------------------------------------------------

        iteration += 1;
        if iteration > max_iterations {
            return use_case_err(
                "The maximum number of iterations has been reached.",
            )
            .as_error();
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
                let child_kmers_map = match root_kmers
                    .to_owned()
                    .get_kmers_with_node(clade.to_owned().id)
                {
                    None => 0,
                    Some(kmers) => kmers.len(),
                };

                //
                // The sibling set represents the `rest` part of the
                // `one-vs-rest` strategy. The sibling kmers set should be used
                // to test against the sucker kmers set.
                //
                let sibling_clades: usize = children
                    .par_iter()
                    .filter_map(|c| {
                        if c.id != child.id && !c.is_leaf() {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .filter_map(|clade| {
                        root_kmers.get_kmers_with_node(clade.id)
                    })
                    .flatten()
                    .map(|i| *i)
                    .collect::<HashSet<u64>>()
                    .len();

                (child_kmers_map, sibling_clades)
            };

            //
            // This rule is used to determine if the child node has enough kmers
            // to be considered for the adherence test. If the child node does
            // not have enough kmers, the search process should perform a young
            // return with MaxResolutionReached status.
            //
            let expected_min_clade_coverage =
                query_kmers_len as f64 * min_match_coverage;

            if one < expected_min_clade_coverage as usize {
                debug!(
                    "Not enough kmers in node {child_id}.",
                    child_id = child.id
                );

                return Ok(MaxResolutionReached(clade.id));
            }

            clade_proposals.push(AdherenceTest {
                clade: UntaggedParent::Record(child.to_owned()),
                one: one as i32,
                rest: rest as i32,
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
                    Some(adherence.to_owned())
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
                Some(adherence) => adherence.to_owned(),
                None => {
                    return use_case_err("The filtered proposals list is empty. This is unexpected.").as_error();
                }
            };

            clade = match adherence.clade.to_owned() {
                UntaggedParent::Record(clade) => clade,
                UntaggedParent::Id(_) => {
                    return use_case_err(
                        "The adherence test does not contain a clade record.",
                    )
                    .as_error();
                }
            };

            //
            // 🌳 children update
            //
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
            let fold_proposals = filtered_proposals.iter().fold(
                HashMap::<i32, Vec<AdherenceTest>>::new(),
                |mut acc, a| {
                    acc.entry(a.one - a.rest)
                        .or_insert(vec![])
                        .push(a.to_owned());
                    acc
                },
            );

            let max_diff_key = fold_proposals.keys().max().unwrap();
            let max_diff_value = fold_proposals.get(max_diff_key).unwrap();

            if max_diff_value.len() == 1 {
                let adherence = max_diff_value.first().unwrap();

                clade = match adherence.clade.to_owned() {
                    UntaggedParent::Record(clade) => clade,
                    UntaggedParent::Id(_) => {
                        return use_case_err(
                            "The adherence test does not contain a clade record."
                        ).as_error();
                    }
                };

                //
                // 🌳 children update
                //
                children = match clade.to_owned().children {
                    Some(children) => children,
                    None => return Ok(IdentityFound(adherence.to_owned())),
                };

                continue;
            }

            return Ok(Inconclusive(
                filtered_proposals
                    .iter()
                    .map(|item| AdherenceTest {
                        clade: match &item.clade {
                            UntaggedParent::Record(clade) => {
                                UntaggedParent::Id(clade.id.to_owned())
                            }
                            id => id.to_owned(),
                        },
                        one: item.one,
                        rest: item.rest,
                    })
                    .collect(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dtos::sequence::Sequence;
    use std::path::PathBuf;

    #[test]
    fn test_place_sequence() {
        //let path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/outputs/Colletotrichum_acutatum_gapdh-PhyML.yaml");
        let path = PathBuf::from("/tmp/cls2.yaml");

        // Load the tree from file
        let file = std::fs::File::open(path).unwrap();
        let tree: Tree = serde_yaml::from_reader(file).unwrap();

        // Col_orchidophilum
        let query_sequence = Sequence::new(
            "Col_orchidophilum",
            "CCTTCATTGAGACCAAGTACGCTGTGAGTATCACCCCACTTTACCCCTCCATGATGATATCACATCTGTCACGACAATACCAGCCTCATCGGCCACTGGGAAAGAAATGAGCTAGCACTCTCGATCCTGTGACCCAGGATACTGAAGCGGCTCGTCCCAATGGCATGATGTGA",
        );

        // Col_laticiphilum_CBS_129827
        //let query_set = "\
        //CCTTCATTGAGACCAAGTACGCTGTGAGTATCACCCCACTTTACCCCTCCATCATGATAT\
        //CACGCCTACCACGATAACACCAGCTTCGTCGTTATCCACGGGGAAAAGAGTCAGAGCTAG\
        //CACTCTCGACTCTTTTGCCCCAAGGTTTCGATTGGGCTTGTTGTAATGAAACGACGTGAC\
        //ACAATCATGCAGAAACAGCCGAGACAAAATTTGCTGACAGACCATCCATCACAGGCCTAC\
        //ATGCTCAAGTAC";

        // A random sequence
        // let invalid_query = "ASDFASDFASDFASDFASDFADSF";

        match place_sequence(
            &query_sequence.header().to_owned(),
            &query_sequence.sequence().to_owned(),
            &tree,
            &None,
            &None,
        ) {
            Err(err) => panic!("Error: {err}"),
            Ok(response) => {
                println!("{:?}", serde_json::to_string(&response).unwrap());
            }
        }
    }
}
