use anyhow::Result;
use apalis::prelude::*;
use classeq_core::domain::dtos::{
    output_format::OutputFormat, rest_comp_strategy::RestComparisonStrategy,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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

    pub rest_comparison_strategy: RestComparisonStrategy,

    pub work_dir: String,
}

impl BluAnalysisConfig {
    pub fn from_yaml_file(file: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(file)?;
        let config: BluAnalysisConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
