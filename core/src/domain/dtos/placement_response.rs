use self::PlacementStatus::*;
use super::{
    adherence_test::AdherenceTest, annotation::Annotation,
    evidence_rule::PlacementScores,
};

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Serialized as an object tagged by `status`:
///
/// ```yaml
/// status: IdentityFound
/// clade: 2
/// one: 10
/// rest: 3
/// ---
/// status: MaxResolutionReached
/// clade: 7
/// message: LCA Accepted
/// ---
/// status: Inconclusive
/// proposals:
/// - clade: 2
///   one: 5
///   rest: 1
/// message: Multiple proposals
/// ---
/// status: Unclassifiable
/// message: no kmers
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(into = "TaggedPlacementStatus", from = "TaggedPlacementStatus")]
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
    MaxResolutionReached(u64, String),

    /// The search was inconclusive, with more than one clade having the same
    /// maximum resolution
    ///
    Inconclusive(Vec<AdherenceTest>, String),
}

impl ToString for PlacementStatus {
    fn to_string(&self) -> String {
        match self {
            Unclassifiable(msg) => format!("Unclassifiable: {msg}"),
            IdentityFound(_) => "IdentityFound".to_string(),
            MaxResolutionReached(_, msg) => {
                format!("MaxResolutionReached: {msg}")
            }
            //NextIteration(_) => "NextIteration".to_string(),
            Inconclusive(_, msg) => format!("Inconclusive: {msg}"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
enum TaggedPlacementStatus {
    Unclassifiable {
        message: String,
    },
    IdentityFound(AdherenceTest),
    MaxResolutionReached {
        clade: u64,
        message: String,
    },
    Inconclusive {
        proposals: Vec<AdherenceTest>,
        message: String,
    },
}

impl From<PlacementStatus> for TaggedPlacementStatus {
    fn from(status: PlacementStatus) -> Self {
        match status {
            Unclassifiable(message) => Self::Unclassifiable { message },
            IdentityFound(test) => Self::IdentityFound(test),
            MaxResolutionReached(clade, message) => {
                Self::MaxResolutionReached { clade, message }
            }
            Inconclusive(proposals, message) => {
                Self::Inconclusive { proposals, message }
            }
        }
    }
}

impl From<TaggedPlacementStatus> for PlacementStatus {
    fn from(status: TaggedPlacementStatus) -> Self {
        match status {
            TaggedPlacementStatus::Unclassifiable { message } => {
                Unclassifiable(message)
            }
            TaggedPlacementStatus::IdentityFound(test) => IdentityFound(test),
            TaggedPlacementStatus::MaxResolutionReached { clade, message } => {
                MaxResolutionReached(clade, message)
            }
            TaggedPlacementStatus::Inconclusive { proposals, message } => {
                Inconclusive(proposals, message)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlacementResponse<T> {
    query: String,
    code: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    annotations: Option<Vec<Annotation>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    placement: Option<T>,

    /// Reported with the evidence rule only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scores: Option<PlacementScores>,
}

impl<T> PlacementResponse<T> {
    pub fn new(query: String, code: String, placement: Option<T>) -> Self {
        PlacementResponse {
            query,
            code,
            annotations: None,
            placement,
            scores: None,
        }
    }

    pub fn with_scores(mut self, scores: Option<PlacementScores>) -> Self {
        self.scores = scores;
        self
    }

    pub fn with_annotation(
        mut self,
        metadata: Option<Vec<Annotation>>,
    ) -> Self {
        self.annotations = metadata;
        self
    }

    pub fn placement(&self) -> Option<&T> {
        self.placement.as_ref()
    }
}
