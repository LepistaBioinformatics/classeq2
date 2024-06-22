use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemConfig {
    /// Default anonymous directory
    ///
    /// Anonymous directory path should be used when users are not identified.
    pub public_directory: String,

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
