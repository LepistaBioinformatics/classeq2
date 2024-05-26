use super::{annotation::Annotation, clade_kmers::CladeKmers};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum NodeType {
    Internal,
    Root,
    Terminal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Clade {
    pub id: Uuid,
    pub name: String,
    pub node_type: NodeType,
    pub support: Option<f64>,
    pub length: Option<f64>,
    pub parent: Option<Uuid>,
    pub children: Option<Vec<Self>>,
    pub annotation: Option<Annotation>,
    pub kmers: Option<CladeKmers>,
}

impl Clade {
    pub fn is_root(&self) -> bool {
        if let NodeType::Root = self.node_type {
            true
        } else {
            false
        }
    }

    pub fn is_terminal(&self) -> bool {
        if let NodeType::Terminal = self.node_type {
            true
        } else {
            false
        }
    }

    pub fn is_internal(&self) -> bool {
        if let NodeType::Internal = self.node_type {
            true
        } else {
            false
        }
    }
}
