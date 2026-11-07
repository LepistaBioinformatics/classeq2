use crate::domain::dtos::{clade::Clade, placement_response::PlacementStatus};

pub(super) enum IntrospectionUpdateResponse<'a> {
    Continue(&'a Clade, &'a [Clade]),
    Return(PlacementStatus),
}
