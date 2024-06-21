use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{os::unix::fs::MetadataExt, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub accessed_at: Option<String>,
    pub size: u64,
}

impl Node {
    pub fn new(file: PathBuf, prefix: String) -> Self {
        let metadata = file.metadata().unwrap();
        let name = match file.strip_prefix(&prefix) {
            Ok(res) => res.to_str().unwrap().to_string(),
            Err(_) => file.to_str().unwrap().to_string(),
        };

        let is_file = metadata.is_file();
        let is_dir = metadata.is_dir();
        let is_symlink = metadata.file_type().is_symlink();

        let created_at = match metadata.created() {
            Ok(res) => Some(DateTime::<Utc>::from(res).to_rfc3339()),
            Err(_) => None,
        };

        let updated_at = match metadata.modified() {
            Ok(res) => Some(DateTime::<Utc>::from(res).to_rfc3339()),
            Err(_) => None,
        };

        let accessed_at = match metadata.accessed() {
            Ok(res) => Some(DateTime::<Utc>::from(res).to_rfc3339()),
            Err(_) => None,
        };

        Node {
            id: metadata.ino() as u32,
            name,
            is_file,
            is_dir,
            is_symlink,
            created_at,
            updated_at,
            accessed_at,
            size: metadata.size(),
        }
    }
}
