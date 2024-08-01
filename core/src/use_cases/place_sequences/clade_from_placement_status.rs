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
