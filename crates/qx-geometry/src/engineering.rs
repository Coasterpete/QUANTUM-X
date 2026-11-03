//! Canonical engineering rows derived from the same solver measurements as the
//! viewport. Speed is authored, acceleration is kinematic, never physics output.
use crate::{
    Result,
    math::Vec3,
    solver::{GeneratedTrack, GraphSample},
};
#[derive(Clone, Debug)]
pub struct EngineeringRow {
    pub geometry: GraphSample,
    pub radius_m: Option<f64>,
    pub acceleration_mps2: Vec3,
    pub specific_force_mps2: Vec3,
    pub longitudinal_g: f64,
}
pub fn snapshot(track: &GeneratedTrack, gravity_mps2: f64) -> Result<Vec<EngineeringRow>> {
    crate::ensure(
        gravity_mps2.is_finite() && gravity_mps2 > 0.,
        "engineering gravity must be positive",
    )?;
    track
        .graphs
        .iter()
        .map(|g| {
            let sample = track.sample_at_distance(g.distance_m)?;
            let acceleration_mps2 = g.frenet_normal.unwrap_or(Vec3::ZERO)
                * (g.curvature_per_m * g.reference_speed_mps.powi(2));
            let specific_force_mps2 = acceleration_mps2 + Vec3::Y * gravity_mps2;
            Ok(EngineeringRow {
                geometry: g.clone(),
                radius_m: (g.curvature_per_m > 1e-12).then(|| 1. / g.curvature_per_m),
                acceleration_mps2,
                specific_force_mps2,
                longitudinal_g: specific_force_mps2.dot(sample.frame.tangent()) / gravity_mps2,
            })
        })
        .collect()
}
