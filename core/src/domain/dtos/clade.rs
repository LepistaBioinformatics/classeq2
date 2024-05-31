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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clade {
    pub id: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    pub kind: NodeType,

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
            kind: NodeType::Root,
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
            kind: NodeType::Leaf,
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
            kind: NodeType::Node,
            support,
            length,
            children,
        }
    }

    pub fn get_leaves(
        &self,
        parent_ids: Option<Vec<i32>>,
    ) -> Vec<(Clade, Vec<i32>)> {
        // Each entry contains the clade itself and the list of parent clade ids
        let mut leaves = Vec::<(Clade, Vec<i32>)>::new();

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
            for child in self.children.to_owned().unwrap() {
                leaves.extend(child.get_leaves(Some(parent_ids.clone())));
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
