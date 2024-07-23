use crate::domain::dtos::{clade::Clade, placement_response::PlacementStatus};

pub(super) enum IntrospectionUpdateResponse {
    Continue(Clade, Vec<Clade>),
    Return(PlacementStatus),
}
