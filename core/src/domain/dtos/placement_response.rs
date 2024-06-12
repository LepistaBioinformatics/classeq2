use self::PlacementStatus::*;
use super::adherence_test::AdherenceTest;

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged, rename_all = "camelCase")]
pub enum PlacementStatus {
    /// The query sequence may does not belong to the reference tree
    ///
    Unclassifiable,

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

impl ToString for PlacementStatus {
    fn to_string(&self) -> String {
        match self {
            Unclassifiable => "Unclassifiable".to_string(),
            IdentityFound(_) => "IdentityFound".to_string(),
            MaxResolutionReached(_) => "MaxResolutionReached".to_string(),
            NextIteration(_) => "NextIteration".to_string(),
            Inconclusive(_) => "Inconclusive".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlacementResponse<T> {
    query: String,
    code: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    placement: Option<T>,
}

impl<T> PlacementResponse<T> {
    pub fn new(query: String, code: String, placement: Option<T>) -> Self {
        PlacementResponse {
            query,
            code,
            placement,
        }
    }
}
