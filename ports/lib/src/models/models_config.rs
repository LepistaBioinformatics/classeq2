use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub id: Uuid,
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gene: Option<String>,

    #[serde(skip_serializing)]
    path: PathBuf,
}

impl ModelConfig {
    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
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
