use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{os::unix::fs::MetadataExt, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub accessed_at: Option<String>,
    pub size: u64,
}

impl Node {
    pub fn new(file: PathBuf, prefix: String) -> Result<Self> {
        let metadata = file.metadata().unwrap();

        let file_str = (match file.to_str() {
            Some(res) => res,
            None => return Err(anyhow::anyhow!("Invalid file path")),
        })
        .split(&prefix)
        .collect::<Vec<&str>>()[0];

        let strip_prefix = format!("{}/{}/", file_str, prefix);

        let name = match file.strip_prefix(&strip_prefix) {
            Ok(res) => res.to_str().unwrap().to_string(),
            Err(_) => file.to_str().unwrap().to_string(),
        };

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

        Ok(Node {
            id: metadata.ino() as u32,
            name,
            created_at,
            updated_at,
            accessed_at,
            size: metadata.size(),
        })
    }
}
