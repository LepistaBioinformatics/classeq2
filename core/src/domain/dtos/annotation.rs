use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Annotation {
    /// The clade ID to which the annotation belongs.
    pub clade: Uuid,

    /// The taxonomic ID of the annotation.
    pub tax_id: i32,

    /// The taxonomic rank of the annotation.
    pub related_rank: String,
}
