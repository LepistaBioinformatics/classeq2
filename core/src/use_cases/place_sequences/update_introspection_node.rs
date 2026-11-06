use super::_dtos::IntrospectionUpdateResponse;
use crate::domain::dtos::{
    adherence_test::AdherenceTest, placement_response::PlacementStatus::*,
    telemetry_code::TelemetryCode,
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
    // 🌿 children update: every child competes at the next level, leaves
    // included (003-placement-algorithm, BUG-027).
    //
    let children = match parent.to_owned().children {
        Some(children) if !children.is_empty() => {
            trace!(
                code = TelemetryCode::UCPLACE0017.to_string(),
                "One proposal found. Clade {parent} selected",
                parent = parent.id
            );

            children
        }
        _ => {
            //
            // ✅ The selected clade is a leaf: the search loop is finished
            // with a conclusive identity.
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
    use crate::domain::dtos::clade::{Clade, NodeType};

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
    fn update_with_leaf_returns_identity_found() {
        let test = test_for(UntaggedParent::Record(leaf(2)));

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
    fn update_with_only_leaf_children_continues_with_the_leaves() {
        let parent =
            clade(1, NodeType::Node, Some(vec![leaf(2), leaf(3)]));
        let test = test_for(UntaggedParent::Record(parent.clone()));

        match update_introspection_node(&test).unwrap() {
            IntrospectionUpdateResponse::Continue(new_parent, children) => {
                assert_eq!(new_parent, parent);
                assert_eq!(children, vec![leaf(2), leaf(3)]);
            }
            IntrospectionUpdateResponse::Return(status) => {
                panic!("expected Continue, got {status:?}")
            }
        }
    }

    #[test]
    fn update_with_mixed_children_continues_with_all_children() {
        let internal = clade(3, NodeType::Node, Some(vec![leaf(4), leaf(5)]));
        let parent =
            clade(1, NodeType::Node, Some(vec![leaf(2), internal.clone()]));
        let test = test_for(UntaggedParent::Record(parent.clone()));

        match update_introspection_node(&test).unwrap() {
            IntrospectionUpdateResponse::Continue(new_parent, children) => {
                assert_eq!(new_parent, parent);
                assert_eq!(children, vec![leaf(2), internal]);
            }
            IntrospectionUpdateResponse::Return(status) => {
                panic!("expected Continue, got {status:?}")
            }
        }
    }
}
