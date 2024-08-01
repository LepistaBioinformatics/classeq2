use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum NodeType {
    /// The root of the tree
    Root,

    /// An internal node
    Node,

    /// A terminal node
    Leaf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clade {
    pub id: u64,

    pub parent: Option<u64>,

    pub kind: NodeType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub support: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Self>>,
}

impl std::fmt::Display for Clade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Clade: {}", self.id)
    }
}

impl Clade {
    pub(super) fn new_root(length: f64, children: Option<Vec<Clade>>) -> Clade {
        Clade {
            id: 0,
            parent: None,
            name: None,
            kind: NodeType::Root,
            support: None,
            length: Some(length),
            children,
        }
    }

    pub(super) fn new_leaf(
        id: u64,
        parent_id: u64,
        name: String,
        length: Option<f64>,
    ) -> Clade {
        Clade {
            id,
            parent: Some(parent_id),
            name: Some(name),
            kind: NodeType::Leaf,
            support: None,
            length,
            children: None,
        }
    }

    pub(super) fn new_internal(
        id: u64,
        parent_id: u64,
        name: Option<String>,
        support: Option<f64>,
        length: Option<f64>,
        children: Option<Vec<Clade>>,
    ) -> Clade {
        Clade {
            id,
            parent: Some(parent_id),
            name,
            kind: NodeType::Node,
            support,
            length,
            children,
        }
    }

    pub fn get_node_by_id(&self, id: u64) -> Option<&Clade> {
        if self.id == id {
            return Some(self);
        }

        if let Some(children) = &self.children {
            for child in children {
                if let Some(node) = child.get_node_by_id(id) {
                    return Some(node);
                }
            }
        }

        None
    }

    pub fn get_path_to_root(&self, root: &Clade) -> HashSet<u64> {
        let mut path = HashSet::<u64>::new();

        path.insert(self.id);

        if let Some(parent_id) = self.parent {
            path.insert(parent_id);

            if let Some(parent) = root.get_node_by_id(parent_id) {
                path.extend(parent.get_path_to_root(root));
            }
        }

        path
    }

    pub fn get_leaves_with_paths(
        &self,
        parent_ids: Option<Vec<u64>>,
    ) -> Vec<(Clade, Vec<u64>)> {
        // Each entry contains the clade itself and the list of parent clade ids
        let mut leaves = Vec::<(Clade, Vec<u64>)>::new();

        // If the parent_ids is None, then the current clade is the root
        let parent_ids = match parent_ids {
            None => vec![self.id],
            Some(mut ids) => {
                ids.push(self.id);
                ids
            }
        };

        if self.is_leaf() {
            leaves.push((self.clone(), parent_ids));
        } else {
            if let Some(children) = &self.children {
                for child in children {
                    leaves.extend(
                        child.get_leaves_with_paths(Some(parent_ids.clone())),
                    );
                }
            }
        }

        leaves
    }

    pub fn is_root(&self) -> bool {
        if let NodeType::Root = self.kind {
            true
        } else {
            false
        }
    }

    pub fn is_leaf(&self) -> bool {
        if let NodeType::Leaf = self.kind {
            true
        } else {
            false
        }
    }

    pub fn is_internal(&self) -> bool {
        if let NodeType::Node = self.kind {
            true
        } else {
            false
        }
    }
}
