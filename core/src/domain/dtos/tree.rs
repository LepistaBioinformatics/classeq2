use super::{annotation::Annotation, clade::Clade, kmers_map::KmersMap};

use mycelium_base::utils::errors::MappedErrors;
use phylotree::tree::Tree as PhyloTree;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fs::read_to_string, mem::size_of_val, path::Path};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    /// The unique identifier for the tree.
    ///
    /// The id is a unique identifier for the tree. The id is a Universally
    /// Unique Identifier (UUID) that is generated when the tree is created.
    pub id: Uuid,

    /// The human-readable name for the tree.
    ///
    /// When the tree is created using the from_file function, the name is set
    /// from the file name where the tree is parsed from.
    pub name: String,

    /// The in-memory size of the tree (in Mb).
    ///
    /// This is usual to predict the memory usage of the tree index.
    in_memory_size: Option<String>,

    /// The root Clade of the tree.
    ///
    /// The root is the root Clade of the tree. The root Clade is the starting
    /// point of the tree and contains the children nodes.
    pub root: Clade,

    /// The annotations associated with the tree.
    ///
    /// The annotations are the taxonomic annotations associated with nodes in
    /// the tree. The annotations are stored as a vector of Annotation objects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Annotation>>,

    //#[serde(skip_serializing_if = "Option::is_none")]
    pub kmers_map: Option<KmersMap>,
}

impl Tree {
    /// Create a new Tree object.
    ///
    /// The function creates a new Tree object with an id, name, and root Clade.
    /// The id is a unique identifier for the tree. The name is a human-readable
    /// name for the tree. The root is the root Clade of the tree.
    pub fn new(id: Uuid, name: String, root: Clade) -> Tree {
        Tree {
            id,
            name,
            in_memory_size: None,
            root,
            annotations: None,
            kmers_map: None,
        }
    }

    pub fn update_in_memory_size(&mut self) {
        let id_size = size_of_val(&self.id);

        let name_size = size_of_val(&self.name);

        let root_size = size_of_val(&self.root);

        let annotations_size = match &self.annotations {
            Some(annotations) => size_of_val(annotations),
            None => 0,
        };

        let kmers_map_size = match &self.kmers_map {
            Some(kmers_map) => size_of_val(kmers_map),
            None => 0,
        };

        self.in_memory_size = Some(format!(
            "{:.6} Mb",
            ((id_size +
                name_size +
                root_size +
                annotations_size +
                kmers_map_size) as f64 *
                0.000001) as f64
        ));
    }

    /// Pretty print the tree.
    ///
    /// The function prints the tree in a human-readable format. The function
    /// prints the root node and recursively prints each child node.
    pub fn pretty_print(&self) {
        println!("R: {}", self.name);

        for child in self.root.children.to_owned().unwrap() {
            self.pretty_print_clade(&child, 0);
        }
    }

    /// Pretty print a clade.
    ///
    /// The function prints a clade in a human-readable format. The function
    /// prints the clade id, name, and support value. If the clade is an
    /// internal node, the function prints the children nodes recursively.
    fn pretty_print_clade(&self, clade: &Clade, level: usize) {
        let mut indent = String::new();

        for _ in 0..level {
            indent.push_str("  ");
        }

        let id = clade.id;
        let name = clade.name.clone().unwrap_or("Unnamed".to_string());
        let support = clade.support.unwrap_or(-1.0);

        if clade.is_leaf() {
            println!("{}L:{}  {}", indent, id, name);
        } else {
            println!("{}I:{} ({})", indent, id, support);
        }

        if let Some(children) = clade.children.to_owned() {
            for child in children {
                self.pretty_print_clade(&child, level + 1);
            }
        }
    }

    pub fn from_yaml_file(file_path: &Path) -> Result<Tree, MappedErrors> {
        let file_content =
            read_to_string(file_path).expect("Could not read file");

        let tree: Tree =
            serde_yaml::from_str(&file_content).expect("Could not parse tree");

        Ok(tree)
    }

    /// Create a new Tree from a .newick file.
    ///
    /// The phylotre::tree::Tree is parsed from the file and converted to a Tree
    /// object with a root Clade.
    pub fn init_from_file(
        tree_path: &Path,
        min_branch_support: f64,
    ) -> Result<Tree, MappedErrors> {
        assert!(
            vec!["nwk", "newick", "tree"].contains(
                &tree_path
                    .extension()
                    .and_then(OsStr::to_str)
                    .expect("Could not get extension")
            ),
            "Tree file format is not supported"
        );

        let newick_content =
            read_to_string(tree_path).expect("Could not read file");

        let phylo_tree = PhyloTree::from_newick(&newick_content.as_str())
            .expect("Could not parse tree");

        let root_name = (if let Some(name) = tree_path.file_name() {
            Some(
                name.to_str()
                    .expect("Could not convert path to string")
                    .to_string(),
            )
        } else {
            None
        })
        .unwrap_or("UnnamedTree".to_string());

        if !phylo_tree.is_rooted().unwrap_or(false) {
            panic!("Tree is not rooted");
        }

        let root_tree = match phylo_tree.get_root() {
            Err(err) => panic!("Could not get root: {err}"),
            Ok(root) => phylo_tree.get(&root).expect("Could not get root"),
        };

        if !root_tree.is_root() {
            panic!("Root node is not a root");
        }

        let children = Self::get_children_nodes(&phylo_tree, &root_tree.id);

        let sanitized_root =
            Tree::sanitize(Clade::new_root(0.0, children), min_branch_support)?;

        let new_tree = Tree::new(
            Uuid::new_v3(&Uuid::NAMESPACE_DNS, &*root_name.as_bytes()),
            root_name,
            sanitized_root,
        );

        Ok(new_tree)
    }

    /// Remove low supported branches
    ///
    /// The function removes low supported branches from the tree, reconnecting
    /// children to the parent node.
    fn sanitize(
        clade: Clade,
        min_branch_support: f64,
    ) -> Result<Clade, MappedErrors> {
        let mut children = Vec::<Clade>::new();

        if let Some(clade_children) = clade.to_owned().children {
            for child in clade_children {
                let sanitized_child =
                    Self::sanitize(child, min_branch_support)?;

                if let Some(support) = sanitized_child.support {
                    if support >= min_branch_support ||
                        sanitized_child.is_leaf()
                    {
                        children.push(sanitized_child);
                    } else {
                        if let Some(grand_children) = sanitized_child.children {
                            for grand_child in grand_children {
                                children.push(grand_child);
                            }
                        }
                    }
                } else {
                    children.push(sanitized_child);
                }
            }
        }

        let mut sanitized_clade = clade.clone();

        sanitized_clade.children = match children.len() {
            0 => None,
            _ => Some(children),
        };

        Ok(sanitized_clade)
    }

    /// Recursively extract children nodes from a PhyloTree.
    ///
    /// The function recursively extracts children nodes from a PhyloTree and
    /// creates a Clade object for each node. The function returns a vector of
    /// Clade objects.
    fn get_children_nodes(
        tree: &PhyloTree,
        node_id: &usize,
    ) -> Option<Vec<Clade>> {
        if let Ok(node) = tree.get(node_id) {
            let mut children = Vec::<Clade>::new();

            //
            // Each child node is a single level below the parent node.
            //
            for child_id in node.children.to_owned() {
                //
                // Try to extract children node by node id.
                //
                let child_node =
                    tree.get(&child_id).expect("Child node not found");

                //
                // If the child node is a tip, create a new leaf Clade and
                // insert into the children vector.
                //
                if child_node.is_tip() {
                    let leaf_node = Clade::new_leaf(
                        child_node.id.try_into().expect("Could not convert id"),
                        child_node
                            .name
                            .clone()
                            .unwrap_or("Unnamed".to_string()),
                        child_node.parent_edge,
                    );
                    children.push(leaf_node);

                //
                // Otherwise, try to extract children nodes from the child node,
                // create a new internal Clade and insert into the children
                // vector.
                //
                } else {
                    let nested_children =
                        Self::get_children_nodes(tree, &child_id);

                    if let Some(nested_children) = nested_children {
                        let internal_node = Clade::new_internal(
                            child_node
                                .id
                                .try_into()
                                .expect("Could not convert id"),
                            None,
                            match child_node.name.clone() {
                                Some(name) => match name.parse::<f64>() {
                                    Err(_) => None,
                                    Ok(value) => Some(value),
                                },
                                None => None,
                            },
                            child_node.parent_edge,
                            Some(nested_children),
                        );

                        children.push(internal_node);
                    } else {
                        return None;
                    }
                }
            }

            return Some(children);
        }

        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_tree_from_file() {
        let path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh-PhyML.nwk");

        let tree = Tree::init_from_file(&path, 80.0);

        assert!(tree.is_ok());

        tree.unwrap().pretty_print();
    }
}
