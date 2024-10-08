use super::{
    _dtos::IntrospectionUpdateResponse::*,
    update_introspection_node::update_introspection_node,
};
use crate::domain::dtos::{
    adherence_test::AdherenceTest,
    clade::Clade,
    placement_response::PlacementStatus::{self, *},
    sequence::{SequenceBody, SequenceHeader},
    telemetry_code::TelemetryCode,
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
use std::collections::{HashMap, HashSet};
use tracing::{debug_span, info, trace, trace_span, Span};

/// Place a sequence in the tree.
///
/// This function tries to place a sequence in the tree using the overlapping
/// kmers. The function uses a recursive strategy to traverse the tree and
/// evaluate the adherence of the query sequence to the clades.
#[tracing::instrument(
    name = "PlaceSingleSequence",
    skip_all,
    fields(
        query.kmers.count = tracing::field::Empty,
        query.kmers.treeMatches = tracing::field::Empty,
        query.kmers.buildTime = tracing::field::Empty,
        subject.kmers.queryMatches = tracing::field::Empty,
        subject.kmers.buildTime = tracing::field::Empty,
        subject.kmers.children = tracing::field::Empty,
    )
)]
pub(super) fn place_sequence(
    header: &SequenceHeader,
    sequence: &SequenceBody,
    tree: &Tree,
    max_iterations: &Option<i32>,
    min_match_coverage: &Option<f64>,
    remove_intersection: &Option<bool>,
    parent_span: &Option<&tracing::Span>,
) -> Result<PlacementStatus, MappedErrors> {
    // ? -----------------------------------------------------------------------
    // ? Configure the logging span
    // ? -----------------------------------------------------------------------

    if let Some(span) = parent_span {
        let span = trace_span!(parent: span.to_owned(), "PlaceSingleSequences");
        let _span_guard = span.enter();
    }

    // ? -----------------------------------------------------------------------
    // ? Start placement
    // ? -----------------------------------------------------------------------

    let remove_intersection = remove_intersection.unwrap_or(false);
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
        "query.kmers.buildTime",
        &Some(format!("{:?}", time.elapsed())),
    );

    if query_kmers.len() < 2 {
        return use_case_err("The sequence does not contain enough kmers.")
            .with_code(TelemetryCode::UCPLACE0005.to_string().as_str())
            .as_error();
    }

    trace!(
        code = TelemetryCode::UCPLACE0005.to_string(),
        "Query kmers built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Sub-sampling kmers_map from the query_kmers
    //
    // Here the kmers_map is sub-sampled to only contain the kmers present in
    // the query sequence. This is done to reduce the number of kmers to be
    // processed.
    //
    // ? -----------------------------------------------------------------------

    let query_kmers_map = kmers_map.get_overlapping_hashed_kmers(query_kmers);

    let query_kmers_len = query_kmers_map
        .get_map()
        .values()
        .par_bridge()
        .map(|i| i.0.len())
        .sum::<usize>();

    Span::current()
        .record("query.kmers.treeMatches", &Some(query_kmers_len as i32));

    if query_kmers_len == 0 {
        let msg = format!(
            "Query sequence {query:?} may not be related to the phylogeny",
            query = header
        );

        info!(code = TelemetryCode::UCPLACE0006.to_string(), msg);

        return Ok(Unclassifiable(msg));
    }

    trace!(
        code = TelemetryCode::UCPLACE0006.to_string(),
        "Query kmers map built successfully"
    );

    // ? -----------------------------------------------------------------------
    // ? Build the root kmers map
    //
    // The root kmers map is built using the kmers from the root node. This map
    // is used to evaluate the adherence of the query sequence to the root node.
    //
    // ? -----------------------------------------------------------------------

    let time = std::time::Instant::now();

    let introspection_kmers = match query_kmers_map
        .get_minimized_hashes_with_node(tree.root.id)
    {
        None => {
            let msg = "Query sequence has no overlapping kmers with the \
                reference tree";
            trace!(code = TelemetryCode::UCPLACE0007.to_string(), msg);
            return Ok(Unclassifiable(msg.to_string()));
        }
        Some(kmers) => query_kmers_map.get_overlapping_minimized_hashes(kmers),
    };

    Span::current().record(
        "subject.kmers.queryMatches",
        &Some(
            introspection_kmers
                .get_map()
                .into_iter()
                .map(|(_, v)| v.0.len() as i32)
                .sum::<i32>(),
        ),
    );

    Span::current().record(
        "subject.kmers.buildTime",
        &Some(format!("{:?}", time.elapsed())),
    );

    trace!(
        code = TelemetryCode::UCPLACE0007.to_string(),
        "Root kmers map built successfully"
    );

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
    // ? -----------------------------------------------------------------------

    let mut parent = tree.root.to_owned();

    Span::current()
        .record("subject.kmers.children", &Some(children.len() as i32));

    //
    // This rule is used to determine if the child node has enough kmers
    // to be considered for the adherence test. If the child node does
    // not have enough kmers, the search process should perform a young
    // return with `Unclassifiable` status.
    //
    let expected_min_clade_coverage =
        (query_kmers_len as f64 * min_match_coverage).round();

    trace!(
        code = TelemetryCode::UCPLACE0008.to_string(),
        "Expected min clade coverage (base {base}): {expected}",
        base = min_match_coverage,
        expected = expected_min_clade_coverage
    );

    let introspection_coverage = introspection_kmers
        .get_map()
        .into_par_iter()
        .map(|i| i.1 .0.len())
        .sum::<usize>();

    if introspection_coverage < expected_min_clade_coverage as usize {
        let msg =
            format!("Insufficient kmers coverage: {introspection_coverage}");

        trace!(code = TelemetryCode::UCPLACE0008.to_string(), msg);

        return Ok(Unclassifiable(msg));
    }

    // ? -----------------------------------------------------------------------
    // ? Fire the search loop
    //
    // Each iteration of the loop is a new introspection level. During the
    // search loop the algorithm try to place the sequence into a one or more
    // clades. Case the search loop reach the maximum number of iterations, the
    // search is considered maxed out and this function should return a
    // `use_case_err`.
    //
    // SYMBOLS:
    //  - 🍁: clade update
    //  - 🌿: children update
    //  - 🟢: introspect to the next tree level
    //  - 🔴: young return
    //  - ✅: conclusive identity found
    //
    // ? -----------------------------------------------------------------------

    trace!(
        code = TelemetryCode::UCPLACE0009.to_string(),
        "Starting tree introspection"
    );

    loop {
        iteration += 1;

        let iteration_span = debug_span!(
            "Introspection",
            code = TelemetryCode::UCPLACE0010.to_string(),
            level = iteration,
            clade_id = parent.id
        );

        let _iteration_span_guard = iteration_span.enter();

        // ? -------------------------------------------------------------------
        // ? Start the placement process
        // ? -------------------------------------------------------------------

        if iteration > max_iterations {
            return use_case_err(
                "The maximum number of iterations has been reached.",
            )
            .with_code(TelemetryCode::UCPLACE0010.to_string().as_str())
            .as_error();
        }

        // ? -------------------------------------------------------------------
        // ? PHASE 1: Generate clade proposals for the current level
        //
        // Clade proposals include all possible clades that a given query
        // sequence can be placed. The clade proposals are generated by
        // comparing the query sequence to the sibling clades.
        //
        // ? -------------------------------------------------------------------
        let clade_proposals = {
            let children_lengths_time = std::time::Instant::now();

            //
            // Aggregate children kmer lengths. This action is necessary to
            // determine the adherence of the query sequence to the sibling
            // clades.
            //
            let mut children_kmers = children
                .par_iter()
                .filter_map(|record| {
                    if record.is_leaf() {
                        return None;
                    }

                    match introspection_kmers
                        .get_hashed_kmers_with_node(record.id)
                    {
                        None => None,
                        Some(kmers) => Some((kmers, record)),
                    }
                })
                .collect::<Vec<(HashSet<u64>, &Clade)>>();

            children_kmers.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

            trace!(
                code = TelemetryCode::UCPLACE0012.to_string(),
                "Level clades (runtime {time}): {lengths}",
                time = format!("{:?}", children_lengths_time.elapsed()),
                lengths = children_kmers
                    .to_owned()
                    .iter()
                    .map(|(kmers, clade)| {
                        format!("{} ({})", clade.id, kmers.len())
                    })
                    .collect::<Vec<String>>()
                    .join(", ")
            );

            let clade_proposals_time = std::time::Instant::now();

            let clade_proposals = children_kmers
                .to_owned()
                .into_iter()
                .par_bridge()
                .filter_map(|(kmers, clade)| {
                    let rest: Vec<_> = children_kmers
                        .par_iter()
                        .filter_map(|(rest_kmers, nested_clade)| {
                            if nested_clade.id == clade.id {
                                return None;
                            }

                            Some(rest_kmers.to_owned())
                        })
                        .collect();

                    if rest.is_empty() {
                        return Some(AdherenceTest {
                            clade: UntaggedParent::Record(clade.to_owned()),
                            one: kmers.len() as i32,
                            rest: 0,
                        });
                    }

                    let rest_len = rest
                        .iter()
                        .map(|i| i.to_owned())
                        .flatten()
                        .collect::<HashSet<u64>>();

                    let (one_kmers, rest_kmers) = match remove_intersection {
                        true => (
                            kmers
                                .difference(&rest_len)
                                .map(|i| *i)
                                .collect::<HashSet<_>>(),
                            rest_len
                                .difference(&kmers)
                                .map(|i| *i)
                                .collect::<HashSet<_>>(),
                        ),
                        false => (kmers.to_owned(), rest_len.to_owned()),
                    };

                    trace!(
                        code = TelemetryCode::UCPLACE0013.to_string(),
                        "Clade {id}: one {one_kmers} vs rest {rest_kmers}",
                        id = clade.id,
                        one_kmers = one_kmers.len(),
                        rest_kmers = rest_kmers.len(),
                    );

                    Some(AdherenceTest {
                        clade: UntaggedParent::Record(clade.to_owned()),
                        one: one_kmers.len() as i32,
                        rest: rest_kmers.len() as i32,
                    })
                })
                .filter_map(|adherence| {
                    if adherence.one > adherence.rest as i32 {
                        Some(adherence.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<Vec<AdherenceTest>>();

            trace!(
                code = TelemetryCode::UCPLACE0014.to_string(),
                "Available proposals (runtime {time}): {proposals}",
                time = format!("{:?}", clade_proposals_time.elapsed()),
                proposals = clade_proposals.len()
            );

            clade_proposals
        };

        // ? -------------------------------------------------------------------
        // ? PHASE 2: Evaluate proposals
        //
        // The algorithm evaluates the proposals and decide the next step.
        //
        // ? -------------------------------------------------------------------
        {
            // ? ---------------------------------------------------------------
            // ? Case 1: No proposals
            // ? Events: 🔴
            //
            // If none of the proposed clades have a higher adherence than the
            // sibling clades, the search are considered maxed out, than, the
            // query sequence is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.is_empty() {
                if iteration == 1 {
                    let msg = "Tree introspection not possible. Query \
                        sequence has no overlapping kmers with the reference \
                        tree";
                    trace!(code = TelemetryCode::UCPLACE0011.to_string(), msg);
                    return Ok(Unclassifiable(msg.to_string()));
                }

                trace!(
                    code = TelemetryCode::UCPLACE0015.to_string(),
                    "No proposals found. Max resolution reached at clade {clade_id}",
                    clade_id = parent.id
                );

                return Ok(MaxResolutionReached(
                    parent.id,
                    "LCA Accepted".to_string(),
                ));
            }

            // ? ---------------------------------------------------------------
            // ? Case 2: One clade proposal
            // ? Events: 🟢 or ✅
            //
            // If only one clade has a higher adherence than the sibling clades,
            // the query sequence is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.len() == 1 {
                let adherence: AdherenceTest = match clade_proposals.first() {
                    Some(adherence) => adherence.to_owned(),
                    None => {
                        return use_case_err(
                            "The filtered proposals list is empty. This is \
                            unexpected.",
                        )
                        .as_error();
                    }
                };

                //
                // 🍁 1st clade update
                // 🌿 1st children update
                //
                (parent, children) =
                    match update_introspection_node(&adherence)? {
                        //
                        // ✅ Return a status if the search loop is finished with
                        // a conclusive identity.
                        //
                        Return(status) => {
                            return Ok(status);
                        }
                        //
                        // 🟢 Continue the search loop with the new parent and
                        // children
                        //
                        Continue(a, b) => (a, b),
                    };

                continue;
            }

            // ? ---------------------------------------------------------------
            // ? Case 3: No proposals
            // ? Events: ✅ or 🟢
            //
            // If more than one clade has a higher adherence than the sibling
            // clades, the search is considered inconclusive. The query sequence
            // is placed at the current clade.
            //
            // ? ---------------------------------------------------------------
            if clade_proposals.len() > 1 {
                trace!(
                    code = TelemetryCode::UCPLACE0018.to_string(),
                    "Multiple proposals found. Clade {parent} selected",
                    parent = parent.id
                );

                let fold_proposals = clade_proposals.iter().fold(
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
                    let adherence: &AdherenceTest = match max_diff_value.first()
                    {
                        Some(adherence) => adherence,
                        None => {
                            return use_case_err(
                                "The filtered proposals list is empty. This is \
                                unexpected."
                            ).as_error();
                        }
                    };

                    //
                    // 🍁 2nd clade update
                    // 🌿 2nd children update
                    //
                    (parent, children) =
                        match update_introspection_node(&adherence)? {
                            //
                            // ✅ Return a status if the search loop is finished
                            // with a conclusive identity.
                            //
                            Return(status) => {
                                return Ok(status);
                            }
                            //
                            // 🟢 Continue the search loop with the new parent
                            // and children
                            //
                            Continue(a, b) => (a, b),
                        };

                    continue;
                }

                //
                // 🔴 Case more than one proposals has the same probability,
                // return all proposals.
                //
                trace!(
                    code = TelemetryCode::UCPLACE0019.to_string(),
                    "Inconclusive identity found at clade",
                );

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
