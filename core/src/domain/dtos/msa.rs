use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Msa;

impl Msa {
    pub fn new() -> Self {
        Msa
    }
}
