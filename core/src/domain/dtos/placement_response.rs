use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::adherence_test::AdherenceTest;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlacementStatus {
    /// The query sequence may does not belong to the reference tree
    ///
    Unclassifiable(String),

    /// The query sequence was successfully placed on the reference tree with an
    /// absolute match
    ///
    IdentityFound(AdherenceTest),

    /// The query sequence was successfully placed on the reference tree but
    /// with no absolute match
    ///
    MaxResolutionReached(i32),

    /// An internal status used to indicate the search loop to go to the next
    /// clade
    ///
    NextIteration(i32),

    /// The search was inconclusive, with more than one clade having the same
    /// maximum resolution
    ///
    Inconclusive(Vec<AdherenceTest>),
}
