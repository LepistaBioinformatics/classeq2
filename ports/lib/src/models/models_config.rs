use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    /// The model ID
    ///
    /// This is the same ID generated during the model building using the CLI
    /// tool.
    pub id: Uuid,

    /// The model name
    ///
    /// An arbitrary name given to the model a human-readable name.
    pub name: String,

    /// The gene name
    ///
    /// The gene name is the name of the gene used to build the phylogeny/model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gene: Option<String>,

    /// Path to the model file
    ///
    /// The serialization are skipped to avoid exposing the path during the API
    /// client responses.
    #[serde(skip_serializing)]
    model_path: PathBuf,

    /// Path to the annotations file
    ///
    /// The serialization are skipped to avoid exposing the path during the API
    /// client responses.
    #[serde(skip_serializing)]
    annotations_path: Option<PathBuf>,
}

impl ModelConfig {
    pub fn model_path(&self) -> PathBuf {
        self.model_path.clone()
    }

    pub fn annotations_path(&self) -> Option<PathBuf> {
        self.annotations_path.clone()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsConfig(pub Vec<ModelConfig>);

impl ModelsConfig {
    pub fn get_models(&self) -> Vec<ModelConfig> {
        self.0.clone()
    }
}
