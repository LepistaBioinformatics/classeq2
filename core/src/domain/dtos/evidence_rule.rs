use serde::{Deserialize, Serialize};

/// The ratio of a clade's calibrated containment that a query must reach. It
/// also sets the default match coverage of a calibrated database.
pub const DEFAULT_CONTAINMENT_RATIO: f64 = 0.5;

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

    /// With a calibrated database: the descent enters the children of a
    /// clade only when the share of the query's k-mers under that clade is
    /// at least this ratio of the clade's own lowest leaf share
    /// (`Calibration::containment`). Zero turns the test off.
    pub min_containment_ratio: f64,
}

impl Default for EvidenceRule {
    fn default() -> Self {
        EvidenceRule {
            min_evidence: 2,
            max_rival_ratio: 0.5,
            min_containment_ratio: DEFAULT_CONTAINMENT_RATIO,
        }
    }
}

/// How strongly a placement is supported, reported with the evidence rule.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementScores {
    /// The share of the query's distinct k-mers indexed under the placed
    /// clade. With a calibrated database, a query with more k-mers than a
    /// typical leaf is measured against the leaf's count.
    pub coverage: f64,

    /// The specific k-mers of the last child the descent entered.
    pub evidence: u32,

    /// The specific k-mers of that child's strongest sibling.
    pub rival: u32,
}
