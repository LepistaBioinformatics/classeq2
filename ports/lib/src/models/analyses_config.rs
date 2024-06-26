use apalis::prelude::*;
use classeq_core::domain::dtos::output_format::OutputFormat;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

impl Message for BluAnalysisConfig {
    const NAME: &'static str = "watcher:blu-analysis";
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BluAnalysisConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub email: String,

    pub query_file_id: u32,

    pub model_id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,

    pub output_format: OutputFormat,

    pub work_dir: String,
}
