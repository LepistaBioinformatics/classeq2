use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NodeType {
    Internal,
    Root,
    Terminal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clade {
    pub id: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub node_type: NodeType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub support: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Self>>,
}

impl Clade {
    pub(super) fn new_root(length: f64, children: Option<Vec<Clade>>) -> Clade {
        Clade {
            id: 0,
            name: None,
            node_type: NodeType::Root,
            support: None,
            length: Some(length),
            children,
        }
    }

    pub(super) fn new_leaf(
        id: i32,
        name: String,
        length: Option<f64>,
    ) -> Clade {
        Clade {
            id,
            name: Some(name),
            node_type: NodeType::Terminal,
            support: None,
            length,
            children: None,
        }
    }

    pub(super) fn new_internal(
        id: i32,
        name: Option<String>,
        support: Option<f64>,
        length: Option<f64>,
        children: Option<Vec<Clade>>,
    ) -> Clade {
        Clade {
            id,
            name,
            node_type: NodeType::Internal,
            support,
            length,
            children,
        }
    }

    pub fn is_root(&self) -> bool {
        if let NodeType::Root = self.node_type {
            true
        } else {
            false
        }
    }

    pub fn is_leaf(&self) -> bool {
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
