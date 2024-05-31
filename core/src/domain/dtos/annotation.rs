use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// The clade ID to which the annotation belongs.
    pub clade: i32,

    /// The taxonomic ID of the annotation.
    pub tax_id: i32,

    /// The taxonomic rank of the annotation.
    pub rank: String,

    /// A simple list of tags associated with the annotation.
    pub tags: Vec<String>,
}
