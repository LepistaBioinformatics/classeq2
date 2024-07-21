use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    clade::Clade,
    placement_response::PlacementStatus::{self, *},
    rest_comp_strategy::RestComparisonStrategy,
    sequence::{SequenceBody, SequenceHeader},
    tree::Tree,
};

use mycelium_base::{
    dtos::UntaggedParent,
    utils::errors::{use_case_err, MappedErrors},
};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, ParallelBridge,
    ParallelIterator,
};
use std::collections::HashMap;
use tracing::{debug, trace, warn, Span};
use uuid::Uuid;

/// Place a sequence in the tree.
///
/// This function tries to place a sequence in the tree using the overlapping
/// kmers. The function uses a recursive strategy to traverse the tree and
/// evaluate the adherence of the query sequence to the clades.
#[tracing::instrument(
    name = "Place single sequence",
    skip_all,
    fields(
        id = Uuid::new_v3(
            &Uuid::NAMESPACE_DNS, header.header().as_bytes()
        ).to_string().replace("-", ""),
        query.name = header.header(),
        query.kmers.count = tracing::field::Empty,
        query.kmers.tree_matches = tracing::field::Empty,
        query.kmers.build_time = tracing::field::Empty,
        subject.kmers.query_matches = tracing::field::Empty,
        subject.kmers.build_time = tracing::field::Empty,
        subject.kmers.children = tracing::field::Empty,
    )
)]
pub(super) fn place_sequence(
    header: &SequenceHeader,
    sequence: &SequenceBody,
    tree: &Tree,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
    rest_comparison_strategy: &RestComparisonStrategy,
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

    let time = std::time::Instant::now();
    let query_kmers =
        kmers_map.build_kmer_from_string(sequence.seq().to_string(), None);

    Span::current()
        .record("query.kmers.count", &Some(query_kmers.len() as i32));

    Span::current().record(
        "query.kmers.build_time",
        &Some(format!("{:?}", time.elapsed())),
    );

    if query_kmers.len() < 2 {
        return use_case_err("The sequence does not contain enough kmers.")
            .as_error();
    }

    debug!("Query kmers built successfully");

    // ? -----------------------------------------------------------------------
    // ? Sub-sampling kmers_map from the query_kmers
    //
    // Here the kmers_map is sub-sampled to only contain the kmers present in
    // the query sequence. This is done to reduce the number of kmers to be
    // processed.
    //
    // ? -----------------------------------------------------------------------

    let mut query_kmers_map = kmers_map.get_overlapping_hashes(
        &query_kmers
            .to_owned()
            .into_par_iter()
            .map(|(_, hash)| hash)
            .collect(),
    );

    let query_kmers_len = query_kmers_map
        .get_map()
        .values()
        .par_bridge()
        .map(|i| i.0.len())
        .sum::<usize>();

    Span::current()
        .record("query.kmers.tree_matches", &Some(query_kmers_len as i32));

    if query_kmers_len == 0 {
        warn!("Query sequence may not be related to the phylogeny");
    }

    debug!("Query kmers map built successfully");

    // ? -----------------------------------------------------------------------
    // ? Try to place the sequence
    //
    // Recursive function to traverse the tree and try to place the query
    // sequence using the overlapping kmers.
    //
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();

    let root_kmers = match query_kmers_map.get_kmers_with_node(tree.root.id) {
        None => return Ok(Unclassifiable),
        Some(kmers) => query_kmers_map
            .get_overlapping_hashes(&kmers.into_iter().map(|i| *i).collect()),
    };

    Span::current().record(
        "subject.kmers.query_matches",
        &Some(
            root_kmers
                .get_map()
                .into_iter()
                .map(|(_, v)| v.0.len() as i32)
                .sum::<i32>(),
        ),
    );

    Span::current().record(
        "subject.kmers.build_time",
        &Some(format!("{:?}", time.elapsed())),
    );

    debug!("Root kmers map built successfully");

    // ? -----------------------------------------------------------------------
    // ? Start the children clades with the root
    //
    // Symbol: 🌿
    //
    // This object should be updated during the search process. The symbol 🌿
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

    // ? -----------------------------------------------------------------------
    // ? Set the initial parent
    //
    // Symbol: 🍁
    //
    // The clade object is used to store the current clade as a parent being
    // evaluated. The symbol 🍁 indicate wether this object is updated.
    //
    let mut parent = tree.root.to_owned();

    Span::current()
        .record("subject.kmers.children", &Some(children.len() as i32));

    debug!("Starting tree introspection");

    //
    // This rule is used to determine if the child node has enough kmers
    // to be considered for the adherence test. If the child node does
    // not have enough kmers, the search process should perform a young
    // return with MaxResolutionReached status.
    //
    let expected_min_clade_coverage =
        query_kmers_len as f64 * min_match_coverage;

    debug!(
        "Expected min clade coverage (base {base}): {expected}",
        base = min_match_coverage,
        expected = expected_min_clade_coverage
    );

    loop {
        // ? -------------------------------------------------------------------
        // ? Start the placement process
        // ? -------------------------------------------------------------------

        iteration += 1;
        if iteration > max_iterations {
            return use_case_err(
                "The maximum number of iterations has been reached.",
            )
            .as_error();
        }

        //
        // Aggregate children kmer lengths. This action is necessary to
        // determine the adherence of the query sequence to the sibling
        // clades.
        //
        let mut children_lenghts = children
            .par_iter()
            .filter_map(|record| {
                if record.is_leaf() {
                    return None;
                }

                match root_kmers.get_kmers_with_node(record.id) {
                    Some(kmers) => Some((record.id, kmers.len(), record)),
                    None => None,
                }
            })
            .collect::<Vec<(i32, usize, &Clade)>>();

        children_lenghts.sort_by(|a, b| b.1.cmp(&a.1));

        trace!(
            "Level clades: {lenghts}",
            lenghts = children_lenghts
                .to_owned()
                .iter()
                .map(|(id, len, _)| { format!("{} ({})", id, len) })
                .collect::<Vec<String>>()
                .join(", ")
        );

        let clade_proposals: Vec<AdherenceTest> = children_lenghts
            .to_owned()
            .into_iter()
            .par_bridge()
            .filter_map(|(child_id, len, clade)| {
                if len < expected_min_clade_coverage as usize {
                    trace!(
                        "Clade {clade_id} ignored with insuficient similarity {kmers_len}",
                        clade_id = child_id,
                        kmers_len = len
                    );

                    return None;
                };

                let rest: Vec<i32> = children_lenghts
                    .par_iter()
                    .filter(|(id, _, _)| *id != child_id)
                    .map(|(_, len, _)| *len as i32)
                    .collect();

                if rest.is_empty() {
                    return Some(
                        AdherenceTest {
                            clade: UntaggedParent::Record(clade.to_owned()),
                            one: len as i32,
                            rest_len: 0,
                            rest_avg: 0.0,
                            rest_max: 0,
                        },
                    );
                }

                let rest_len = rest.len() as i32;
                let rest_avg = (rest.iter().sum::<i32>() as f64 / rest_len as f64).round();
                let rest_max = rest.iter().max().unwrap().to_owned();

                trace!(
                    "Clade {id}: one {one_kmers} vs rest {rest_kmers}",
                    id = child_id,
                    one_kmers = len,
                    rest_kmers = match rest_comparison_strategy {
                        RestComparisonStrategy::Avg => rest_avg,
                        RestComparisonStrategy::Max => rest_max as f64,
                    }
                );

                Some(AdherenceTest {
                    clade: UntaggedParent::Record(clade.to_owned()),
                    one: len as i32,
                    rest_len,
                    rest_avg,
                    rest_max,
                })
            })
            .filter_map(|adherence| {
                let rest_value = match rest_comparison_strategy {
                    RestComparisonStrategy::Avg => adherence.rest_avg,
                    RestComparisonStrategy::Max => adherence.rest_max as f64,
                };

                if adherence.one > rest_value as i32 {
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
        if clade_proposals.is_empty() {
            return Ok(MaxResolutionReached(
                parent.id,
                "LCA Accepted".to_string(),
            ));
        }

        //
        // If only one clade has a higher adherence than the sibling clades, the
        // query sequence is placed at the current clade.
        //
        if clade_proposals.len() == 1 {
            let adherence: AdherenceTest = match clade_proposals.first() {
                Some(adherence) => adherence.to_owned(),
                None => {
                    return use_case_err(
                        "The filtered proposals list is empty. This is unexpected."
                    ).as_error();
                }
            };

            //
            // 🍁 clade update
            //
            parent = match adherence.clade.to_owned() {
                UntaggedParent::Record(record) => record,
                UntaggedParent::Id(_) => {
                    return use_case_err(
                        "The adherence test does not contain a clade record.",
                    )
                    .as_error();
                }
            };

            //
            // 🌿 children update
            //
            children = match parent.to_owned().children {
                Some(children) => children,
                None => return Ok(IdentityFound(adherence)),
            };
        }

        //
        // If more than one clade has a higher adherence than the sibling
        // clades, the search is considered inconclusive. The query sequence is
        // placed at the current clade.
        //
        if clade_proposals.len() > 1 {
            let fold_proposals = clade_proposals.iter().fold(
                HashMap::<i32, Vec<AdherenceTest>>::new(),
                |mut acc, a| {
                    let rest_value = match rest_comparison_strategy {
                        RestComparisonStrategy::Avg => a.rest_avg as i32,
                        RestComparisonStrategy::Max => a.rest_max,
                    };

                    acc.entry(a.one - rest_value)
                        .or_insert(vec![])
                        .push(a.to_owned());
                    acc
                },
            );

            let max_diff_key = fold_proposals.keys().max().unwrap();
            let max_diff_value = fold_proposals.get(max_diff_key).unwrap();

            if max_diff_value.len() == 1 {
                let adherence = max_diff_value.first().unwrap();

                //
                // 🍁 clade update
                //
                parent = match adherence.clade.to_owned() {
                    UntaggedParent::Record(record) => record,
                    UntaggedParent::Id(_) => {
                        return use_case_err(
                            "The adherence test does not contain a clade record."
                        ).as_error();
                    }
                };

                //
                // 🌿 children update
                //
                children = match parent.to_owned().children {
                    Some(children) => children,
                    None => return Ok(IdentityFound(adherence.to_owned())),
                };

                continue;
            }

            return Ok(Inconclusive(
                clade_proposals
                    .iter()
                    .map(|item| AdherenceTest {
                        clade: match &item.clade {
                            UntaggedParent::Record(clade) => {
                                UntaggedParent::Id(clade.id.to_owned())
                            }
                            id => id.to_owned(),
                        },
                        ..item.to_owned()
                    })
                    .collect(),
                "Multiple proposals".to_string(),
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
            &RestComparisonStrategy::Avg,
        ) {
            Err(err) => panic!("Error: {err}"),
            Ok(response) => {
                println!("{:?}", serde_json::to_string(&response).unwrap());
            }
        }
    }
}
