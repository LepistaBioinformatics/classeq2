use super::build_database::map_spaced_kmers_to_tree;
use crate::domain::{
    dtos::{
        calibration::Calibration,
        clade::Clade,
        evidence_rule::{EvidenceRule, DEFAULT_CONTAINMENT_RATIO},
        kmer_index::query_kmers,
        kmers_map::{KmersMap, MinimizerKey},
        sequence::SequenceBody,
        tree::Tree,
    },
    rooting::RootingMethod,
};

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::BufRead,
    path::{Path, PathBuf},
};

/// What happens to a reference sequence placed as if its clade were missing
/// from the reference (010-reference-assessment).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Unclassifiable.
    Refused,

    /// A clade that contains the exact clade: correct, but shallow.
    Under,

    /// The clade the missing clade was attached to.
    Exact,

    /// Inside the exact clade: a false annotation.
    Over,

    /// Any other clade: a false annotation.
    Wrong,
}

/// Why the descent of a simulated query ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stop {
    /// Too few of the query's k-mers are in the reference.
    Coverage,

    /// The query shares less with the clade than the clade's own leaves.
    Novelty,

    /// No child holds enough k-mers specific to it.
    NoEvidence,

    /// The strongest sibling holds too many specific k-mers.
    Contested,

    /// The descent reached a leaf.
    Leaf,
}

/// The share of each outcome over a set of simulated queries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeShares {
    pub queries: usize,
    pub exact: f64,
    pub under: f64,
    pub over: f64,
    pub wrong: f64,
    pub refused: f64,

    /// The share of queries whose exact clade exists in the reference at
    /// the chosen branch support: the most `exact` could be.
    pub reachable: f64,

    /// The share of queries that stopped above their clade because no child
    /// held enough specific k-mers.
    pub under_no_evidence: f64,

    /// The share of queries that stopped above their clade because two
    /// children held specific k-mers.
    pub under_contested: f64,
}

impl OutcomeShares {
    /// The share of false annotations.
    pub fn false_share(&self) -> f64 {
        self.over + self.wrong
    }

    fn from_details<'a>(
        details: impl Iterator<Item = &'a QueryOutcome>,
    ) -> Self {
        let mut counts = [0usize; 5];
        let mut reachable = 0usize;
        let mut no_evidence = 0usize;
        let mut contested = 0usize;

        for detail in details {
            counts[detail.outcome as usize] += 1;
            reachable += usize::from(detail.reachable);

            if detail.outcome == Outcome::Under {
                no_evidence += usize::from(detail.stop == Stop::NoEvidence);
                contested += usize::from(detail.stop == Stop::Contested);
            }
        }

        let queries = counts.iter().sum::<usize>();
        let share = |count: usize| match queries {
            0 => 0.0,
            _ => count as f64 / queries as f64,
        };

        OutcomeShares {
            queries,
            exact: share(counts[Outcome::Exact as usize]),
            under: share(counts[Outcome::Under as usize]),
            over: share(counts[Outcome::Over as usize]),
            wrong: share(counts[Outcome::Wrong as usize]),
            refused: share(counts[Outcome::Refused as usize]),
            reachable: share(reachable),
            under_no_evidence: share(no_evidence),
            under_contested: share(contested),
        }
    }
}

/// How well the evidence rule is expected to work on a reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Few false annotations and a useful share of exact clades.
    Good,

    /// Few false annotations, but most queries stop above their clade.
    Shallow,

    /// Many false annotations or refusals.
    Poor,
}

/// The largest share of false annotations of a reference that is not `Poor`.
pub const MAX_FALSE_SHARE: f64 = 0.10;

/// The largest share of refusals of a reference that is not `Poor`.
pub const MAX_REFUSED_SHARE: f64 = 0.25;

/// The smallest share of exact clades of a `Good` reference.
pub const MIN_EXACT_SHARE: f64 = 0.20;

impl Verdict {
    /// The thresholds come from three datasets (010 report), so the verdict
    /// is an indication, not a guarantee.
    pub fn of(shares: &OutcomeShares) -> Self {
        if shares.false_share() > MAX_FALSE_SHARE
            || shares.refused > MAX_REFUSED_SHARE
        {
            Verdict::Poor
        } else if shares.exact < MIN_EXACT_SHARE {
            Verdict::Shallow
        } else {
            Verdict::Good
        }
    }
}

/// One simulated query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOutcome {
    pub leaf: String,

    /// The clade of the input tree taken as missing from the reference: the
    /// leaf itself or one of its ancestors.
    pub missing: u64,

    /// The number of leaves of that clade.
    pub missing_leaves: u32,

    /// The start of the read in the leaf's sequence; `None` for the
    /// full-length sequence.
    pub read_start: Option<usize>,

    pub outcome: Outcome,
    pub stop: Stop,

    /// The clade the query ended in; `None` when refused.
    pub clade: Option<u64>,

    /// Whether the exact clade exists at the chosen branch support.
    pub reachable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub full_length: OutcomeShares,
    pub reads: OutcomeShares,
}

impl Scenario {
    fn from_details<'a>(
        details: impl Iterator<Item = &'a QueryOutcome> + Clone,
    ) -> Self {
        Scenario {
            full_length: OutcomeShares::from_details(
                details.clone().filter(|d| d.read_start.is_none()),
            ),
            reads: OutcomeShares::from_details(
                details.filter(|d| d.read_start.is_some()),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub leaves: usize,
    pub k_size: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,

    pub min_branch_support: f64,
    pub read_length: usize,
    pub verdict: Verdict,

    /// Queries from a leaf missing from the reference: a new member of a
    /// known clade.
    pub missing_leaf: Scenario,

    /// Queries from a whole clade missing from the reference: a member of
    /// a new clade.
    pub missing_clade: Scenario,

    #[serde(skip)]
    pub details: Vec<QueryOutcome>,
}

#[derive(Clone, Debug)]
pub struct AssessOptions {
    pub rule: EvidenceRule,
    pub read_length: usize,
    pub reads_per_leaf: usize,

    /// The most leaves used as queries for each missing clade.
    pub leaves_per_clade: usize,

    /// Experimental: a spaced seed, a string of `1` (the position counts)
    /// and `0` (the position is ignored). It replaces the k-mer size.
    pub seed: Option<String>,

    /// Experimental: each specific k-mer counts as the share of the
    /// child's leaves that hold it, instead of one.
    pub weighted: bool,
}

impl Default for AssessOptions {
    fn default() -> Self {
        AssessOptions {
            rule: EvidenceRule::default(),
            read_length: 150,
            reads_per_leaf: 5,
            leaves_per_clade: 3,
            seed: None,
            weighted: false,
        }
    }
}

struct Node {
    /// The first and last leaf of the clade, in depth-first order.
    first: u32,
    last: u32,
    parent: Option<u64>,
    children: Vec<u64>,
}

impl Node {
    fn span(&self) -> (u32, u32) {
        (self.first, self.last)
    }

    fn holds(&self, (first, last): (u32, u32)) -> bool {
        self.first <= first && last <= self.last
    }

    fn within(&self, (first, last): (u32, u32)) -> bool {
        first <= self.first && self.last <= last
    }
}

/// A tree as spans of leaves in depth-first order. Collapsing low-support
/// branches keeps that order, so the spans of the input tree and of the
/// collapsed tree can be compared.
struct Layout {
    nodes: HashMap<u64, Node>,
    leaf_order: HashMap<u64, u32>,
    leaf_by_name: HashMap<String, u64>,
}

impl Layout {
    fn new(root: &Clade) -> Self {
        let mut layout = Layout {
            nodes: HashMap::new(),
            leaf_order: HashMap::new(),
            leaf_by_name: HashMap::new(),
        };

        layout.visit(root, None);
        layout
    }

    fn visit(&mut self, clade: &Clade, parent: Option<u64>) {
        let first = self.leaf_order.len() as u32;
        let children = clade.children.as_deref().unwrap_or(&[]);

        if children.is_empty() {
            self.leaf_order.insert(clade.id, first);

            if let Some(name) = &clade.name {
                self.leaf_by_name.insert(name.to_owned(), clade.id);
            }
        }

        for child in children {
            self.visit(child, Some(clade.id));
        }

        self.nodes.insert(
            clade.id,
            Node {
                first,
                last: (self.leaf_order.len() as u32).max(first + 1) - 1,
                parent,
                children: children.iter().map(|child| child.id).collect(),
            },
        );
    }
}

/// The reference leaves holding each distinct k-mer of a query, without the
/// leaves of the missing clade. K-mers held by no other leaf are left out.
struct Holders {
    leaves: Vec<u32>,
    bounds: Vec<(usize, usize)>,
}

impl Holders {
    fn find(
        map: &KmersMap,
        layout: &Layout,
        kmers: &[(u64, u64)],
        missing: (u32, u32),
    ) -> Self {
        let mut holders = Holders {
            leaves: Vec::new(),
            bounds: Vec::new(),
        };

        let mut pairs = kmers.to_vec();
        pairs.sort_unstable();
        pairs.dedup();

        for (key, hash) in pairs {
            let Some(nodes) = map
                .get_map()
                .get(&MinimizerKey(key))
                .and_then(|bucket| bucket.0.get(&hash))
            else {
                continue;
            };

            let start = holders.leaves.len();

            holders.leaves.extend(
                nodes
                    .iter()
                    .filter_map(|node| layout.leaf_order.get(node))
                    .filter(|leaf| **leaf < missing.0 || missing.1 < **leaf),
            );

            if holders.leaves.len() > start {
                holders.leaves[start..].sort_unstable();
                holders.bounds.push((start, holders.leaves.len()));
            }
        }

        holders
    }

    fn of(&self, kmer: usize) -> &[u32] {
        let (start, end) = self.bounds[kmer];
        &self.leaves[start..end]
    }

    /// Whether a leaf between `first` and `last` holds the k-mer.
    fn any_within(&self, kmer: usize, (first, last): (u32, u32)) -> bool {
        let leaves = self.of(kmer);
        let at = leaves.partition_point(|leaf| *leaf < first);
        leaves.get(at).is_some_and(|leaf| *leaf <= last)
    }
}

/// Place `sequence`, taken from the leaves `missing`, with the evidence rule
/// on the reference without those leaves. The steps follow `place_sequence`.
/// Returns `None` for a sequence with too few k-mers; otherwise the clade
/// (`None` for a refusal) and why the descent ended.
fn simulate(
    layout: &Layout,
    map: &KmersMap,
    calibration: Option<&Calibration>,
    root: u64,
    options: &AssessOptions,
    missing: (u32, u32),
    sequence: &str,
) -> Option<(Option<u64>, Stop)> {
    let rule = &options.rule;

    let kmers = match &options.seed {
        Some(seed) => KmersMap::spaced_kmers(sequence, seed)
            .into_iter()
            .map(|(kmer, hash)| (map.bucket_key(&kmer), hash))
            .collect(),
        None => {
            query_kmers(sequence, map.get_kmer_size(), map.get_minimizer_size())
        }
    };

    if kmers.len() < 2 {
        return None;
    }

    let distinct = kmers
        .iter()
        .map(|(_, hash)| *hash)
        .collect::<HashSet<u64>>()
        .len();

    let distinct = match calibration {
        Some(c) if c.mean_leaf_kmers > 0 => {
            distinct.min(c.mean_leaf_kmers as usize)
        }
        _ => distinct,
    };

    let containment_ratio = Some(rule.min_containment_ratio)
        .filter(|ratio| *ratio > 0.0)
        .unwrap_or(DEFAULT_CONTAINMENT_RATIO);

    let min_match_coverage = calibration
        .and_then(|c| c.containment_of(root))
        .map_or(0.7, |lowest| containment_ratio * lowest);

    let holders = Holders::find(map, layout, &kmers, missing);
    let mut alive = (0..holders.bounds.len()).collect::<Vec<usize>>();

    if alive.is_empty()
        || alive.len() < (distinct as f64 * min_match_coverage).round() as usize
    {
        return Some((None, Stop::Coverage));
    }

    let mut clade = root;
    let mut at_root = true;

    loop {
        let node = &layout.nodes[&clade];

        // Without the missing leaves, a clade left with one child is that
        // child: the descent is already in it
        let mut live = node
            .children
            .iter()
            .filter(|child| !layout.nodes[*child].within(missing));

        if let (Some(only), None) = (live.next(), live.next()) {
            let child = &layout.nodes[only];

            if child.children.is_empty() {
                return Some((Some(*only), Stop::Leaf));
            }

            alive.retain(|kmer| holders.any_within(*kmer, child.span()));
            clade = *only;
            continue;
        }

        let lowest = calibration
            .and_then(|c| c.containment_of(clade))
            .filter(|_| !at_root);

        if let Some(lowest) = lowest {
            let coverage = alive.len() as f64 / distinct as f64;

            if coverage < rule.min_containment_ratio * lowest {
                return Some((Some(clade), Stop::Novelty));
            }
        }

        let mut evidence = vec![0f64; node.children.len()];

        for kmer in &alive {
            let leaves = holders.of(*kmer);
            let span = (leaves[0], leaves[leaves.len() - 1]);

            if !node.holds(span) {
                continue;
            }

            let child = node
                .children
                .partition_point(|child| layout.nodes[child].last < span.0);

            let held = &layout.nodes[&node.children[child]];

            if held.holds(span) {
                evidence[child] += match options.weighted {
                    false => 1.0,
                    true => {
                        let size = (held.last - held.first + 1) as f64;
                        let gone = match held.holds(missing) {
                            true => (missing.1 - missing.0 + 1) as f64,
                            false => 0.0,
                        };

                        leaves.len() as f64 / (size - gone).max(1.0)
                    }
                };
            }
        }

        let (chosen, best) = evidence
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));

        let rival = evidence
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != chosen)
            .map(|(_, value)| *value)
            .fold(0.0, f64::max);

        let decisive = best >= rule.min_evidence as f64
            && best > 0.0
            && rival <= rule.max_rival_ratio * best;

        if !decisive {
            let stop = match best >= rule.min_evidence as f64 && best > 0.0 {
                true => Stop::Contested,
                false => Stop::NoEvidence,
            };

            return Some(((!at_root).then_some(clade), stop));
        }

        let chosen = node.children[chosen];
        let child = &layout.nodes[&chosen];

        alive.retain(|kmer| holders.any_within(*kmer, child.span()));

        if child.children.is_empty() {
            return Some((Some(chosen), Stop::Leaf));
        }

        clade = chosen;
        at_root = false;
    }
}

/// The evenly spaced starts of `count` reads of `read_length` in a sequence.
fn read_starts(length: usize, read_length: usize, count: usize) -> Vec<usize> {
    if length < read_length || count == 0 {
        return vec![];
    }

    let room = length - read_length;

    let mut starts = match count {
        1 => vec![room / 2],
        _ => (0..count).map(|i| i * room / (count - 1)).collect(),
    };

    starts.dedup();
    starts
}

/// Estimate how the evidence rule places sequences on a reference, from the
/// reference alone.
///
/// `tree` is the reference as `build-db` builds it: low-support branches
/// collapsed, with its k-mer map. `input_root` is the same tree without the
/// collapse; it says where each clade is attached. `records` are the
/// reference sequences by leaf name.
pub fn assess_tree(
    tree: &Tree,
    input_root: &Clade,
    records: &[(String, String)],
    options: &AssessOptions,
) -> Result<Assessment, MappedErrors> {
    let Some(map) = tree.kmers_map.as_ref() else {
        return use_case_err("The tree does not have a kmers map.").as_error();
    };

    let layout = Layout::new(&tree.root);
    let input = Layout::new(input_root);

    if layout.leaf_order != input.leaf_order {
        return use_case_err(
            "The input tree and the collapsed tree have different leaves.",
        )
        .as_error();
    }

    let spans = layout
        .nodes
        .values()
        .map(|node| node.span())
        .collect::<HashSet<(u32, u32)>>();

    let calibration = tree.calibration.as_ref();

    let details = records
        .par_iter()
        .filter_map(|(name, sequence)| {
            layout
                .leaf_by_name
                .get(name)
                .map(|leaf| (name, sequence, *leaf))
        })
        .flat_map_iter(|(name, sequence, leaf)| {
            let order = layout.leaf_order[&leaf];

            let reads = read_starts(
                sequence.len(),
                options.read_length,
                options.reads_per_leaf,
            );

            // The leaf itself, then each of its ancestors below the root
            // that takes this leaf as one of its queries
            let mut missing = vec![leaf];
            let mut ancestor = input.nodes[&leaf].parent;

            while let Some(clade) = ancestor {
                let node = &input.nodes[&clade];
                ancestor = node.parent;

                let picked = read_starts(
                    (node.last - node.first + 1) as usize,
                    1,
                    options.leaves_per_clade,
                )
                .contains(&((order - node.first) as usize));

                if node.parent.is_some() && picked {
                    missing.push(clade);
                }
            }

            let mut details = Vec::new();

            for missing in missing {
                let node = &input.nodes[&missing];

                let Some(parent) =
                    node.parent.map(|parent| &input.nodes[&parent])
                else {
                    continue;
                };

                // Without the missing clade, a parent of two children is
                // the other child
                let sibling = match parent.children[..] {
                    [a, b] => Some(if a == missing { b } else { a }),
                    _ => None,
                }
                .map(|sibling| input.nodes[&sibling].span());

                let reachable = spans.contains(&parent.span())
                    || sibling.is_some_and(|span| spans.contains(&span));

                for read_start in
                    std::iter::once(None).chain(reads.iter().copied().map(Some))
                {
                    let query = match read_start {
                        None => sequence.as_str(),
                        Some(start) => {
                            &sequence[start..start + options.read_length]
                        }
                    };

                    let Some((clade, stop)) = simulate(
                        &layout,
                        map,
                        calibration,
                        tree.root.id,
                        options,
                        node.span(),
                        query,
                    ) else {
                        continue;
                    };

                    let outcome = match clade.map(|id| &layout.nodes[&id]) {
                        None => Outcome::Refused,
                        Some(placed)
                            if placed.span() == parent.span()
                                || Some(placed.span()) == sibling =>
                        {
                            Outcome::Exact
                        }
                        Some(placed) if placed.holds(parent.span()) => {
                            Outcome::Under
                        }
                        Some(placed) if placed.within(parent.span()) => {
                            Outcome::Over
                        }
                        Some(_) => Outcome::Wrong,
                    };

                    details.push(QueryOutcome {
                        leaf: name.to_owned(),
                        missing,
                        missing_leaves: node.last - node.first + 1,
                        read_start,
                        outcome,
                        stop,
                        clade,
                        reachable,
                    });
                }
            }

            details
        })
        .collect::<Vec<QueryOutcome>>();

    let missing_leaf = Scenario::from_details(
        details.iter().filter(|detail| detail.missing_leaves == 1),
    );

    let missing_clade = Scenario::from_details(
        details.iter().filter(|detail| detail.missing_leaves > 1),
    );

    Ok(Assessment {
        leaves: missing_leaf.full_length.queries,
        k_size: map.get_kmer_size(),
        seed: options.seed.clone(),
        min_branch_support: tree.min_branch_support,
        read_length: options.read_length,
        verdict: Verdict::of(&missing_leaf.full_length),
        missing_leaf,
        missing_clade,
        details,
    })
}

/// Assess the reference that `build-db` would build from the same tree,
/// alignment and options. Nothing is written.
pub fn assess_reference(
    tree_path: PathBuf,
    msa_path: PathBuf,
    k_size: Option<u64>,
    m_size: Option<u64>,
    min_branch_support: Option<f64>,
    rooting: Option<RootingMethod>,
    options: &AssessOptions,
) -> Result<Assessment, MappedErrors> {
    let input = Tree::init_from_file_with_rooting(
        &tree_path,
        f64::NEG_INFINITY,
        rooting.as_ref(),
    )?;

    let records = read_reference_records(&msa_path)?;

    if let Some(seed) = &options.seed {
        let counted = seed.bytes().filter(|flag| *flag == b'1').count();

        if !seed.bytes().all(|flag| matches!(flag, b'0' | b'1'))
            || !seed.starts_with('1')
            || !seed.ends_with('1')
            || counted < 2
        {
            return use_case_err(
                "The seed pattern must be made of 1 and 0, start and end \
                with 1, and count at least two positions.",
            )
            .as_error();
        }
    }

    let tree = map_spaced_kmers_to_tree(
        tree_path,
        msa_path,
        k_size,
        m_size,
        min_branch_support,
        rooting,
        options.seed.as_deref(),
    )?;

    assess_tree(&tree, &input.root, &records, options)
}

/// The records of a FASTA file, cleaned as `build-db` cleans them.
pub fn read_reference_records(
    msa_path: &Path,
) -> Result<Vec<(String, String)>, MappedErrors> {
    let reader = match std::fs::File::open(msa_path) {
        Ok(file) => std::io::BufReader::new(file),
        Err(err) => {
            return use_case_err(format!(
                "The MSA file could not be opened: {err}"
            ))
            .as_error()
        }
    };

    let mut records = Vec::<(String, String)>::new();

    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                return use_case_err(format!(
                    "The MSA file could not be read: {err}"
                ))
                .as_error()
            }
        };

        if line.is_empty() {
            continue;
        }

        if line.starts_with('>') {
            records.push((line.replace('>', ""), String::new()));
        } else if let Some((_, sequence)) = records.last_mut() {
            sequence
                .push_str(&SequenceBody::remove_non_iupac_from_sequence(&line));
        }
    }

    Ok(records)
}
