use myc_config::load_config_from_file;
use mycelium_base::utils::errors::{creation_err, MappedErrors};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemConfig {
    /// Default anonymous directory
    ///
    /// Anonymous directory path should be used when users are not identified.
    pub anonymous_directory: String,

    /// The directory to be used for storing the database.
    pub serve_directory: String,

    /// The directory to be used for storing source data from users.
    pub input_directory: String,

    /// The directory to be used for storing output data from users.
    pub output_directory: String,

    /// The name of the configuration file generated to store analysis
    /// configurations.
    pub config_file_name: String,
}

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeConfig {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableTreesConfig {
    pub trees: Vec<TreeConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConfig {
    pub fs: FileSystemConfig,
    pub server: ServerConfig,
    pub available_trees: AvailableTreesConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TmpConfig {
    api: ApiConfig,
}

impl ApiConfig {
    pub fn from_default_config_file(
        file: PathBuf,
    ) -> Result<Self, MappedErrors> {
        if !file.exists() {
            return creation_err(format!(
                "Could not find config file: {}",
                file.to_str().unwrap()
            ))
            .as_error();
        }

        match load_config_from_file::<TmpConfig>(file) {
            Ok(config) => Ok(config.api),
            Err(err) => Err(err),
        }
    }
}
