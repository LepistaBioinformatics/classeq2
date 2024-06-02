use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::adherence_test::AdherenceTest;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlacementStatus {
    Unclassifiable(String),
    IdentityFound(AdherenceTest),
    MaxResolutionReached(i32),
    NextIteration(i32),
    Inconclusive(Vec<AdherenceTest>),
}
