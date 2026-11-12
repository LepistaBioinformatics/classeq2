use serde::{Deserialize, Serialize};

/// The evidence placement rule (009-evidence-placement).
///
/// The default rule descends into the child with the most query k-mers, by
/// any margin. A query from a lineage missing from the reference then ends in
/// a clade nested inside its true position. Under this rule, the descent into
/// a child needs k-mers specific to that child: indexed under it, under none
/// of its siblings, and under no clade left behind on the way from the root.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRule {
    /// The fewest specific k-mers the chosen child must hold.
    pub min_evidence: u32,

    /// The largest ratio allowed between the specific k-mers of the strongest
    /// sibling and those of the chosen child.
    pub max_rival_ratio: f64,
}

impl Default for EvidenceRule {
    fn default() -> Self {
        EvidenceRule {
            min_evidence: 2,
            max_rival_ratio: 0.5,
        }
    }
}

/// How strongly a placement is supported, reported with the evidence rule.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementScores {
    /// The share of the query's distinct k-mers indexed under the placed
    /// clade.
    pub coverage: f64,

    /// The specific k-mers of the last child the descent entered.
    pub evidence: u32,

    /// The specific k-mers of that child's strongest sibling.
    pub rival: u32,
}
