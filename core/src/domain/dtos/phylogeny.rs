use super::clade::NodeType;

use phylotree::tree::Tree;
use std::{ffi::OsStr, fs::read_to_string, path::Path, str::FromStr};

type Error = Box<dyn std::error::Error>;

pub type Result<T> = std::result::Result<T, Error>;

/// A phylogeny representing a .newick file.
#[derive(PartialEq, Debug)]
pub struct Phylogeny {
    /// The name of the current node.
    ///
    /// Can be empty for internal nodes.
    name: Option<String>,

    /// The length of the branch leading to the current node.
    branch_length: f32,

    /// The support of the current node.
    branch_support: Option<f32>,

    /// The type of the current node.
    branch_type: NodeType,

    /// The children of the current node.
    ///
    /// Empty for leafs, and distances to the parent are optional.
    children: Option<Vec<Phylogeny>>,
}

impl Phylogeny {
    /// Create a new phylogeny node from a name and list of children.
    fn new(
        name: Option<String>,
        branch_length: f32,
        branch_support: Option<f32>,
        branch_type: NodeType,
        children: Option<Vec<Phylogeny>>,
    ) -> Phylogeny {
        Phylogeny {
            name,
            branch_length,
            branch_support,
            branch_type,
            children,
        }
    }

    fn set_branch_length(&mut self, branch_length: f32) {
        self.branch_length = branch_length;
    }

    fn set_branch_support(&mut self, branch_support: Option<f32>) {
        self.branch_support = branch_support;
    }

    /// Create a new leaf node.
    fn new_leaf(name: String, branch_length: f32) -> Phylogeny {
        Phylogeny::new(Some(name), branch_length, None, NodeType::Terminal, None)
    }

    /// Create a new internal node.
    fn new_internal(
        branch_length: f32,
        support: Option<f32>,
        children: Option<Vec<Phylogeny>>,
    ) -> Phylogeny {
        Phylogeny::new(None, branch_length, support, NodeType::Internal, children)
    }

    /// Create a new root node.
    fn new_root(
        branch_length: f32,
        branch_support: Option<f32>,
        children: Option<Vec<Phylogeny>>,
    ) -> Phylogeny {
        Phylogeny::new(
            None,
            branch_length,
            branch_support,
            NodeType::Root,
            children,
        )
    }

    /// Read a `.newick` file into a Phylogeny.
    pub fn from_file(p: &Path) {
        assert!(p.extension() == Some(OsStr::new("nwk")));
        //read_to_string(p)?.parse()

        //let str_path = p.to_str().expect("Could not convert path to string");
        let newick_content = read_to_string(p).expect("Could not read file");

        let tree = Tree::from_newick(&newick_content.as_str()).expect("Could not parse tree");
        //println!("tree: {:?}", tree);

        let root = match tree.get_root() {
            Err(err) => {
                println!("Could not get root: {:?}", err);
                return;
            }
            Ok(root) => root,
        };

        tree.get_descendants(&root).iter().for_each(|descendants| {
            println!("node: {:?}", descendants);

            descendants.iter().for_each(|node| {
                let children = tree.get_descendants(node);
                println!("child: {:?}", children);

                let named_node = tree.get_by_name(node.to_string().as_str());
                println!("named_node: {:?}", named_node);
            });
        });

        tree.get_leaf_names().iter().for_each(|leaf| {
            println!("leaf: {:?}", leaf);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    #[test]
    fn test_read_from_file() {
        let path = PathBuf::from("src/tests/data/colletotrichum-acutatom-complex/inputs/Colletotrichum_acutatum_gapdh-PhyML.nwk");
        let response = Phylogeny::from_file(path.as_path());

        println!("response: {:?}", response);
    }
}
