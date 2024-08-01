use self::PlacementStatus::*;
use super::{adherence_test::AdherenceTest, annotation::Annotation};

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged, rename_all = "camelCase")]
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

impl Serialize for PlacementStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MaxResolutionReached(id, _) => {
                serializer.serialize_u64(id.to_owned())
            }
            IdentityFound(adherence_test) => {
                adherence_test.serialize(serializer)
            }
            _ => serializer.serialize_str(&self.to_string()),
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
}

impl<T> PlacementResponse<T> {
    pub fn new(query: String, code: String, placement: Option<T>) -> Self {
        PlacementResponse {
            query,
            code,
            annotations: None,
            placement,
        }
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
