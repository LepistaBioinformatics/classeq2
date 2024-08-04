use anyhow::Result;
use apalis::prelude::*;
use classeq_core::domain::dtos::output_format::OutputFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

impl Message for PlacementConfig {
    const NAME: &'static str = "watcher:blu-analysis";
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub email: String,

    pub query_file_id: u32,

    pub model_id: Uuid,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_intersection: Option<bool>,

    pub output_format: OutputFormat,

    pub work_dir: String,
}

impl PlacementConfig {
    pub fn from_yaml_file(file: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(file)?;
        let config: PlacementConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
