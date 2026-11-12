use super::{
    annotation::Annotation, calibration::Calibration, clade::Clade,
    kmer_index::KmerIndex,
    kmers_map::KmersMap, telemetry_code::TelemetryCode,
};
use crate::domain::rooting::{
    parse_newick, root_newick, split_rooting_annotation, RootingFlag,
    RootingMethod,
};

use mycelium_base::utils::errors::{use_case_err, MappedErrors};
use phylotree::tree::Tree as PhyloTree;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fs::read_to_string, path::Path, sync::Arc};
use tracing::{info, warn};
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

    /// The minimum branch support value.
    ///
    /// The minimum branch support value is the minimum support value for a
    /// branch to be included in the tree. Branches with support values below
    /// this value are removed from the tree and children are reconnected to the
    /// parent node.
    pub min_branch_support: f64,

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

    /// What the reference says about itself, measured at `build-db`.
    /// Databases built before 009-evidence-placement have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<Calibration>,

    /// The k-mers as built, or as read from a legacy database. Loaders
    /// freeze it into `index` and drop it.
    //#[serde(skip_serializing_if = "Option::is_none")]
    pub kmers_map: Option<KmersMap>,

    /// The index placement reads (005-placement-performance, design D2).
    #[serde(skip)]
    pub index: Option<Arc<KmerIndex>>,
}

impl Tree {
    /// Create a new Tree object.
    ///
    /// The function creates a new Tree object with an id, name, and root Clade.
    /// The id is a unique identifier for the tree. The name is a human-readable
    /// name for the tree. The root is the root Clade of the tree.
    pub fn new(
        id: Uuid,
        name: String,
        min_branch_support: f64,
        root: Clade,
    ) -> Tree {
        Tree {
            id,
            name,
            min_branch_support,
            root,
            annotations: None,
            calibration: None,
            kmers_map: None,
            index: None,
        }
    }

    /// Freeze `kmers_map` into `index` and drop the map, so the k-mers are
    /// not held twice.
    pub fn freeze_kmers_map(&mut self) {
        if let Some(map) = self.kmers_map.take() {
            self.index = Some(Arc::new(KmerIndex::freeze(&map)));
        }
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

    /// YAML parser options for databases. The default parser budget is
    /// sized for configuration files and rejects real databases, so it is
    /// disabled here.
    pub fn yaml_options() -> serde_saphyr::Options {
        let mut options = serde_saphyr::Options::default();
        options.budget = None;
        options.emit_comments = false;
        options
    }

    pub fn from_yaml_file(file_path: &Path) -> Result<Tree, MappedErrors> {
        let file_content =
            read_to_string(file_path).expect("Could not read file");

        let tree: Tree =
            serde_saphyr::from_str_with_options(&file_content, Self::yaml_options())
                .expect("Could not parse tree");

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
        Self::init_from_file_with_rooting(tree_path, min_branch_support, None)
    }

    /// Create a new Tree from a Newick file, optionally rooting it first.
    ///
    /// Without `rooting`, the tree must already be rooted: an unrooted tree
    /// (a root with three or more children, or a `[&U]` annotation) is an
    /// error that suggests `cls root-tree`. With `rooting`, an unrooted tree
    /// is rooted in memory with the given method, and a tree that is already
    /// rooted is used as is.
    pub fn init_from_file_with_rooting(
        tree_path: &Path,
        min_branch_support: f64,
        rooting: Option<&RootingMethod>,
    ) -> Result<Tree, MappedErrors> {
        let fail = |code: TelemetryCode, msg: String| {
            use_case_err(msg).with_code(code.to_string().as_str()).as_error()
        };

        match tree_path.extension().and_then(OsStr::to_str) {
            Some("nwk" | "newick" | "tree") => (),
            _ => {
                return fail(
                    TelemetryCode::UCTREE0007,
                    format!(
                        "Tree file format is not supported: {path}. Use a \
                        Newick file with extension .nwk, .newick or .tree.",
                        path = tree_path.display()
                    ),
                )
            }
        }

        let newick_content = match read_to_string(tree_path) {
            Ok(content) => content,
            Err(err) => {
                return fail(
                    TelemetryCode::UCTREE0007,
                    format!(
                        "Could not read file {path}: {err}",
                        path = tree_path.display()
                    ),
                )
            }
        };

        let hint = format!(
            "Root it first, e.g. \"cls root-tree {path} -o rooted.nwk\", or \
            pass --root to build-db.",
            path = tree_path.display()
        );

        let newick_content = match rooting {
            Some(method) => {
                info!(
                    "Rooting the reference tree in memory ({method:?}) if it \
                    is not rooted"
                );
                root_newick(&newick_content, method, true)?
            }
            None => {
                let (flag, content) = split_rooting_annotation(&newick_content);
                if flag == Some(RootingFlag::Unrooted) {
                    return fail(
                        TelemetryCode::UCTREE0001,
                        format!(
                            "The reference tree is marked as unrooted ([&U]). \
                            {hint}"
                        ),
                    );
                }
                content
            }
        };

        let phylo_tree = parse_newick(&newick_content)?;

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

        let root_tree = match phylo_tree
            .get_root()
            .ok()
            .and_then(|root| phylo_tree.get(&root).ok())
        {
            Some(root) => root,
            None => {
                return fail(
                    TelemetryCode::UCTREE0007,
                    "Could not get the tree root".to_string(),
                )
            }
        };

        match root_tree.children.len() {
            2 => (),
            0 | 1 => {
                return fail(
                    TelemetryCode::UCTREE0003,
                    format!(
                        "The tree root has a single child or none, so the \
                        tree has no rooted split. {hint}"
                    ),
                )
            }
            count => {
                return fail(
                    TelemetryCode::UCTREE0002,
                    format!(
                        "The reference tree is not rooted (the root has \
                        {count} children). {hint}"
                    ),
                )
            }
        }

        let children = Self::get_children_nodes(&phylo_tree, &root_tree.id);

        let sanitized_root =
            Tree::sanitize(Clade::new_root(0.0, children), min_branch_support)?;

        let root_children =
            sanitized_root.children.as_ref().map_or(0, |c| c.len());
        if root_children > 2 {
            warn!(
                code = TelemetryCode::UCTREE0004.to_string(),
                "Collapsing branches with support below {min_branch_support} \
                turned the tree root into a polytomy with {root_children} \
                children. Placement reports a tie between root children as \
                Unclassifiable."
            );
        }

        let mut new_tree = Tree::new(
            Uuid::new_v3(&Uuid::NAMESPACE_DNS, &*root_name.as_bytes()),
            root_name,
            min_branch_support,
            sanitized_root,
        );

        new_tree.root = Self::fix_parent_ids(&mut new_tree.root, None);

        Ok(new_tree)
    }

    fn fix_parent_ids(clade: &mut Clade, parent: Option<u64>) -> Clade {
        clade.children = if let Some(children) = clade.children.to_owned() {
            let children = children
                .into_iter()
                .map(|mut child| {
                    child.parent = Some(clade.id);
                    Self::fix_parent_ids(&mut child, Some(clade.id))
                })
                .collect::<Vec<Clade>>();

            Some(children)
        } else {
            None
        };

        clade.parent = parent;
        clade.clone()
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
                    if support >= min_branch_support
                        || sanitized_child.is_leaf()
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
                        node_id.to_owned() as u64,
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
                            node_id.to_owned() as u64,
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

    fn leaf(id: u64, parent: u64, name: &str) -> Clade {
        Clade::new_leaf(id, parent, name.to_string(), Some(0.1))
    }

    fn internal(
        id: u64,
        parent: u64,
        support: Option<f64>,
        children: Vec<Clade>,
    ) -> Clade {
        Clade::new_internal(
            id,
            parent,
            None,
            support,
            Some(0.2),
            Some(children),
        )
    }

    /// root(0) -> [n1(0.9) -> [n2(0.5) -> [A3, B4], C5], n6(None) -> [D7, E8]]
    fn hand_built_root() -> Clade {
        Clade::new_root(
            0.0,
            Some(vec![
                internal(
                    1,
                    0,
                    Some(0.9),
                    vec![
                        internal(
                            2,
                            1,
                            Some(0.5),
                            vec![leaf(3, 2, "A"), leaf(4, 2, "B")],
                        ),
                        leaf(5, 1, "C"),
                    ],
                ),
                internal(6, 0, None, vec![leaf(7, 6, "D"), leaf(8, 6, "E")]),
            ]),
        )
    }

    fn child_ids(clade: &Clade) -> Vec<u64> {
        clade.children.iter().flatten().map(|c| c.id).collect()
    }

    #[test]
    fn sanitize_reattaches_children_and_keeps_nodes_without_support() {
        let root = Tree::sanitize(hand_built_root(), 0.6).unwrap();

        assert_eq!(child_ids(&root), [1, 6]);
        let n1 = root.get_node_by_id(1).unwrap();
        assert_eq!(child_ids(n1), [3, 4, 5]);
        assert!(root.get_node_by_id(2).is_none());
        assert_eq!(child_ids(root.get_node_by_id(6).unwrap()), [7, 8]);

        // sanitize does not touch parent ids; fix_parent_ids does
        assert_eq!(root.get_node_by_id(3).unwrap().parent, Some(2));
    }

    #[test]
    fn sanitize_keeps_everything_when_threshold_is_met() {
        let root = Tree::sanitize(hand_built_root(), 0.5).unwrap();
        assert_eq!(root, hand_built_root());
    }

    #[test]
    fn fix_parent_ids_points_every_child_to_its_parent() {
        let mut root = Tree::sanitize(hand_built_root(), 1.0).unwrap();
        assert_eq!(child_ids(&root), [3, 4, 5, 6]);

        let fixed = Tree::fix_parent_ids(&mut root, None);

        assert_eq!(fixed.parent, None);
        for id in [3, 4, 5, 6] {
            assert_eq!(fixed.get_node_by_id(id).unwrap().parent, Some(0));
        }
        for id in [7, 8] {
            assert_eq!(fixed.get_node_by_id(id).unwrap().parent, Some(6));
        }
    }
}
