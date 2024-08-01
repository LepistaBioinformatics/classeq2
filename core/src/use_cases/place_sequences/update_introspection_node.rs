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
