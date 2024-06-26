use classeq_ports_lib::{FileSystemConfig, ModelsConfig};
use mycelium_base::utils::errors::{creation_err, MappedErrors};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    /// The address to bind the server to.
    pub address: String,

    /// The port to bind the server to.
    pub port: u16,

    /// The number of workers to use for the server.
    pub workers: Option<u16>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    pub fs: FileSystemConfig,
    pub server: ServerConfig,
    pub models: ModelsConfig,
}

impl ApiConfig {
    pub(crate) fn from_file(file: &PathBuf) -> Result<ApiConfig, MappedErrors> {
        let content = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(e) => return Err(creation_err(e)),
        };

        let config: ApiConfig = match serde_yaml::from_str(&content) {
            Ok(config) => config,
            Err(e) => return Err(creation_err(e)),
        };

        Ok(config)
    }
}
