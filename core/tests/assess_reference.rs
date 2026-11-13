mod common;

use classeq_core::{
    domain::dtos::{
        calibration::Calibration, clade::Clade, evidence_rule::EvidenceRule,
        file_or_stdin::FileOrStdin, output_format::OutputFormat, tree::Tree,
    },
    use_cases::{
        assess_tree, map_kmers_to_tree, place_sequences, AssessOptions,
        Outcome, Verdict,
    },
};
use common::{
    synth::{self, Topology},
    write_inputs,
};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use tempfile::TempDir;

const K: u64 = 15;
const L: usize = 1200;
const LEAVES: usize = 16;

/// The reference as `build-db` builds it, and the root of the same tree
/// with every branch kept.
fn build(topology: &Topology, records: &[(String, String)]) -> (Tree, Clade) {
    let dir = TempDir::new().unwrap();
    let (tree_path, msa_path) = write_inputs(
        &dir,
        &synth::write_newick(topology),
        &synth::write_fasta(records, 60, false, false),
    );

    let input = Tree::init_from_file(&tree_path, f64::NEG_INFINITY).unwrap();
    let tree =
        map_kmers_to_tree(tree_path, msa_path, Some(K), Some(4), None).unwrap();

    (tree, input.root)
}

/// `topology` without the leaves `names`. A node left with one child is
/// replaced by that child, with the higher support of the two.
fn without(topology: &Topology, names: &BTreeSet<String>) -> Option<Topology> {
    match topology {
        Topology::Leaf { name, length } => {
            (!names.contains(name)).then(|| synth::leaf(name, *length))
        }
        Topology::Node {
            children,
            support,
            length,
        } => {
            let mut kept = children
                .iter()
                .filter_map(|child| without(child, names))
                .collect::<Vec<Topology>>();

            match kept.len() {
                0 => None,
                // The two branches become one, as supported as the better
                // of them
                1 => kept.pop().map(|child| match child {
                    Topology::Node {
                        children,
                        support: below,
                        length,
                    } => synth::node(
                        children,
                        match (below, *support) {
                            (Some(a), Some(b)) => Some(a.max(b)),
                            (a, b) => a.or(b),
                        },
                        length,
                    ),
                    leaf => leaf,
                }),
                _ => Some(synth::node(kept, *support, *length)),
            }
        }
    }
}

/// `topology` with support 50 on every third internal node, so that
/// `build-db` collapses it.
fn weaken(topology: &Topology, count: &mut usize) -> Topology {
    match topology {
        Topology::Leaf { name, length } => synth::leaf(name, *length),
        Topology::Node {
            children,
            support,
            length,
        } => {
            *count += 1;
            let support = match *count % 3 {
                0 => support.map(|_| 50.0),
                _ => *support,
            };

            synth::node(
                children.iter().map(|child| weaken(child, count)).collect(),
                support,
                *length,
            )
        }
    }
}

fn leaf_names(clade: &Clade, names: &mut BTreeSet<String>) {
    match clade.children.as_deref() {
        Some(children) if !children.is_empty() => {
            children.iter().for_each(|child| leaf_names(child, names))
        }
        _ => {
            names.insert(clade.name.clone().unwrap());
        }
    }
}

/// The leaf names under each clade.
fn clade_leaves(root: &Clade) -> HashMap<u64, BTreeSet<String>> {
    fn visit(clade: &Clade, sets: &mut HashMap<u64, BTreeSet<String>>) {
        let mut names = BTreeSet::new();
        leaf_names(clade, &mut names);
        sets.insert(clade.id, names);

        for child in clade.children.as_deref().unwrap_or(&[]) {
            visit(child, sets);
        }
    }

    let mut sets = HashMap::new();
    visit(root, &mut sets);
    sets
}

/// The leaves of the clade where `cls place --evidence-rule` puts `sequence`
/// on `tree`; `None` when unclassifiable.
fn place(tree: &Tree, sequence: &str) -> Option<BTreeSet<String>> {
    let dir = TempDir::new().unwrap();
    let query_path = dir.path().join("query.fasta");
    std::fs::write(&query_path, format!(">query\n{sequence}\n")).unwrap();
    let out = dir.path().join("out");

    place_sequences(
        FileOrStdin::from_file(query_path.to_str().unwrap()),
        tree,
        &out,
        &None,
        &None,
        &false,
        &OutputFormat::Jsonl,
        &None,
        &Some(EvidenceRule::default()),
        &None,
    )
    .unwrap();

    let text = std::fs::read_to_string(out.with_extension("jsonl")).unwrap();
    let record: Value = serde_json::from_str(text.lines().next()?).unwrap();
    let placement = record.get("placement")?;

    match placement["status"].as_str() {
        Some("IdentityFound" | "MaxResolutionReached") => {
            let clade = &placement["clade"];
            let id = clade["id"].as_u64().or_else(|| clade.as_u64())?;
            clade_leaves(&tree.root).remove(&id)
        }
        _ => None,
    }
}

/// ASM-02: the simulated placement of each query is the placement of that
/// sequence on a database built without the missing clade. The second
/// topology has branches below the support threshold.
#[test]
fn assessment_matches_placement_on_a_reference_without_the_clade() {
    let mut compared = 0;
    let mut placed = 0;
    let mut clades = 0;

    for (seed, weak) in [(11, false), (12, true), (13, true)] {
        let topology = match weak {
            false => synth::yule(LEAVES, seed),
            true => weaken(&synth::yule(LEAVES, seed), &mut 0),
        };
        let records = synth::evolve(&topology, L, seed);
        let (tree, input_root) = build(&topology, &records);
        let sets = clade_leaves(&tree.root);
        let input_sets = clade_leaves(&input_root);

        let options = AssessOptions {
            reads_per_leaf: 0,
            ..AssessOptions::default()
        };
        let assessment =
            assess_tree(&tree, &input_root, &records, &options).unwrap();

        let mut references = HashMap::<u64, Tree>::new();

        for detail in &assessment.details {
            let missing = &input_sets[&detail.missing];

            let simulated = detail.clade.map(|clade| {
                sets[&clade].difference(missing).cloned().collect()
            });

            let reference =
                references.entry(detail.missing).or_insert_with(|| {
                    let kept = records
                        .iter()
                        .filter(|(name, _)| !missing.contains(name))
                        .cloned()
                        .collect::<Vec<_>>();

                    let (mut reference, _) =
                        build(&without(&topology, missing).unwrap(), &kept);

                    // The simulation uses the calibration of the whole
                    // reference (a documented difference), so the pruned
                    // reference gets the same values
                    let whole = tree.calibration.as_ref().unwrap();

                    let containment = clade_leaves(&reference.root)
                        .into_iter()
                        .filter_map(|(id, leaves)| {
                            sets.iter()
                                .filter(|(_, set)| {
                                    set.difference(missing).eq(leaves.iter())
                                })
                                .min_by_key(|(_, set)| set.len())
                                .and_then(|(full, _)| {
                                    whole.containment.get(full)
                                })
                                .map(|value| (id, *value))
                        })
                        .collect();

                    reference.calibration = Some(Calibration {
                        mean_leaf_kmers: whole.mean_leaf_kmers,
                        containment,
                        root_crossing: whole.root_crossing.clone(),
                    });

                    reference
                });

            let sequence = &records
                .iter()
                .find(|(name, _)| name == &detail.leaf)
                .unwrap()
                .1;
            let real = place(reference, sequence);

            assert_eq!(
                simulated, real,
                "seed {seed}, leaf {}, missing {missing:?}: {:?}",
                detail.leaf, detail.outcome
            );

            compared += 1;
            placed += usize::from(real.is_some());
            clades += usize::from(missing.len() > 1);
        }
    }

    assert!(compared > 3 * 2 * LEAVES, "{compared} compared");
    assert!(clades > 3 * LEAVES / 2, "{clades} from missing clades");
    assert!(placed * 2 > compared, "{placed} of {compared} placed");
}

/// The shares of each scenario add up to one, and a refusal has no clade.
#[test]
fn assessment_reports_shares_that_add_up_and_a_verdict() {
    let topology = weaken(&synth::yule(LEAVES, 21), &mut 0);
    let records = synth::evolve(&topology, L, 21);
    let (tree, input_root) = build(&topology, &records);

    let assessment =
        assess_tree(&tree, &input_root, &records, &AssessOptions::default())
            .unwrap();

    for scenario in [&assessment.missing_leaf, &assessment.missing_clade] {
        for shares in [&scenario.full_length, &scenario.reads] {
            let total = shares.refused
                + shares.under
                + shares.exact
                + shares.over
                + shares.wrong;

            assert!((total - 1.0).abs() < 1e-9, "{shares:?}");
            assert!(shares.exact <= shares.reachable + 1e-9, "{shares:?}");
            assert_eq!(
                shares.verdict,
                Verdict::of(shares.exact, shares.false_share(), shares.refused)
            );
        }
    }

    assert_eq!(assessment.leaves, LEAVES);
    assert_eq!(assessment.missing_leaf.full_length.queries, LEAVES);
    assert_eq!(assessment.missing_leaf.reads.queries, LEAVES * 5);
    assert!(assessment.missing_clade.full_length.queries > LEAVES);
    assert_eq!(assessment.k_size, K);

    for detail in &assessment.details {
        assert_eq!(
            detail.clade.is_none(),
            detail.outcome == Outcome::Refused,
            "{detail:?}"
        );
    }
}
