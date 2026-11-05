use crate::domain::dtos::placement_response::PlacementStatus::{self, *};

use mycelium_base::dtos::UntaggedParent;

pub(super) fn clade_from_placement_status(
    placement: Option<&PlacementStatus>,
) -> Option<u64> {
    match placement {
        None => None,
        Some(res) => match res {
            MaxResolutionReached(id, _) => Some(id.to_owned()),
            IdentityFound(test) => match test.to_owned().clade {
                UntaggedParent::Record(record) => Some(record.id),
                UntaggedParent::Id(id) => Some(id),
            },
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dtos::{
        adherence_test::AdherenceTest,
        clade::{Clade, NodeType},
    };

    fn identity(clade: UntaggedParent<Clade, u64>) -> PlacementStatus {
        IdentityFound(AdherenceTest {
            clade,
            one: 1,
            rest: 0,
        })
    }

    #[test]
    fn clade_from_none_is_none() {
        assert_eq!(clade_from_placement_status(None), None);
    }

    #[test]
    fn clade_from_max_resolution_reached_is_its_id() {
        let status = MaxResolutionReached(7, "LCA Accepted".to_string());

        assert_eq!(clade_from_placement_status(Some(&status)), Some(7));
    }

    #[test]
    fn clade_from_identity_found_record_is_record_id() {
        let status = identity(UntaggedParent::Record(Clade {
            id: 5,
            parent: Some(0),
            kind: NodeType::Node,
            name: None,
            support: None,
            length: None,
            children: None,
        }));

        assert_eq!(clade_from_placement_status(Some(&status)), Some(5));
    }

    #[test]
    fn clade_from_identity_found_id_is_that_id() {
        let status = identity(UntaggedParent::Id(9));

        assert_eq!(clade_from_placement_status(Some(&status)), Some(9));
    }

    #[test]
    fn clade_from_unclassifiable_or_inconclusive_is_none() {
        let unclassifiable = Unclassifiable("x".to_string());
        let inconclusive = Inconclusive(
            vec![AdherenceTest {
                clade: UntaggedParent::Id(1),
                one: 1,
                rest: 0,
            }],
            "Multiple proposals".to_string(),
        );

        assert_eq!(clade_from_placement_status(Some(&unclassifiable)), None);
        assert_eq!(clade_from_placement_status(Some(&inconclusive)), None);
    }
}
