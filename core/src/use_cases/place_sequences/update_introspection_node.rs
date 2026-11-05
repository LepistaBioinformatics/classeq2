use super::_dtos::IntrospectionUpdateResponse;
use crate::domain::dtos::{
    adherence_test::AdherenceTest, clade::Clade,
    placement_response::PlacementStatus::*, telemetry_code::TelemetryCode,
};

use mycelium_base::{
    dtos::UntaggedParent,
    utils::errors::{use_case_err, MappedErrors},
};
use tracing::trace;

pub(super) fn update_introspection_node(
    adherence: &AdherenceTest,
) -> Result<IntrospectionUpdateResponse, MappedErrors> {
    //
    // 🍁 clade update
    //
    let parent = match adherence.clade.to_owned() {
        UntaggedParent::Record(record) => record,
        UntaggedParent::Id(_) => {
            return use_case_err(
                "The adherence test does not contain a clade record.",
            )
            .as_error();
        }
    };

    //
    // 🌿 1st children update
    //
    let children = match parent.to_owned().children {
        Some(children) => {
            let non_leaf_children = children
                .iter()
                .filter_map(|record| {
                    if record.is_leaf() {
                        return None;
                    }

                    Some(record.to_owned())
                })
                .collect::<Vec<Clade>>();

            //
            // ✅ Case no children clades exits, the search loop is
            // finished with a conclusive identity.
            //
            if non_leaf_children.is_empty() {
                trace!(
                    code = TelemetryCode::UCPLACE0016.to_string(),
                    "Conclusive identity found at clade {clade_id}",
                    clade_id = parent.id
                );

                return Ok(IntrospectionUpdateResponse::Return(IdentityFound(
                    adherence.to_owned(),
                )));
            }

            //
            // 🟢 Case the clade contain children ones, the search loop
            // continues.
            //
            trace!(
                code = TelemetryCode::UCPLACE0017.to_string(),
                "One proposal found. Clade {parent} selected",
                parent = parent.id
            );

            non_leaf_children
        }
        None => {
            //
            // ✅ Case no children clades exits, the search loop is
            // finished with a conclusive identity.
            //
            trace!(
                code = TelemetryCode::UCPLACE0016.to_string(),
                "Conclusive identity found at clade {clade_id}",
                clade_id = parent.id
            );

            return Ok(IntrospectionUpdateResponse::Return(IdentityFound(
                adherence.to_owned(),
            )));
        }
    };

    Ok(IntrospectionUpdateResponse::Continue(parent, children))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dtos::clade::NodeType;

    fn clade(id: u64, kind: NodeType, children: Option<Vec<Clade>>) -> Clade {
        Clade {
            id,
            parent: Some(0),
            kind,
            name: None,
            support: None,
            length: None,
            children,
        }
    }

    fn leaf(id: u64) -> Clade {
        clade(id, NodeType::Leaf, None)
    }

    fn test_for(clade: UntaggedParent<Clade, u64>) -> AdherenceTest {
        AdherenceTest {
            clade,
            one: 10,
            rest: 3,
        }
    }

    #[test]
    fn update_with_clade_id_instead_of_record_returns_err() {
        let result =
            update_introspection_node(&test_for(UntaggedParent::Id(1)));

        assert!(result.is_err());
    }

    #[test]
    fn update_with_clade_without_children_returns_identity_found() {
        let test =
            test_for(UntaggedParent::Record(clade(1, NodeType::Node, None)));

        match update_introspection_node(&test).unwrap() {
            IntrospectionUpdateResponse::Return(status) => {
                assert_eq!(status, IdentityFound(test))
            }
            IntrospectionUpdateResponse::Continue(..) => {
                panic!("expected Return")
            }
        }
    }

    #[test]
    fn update_with_only_leaf_children_returns_identity_found() {
        let test = test_for(UntaggedParent::Record(clade(
            1,
            NodeType::Node,
            Some(vec![leaf(2), leaf(3)]),
        )));

        match update_introspection_node(&test).unwrap() {
            IntrospectionUpdateResponse::Return(status) => {
                assert_eq!(status, IdentityFound(test))
            }
            IntrospectionUpdateResponse::Continue(..) => {
                panic!("expected Return")
            }
        }
    }

    #[test]
    fn update_with_internal_children_continues_with_non_leaf_children_only() {
        let internal = clade(3, NodeType::Node, Some(vec![leaf(4), leaf(5)]));
        let parent =
            clade(1, NodeType::Node, Some(vec![leaf(2), internal.clone()]));
        let test = test_for(UntaggedParent::Record(parent.clone()));

        match update_introspection_node(&test).unwrap() {
            IntrospectionUpdateResponse::Continue(new_parent, children) => {
                assert_eq!(new_parent, parent);
                assert_eq!(children, vec![internal]);
            }
            IntrospectionUpdateResponse::Return(status) => {
                panic!("expected Continue, got {status:?}")
            }
        }
    }
}
