use anyhow::Result;
use classeq_ports_lib::{FileSystemConfig, ModelsConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchConfig {
    pub(crate) worker_name: String,
    pub(crate) workers: u32,
    pub(crate) interval: u64,
    pub(crate) retries: u32,
    pub(crate) max_threads_per_worker: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigFile {
    pub(crate) fs: FileSystemConfig,
    pub(crate) watcher: WatchConfig,
    pub(crate) models: ModelsConfig,
}

impl ConfigFile {
    pub(crate) fn from_file(file: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(file)?;
        let config: ConfigFile = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
