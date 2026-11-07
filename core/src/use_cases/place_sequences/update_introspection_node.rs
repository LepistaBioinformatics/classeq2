use super::_dtos::IntrospectionUpdateResponse;
use crate::domain::dtos::{
    adherence_test::AdherenceTest, clade::Clade,
    placement_response::PlacementStatus::*, telemetry_code::TelemetryCode,
};

use mycelium_base::dtos::UntaggedParent;
use tracing::trace;

/// Descend into the selected clade. The clade and its children are borrowed
/// from the tree; only a final leaf is cloned into the result.
pub(super) fn update_introspection_node(
    clade: &Clade,
    one: i32,
    rest: i32,
) -> IntrospectionUpdateResponse<'_> {
    //
    // 🌿 children update: every child competes at the next level, leaves
    // included (003-placement-algorithm, BUG-027).
    //
    match clade.children.as_deref() {
        Some(children) if !children.is_empty() => {
            trace!(
                code = TelemetryCode::UCPLACE0017.to_string(),
                "One proposal found. Clade {parent} selected",
                parent = clade.id
            );

            IntrospectionUpdateResponse::Continue(clade, children)
        }
        _ => {
            //
            // ✅ The selected clade is a leaf: the search loop is finished
            // with a conclusive identity.
            //
            trace!(
                code = TelemetryCode::UCPLACE0016.to_string(),
                "Conclusive identity found at clade {clade_id}",
                clade_id = clade.id
            );

            IntrospectionUpdateResponse::Return(IdentityFound(AdherenceTest {
                clade: UntaggedParent::Record(clade.to_owned()),
                one,
                rest,
            }))
        }
    }
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

    trait Record {
        fn record(&self) -> &Clade;
    }

    impl Record for AdherenceTest {
        fn record(&self) -> &Clade {
            match &self.clade {
                UntaggedParent::Record(clade) => clade,
                UntaggedParent::Id(_) => unreachable!(),
            }
        }
    }

    #[test]
    fn update_with_clade_without_children_returns_identity_found() {
        let test =
            test_for(UntaggedParent::Record(clade(1, NodeType::Node, None)));

        match update_introspection_node(test.record(), test.one, test.rest) {
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

        match update_introspection_node(test.record(), test.one, test.rest) {
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
        let parent = clade(1, NodeType::Node, Some(vec![leaf(2), leaf(3)]));
        let test = test_for(UntaggedParent::Record(parent.clone()));

        match update_introspection_node(test.record(), test.one, test.rest) {
            IntrospectionUpdateResponse::Continue(new_parent, children) => {
                assert_eq!(new_parent, &parent);
                assert_eq!(children, &[leaf(2), leaf(3)]);
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

        match update_introspection_node(test.record(), test.one, test.rest) {
            IntrospectionUpdateResponse::Continue(new_parent, children) => {
                assert_eq!(new_parent, &parent);
                assert_eq!(children, &[leaf(2), internal]);
            }
            IntrospectionUpdateResponse::Return(status) => {
                panic!("expected Continue, got {status:?}")
            }
        }
    }
}
