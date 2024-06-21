use classeq_core::domain::dtos::output_format::OutputFormat;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BluAnalysisConfig {
    pub(crate) name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,

    pub(crate) email: String,

    pub(crate) query_file_id: u32,

    pub(crate) tree_id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) overwrite: Option<bool>,

    pub(crate) output_format: OutputFormat,
}
