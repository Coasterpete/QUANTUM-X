//! Renderer-independent station inspection. These are geometric references,
//! not an articulated rigid-body solution or a wheel/rail contact model.
use crate::{Pose, Result};
use qx_geometry::{math::Vec3, solver::GeneratedTrack};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct InspectionReference {
    pub component_id: String,
    /// Signed offset from the selected station, in metres of arc distance.
    pub station_offset_m: f64,
    /// Track-frame offset at the evaluated station, in metres.
    pub local_offset_m: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectionPose {
    pub component_id: String,
    pub station_m: f64,
    pub pose: Pose,
}

/// Evaluates explicit component reference poses. It neither fits rigid bodies
/// between bogies nor mutates the authored track; out-of-range references fail.
pub fn inspect(
    track: &GeneratedTrack,
    station_m: f64,
    references: &[InspectionReference],
) -> Result<Vec<InspectionPose>> {
    if !station_m.is_finite() || references.len() > 4096 {
        return Err("invalid inspection station or reference count".into());
    }
    let mut ids = HashSet::new();
    references
        .iter()
        .map(|reference| {
            if reference.component_id.is_empty()
                || reference.component_id.len() > 128
                || !ids.insert(&reference.component_id)
                || !reference.station_offset_m.is_finite()
                || !reference.local_offset_m.is_finite()
            {
                return Err("invalid or duplicate inspection reference".into());
            }
            let station = station_m + reference.station_offset_m;
            let sample = track
                .sample_at_distance(station)
                .map_err(|e| e.to_string())?;
            Ok(InspectionPose {
                component_id: reference.component_id.clone(),
                station_m: station,
                pose: Pose {
                    position_m: sample.position_m
                        + sample.frame.local_to_world(reference.local_offset_m),
                    orientation: sample.frame.orientation(),
                },
            })
        })
        .collect()
}
