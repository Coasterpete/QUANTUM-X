//! Opt-in, read-only wheel alignment of existing rigid component-origin poses.
//! No serde/schema, fitting, contacts, impulses, spin or simulation-step hooks.
use crate::{Body, MassProperties, Result, consist::ConsistInspection};
use qx_document::train::Train;
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    rail_geometry::{
        NearestPath, PhysicalRailGeometry, SearchOptions, SearchStatus, StationWindow,
        WheelCenterPath,
    },
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelRole {
    Running,
    SideGuide,
    UpStop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WheelTarget {
    Declared(WheelCenterPath),
    /// Missing contact/locus metadata is explicit; markers are never promoted
    /// automatically to mechanical wheels, and GLB geometry is never consulted.
    Unavailable(String),
}
#[derive(Clone, Debug, PartialEq)]
pub struct WheelGeometry {
    pub id: String,
    /// Stable ID of the rigid component containing the center and wheel frame.
    /// Shared running gear owns one wheel set regardless of car associations.
    pub component_id: String,
    pub center_local_m: Vec3,
    /// Wheel-local +X rolling direction, +Y radial reference, +Z axle.
    /// Independent of component axes, joint frames and visual corrections.
    pub orientation_local: Quaternion,
    pub role: Option<WheelRole>,
    pub target: WheelTarget,
    /// Each wheel has its OWN explicit local search window in physical stations.
    pub window: StationWindow,
}
#[derive(Clone, Debug, PartialEq)]
pub enum TargetObservation {
    Measured(NearestPath),
    Unavailable(String),
    Invalid(String),
    /// Whole-report work limit, distinct from unavailable mechanical metadata.
    BudgetExceeded,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WheelAlignment {
    pub id: String,
    pub component_id: String,
    pub role: Option<WheelRole>,
    pub center_world_m: Vec3,
    pub wheel_frame_world: Frame,
    pub target: TargetObservation,
    /// Directed wheel +X to locus tangent; omitted for unresolved targets.
    pub rolling_direction_error_rad: Option<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JointAlignment {
    pub id: String,
    pub anchor_gap_m: Vec3,
    pub angular_error_rad: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WheelAlignmentReport {
    pub wheels: Vec<WheelAlignment>,
    pub joints: Vec<JointAlignment>,
    /// Pairwise wheel/origin/joint-anchor distance preservation; distinct from
    /// wheel-to-path error and inter-component mechanical closure.
    pub max_rigid_attachment_error_m: f64,
    /// Actual segments on successful queries; failed queries conservatively
    /// charge their full allowance so total diagnostic work stays bounded.
    pub segments_tested: usize,
    pub resolved_wheels: usize,
    /// Entire wheel set only. Any missing/ambiguous/boundary/invalid target
    /// suppresses aggregates; an empty set is not reported as zero error.
    pub rms_error_m: Option<f64>,
    pub max_error_m: Option<f64>,
}

const MAX_WHEELS: usize = 4096;
const MAX_TOTAL_SEGMENTS: usize = 262144;
const MAX_ATTACHMENT_PAIRS: usize = 262144;

/// Evaluate an immutable inspection or playback snapshot. Poses MUST be at
/// component origins; convert mass-center body poses via the existing consist
/// snapshot adapter first. Does not call assembly, even if wheel errors are large.
pub fn diagnose(
    train: &Train,
    poses: &ConsistInspection,
    geometry: &PhysicalRailGeometry<'_>,
    wheels: &[WheelGeometry],
    options: SearchOptions,
) -> Result<WheelAlignmentReport> {
    train.validate().map_err(|e| e.to_string())?;
    options.validate().map_err(|e| e.to_string())?;
    if wheels.len() > MAX_WHEELS || poses.components.len() != train.components.len() {
        return Err("invalid wheel/pose count".into());
    }
    let mut bodies = Vec::new();
    let mut indices = BTreeMap::new();
    for p in &poses.components {
        if indices.insert(p.id.as_str(), bodies.len()).is_some() {
            return Err("duplicate component pose identity".into());
        }
        let body = Body {
            pose: p.pose,
            mass: MassProperties {
                mass_kg: 1.,
                principal_inertia_kg_m2: Vec3::new(1., 1., 1.),
            },
            linear_velocity_mps: Vec3::ZERO,
            angular_velocity_radps: Vec3::ZERO,
            force_n: Vec3::ZERO,
            torque_nm: Vec3::ZERO,
        };
        body.validate()?;
        bodies.push(body);
    }
    if train
        .components
        .iter()
        .any(|c| !indices.contains_key(c.id.as_str()))
    {
        return Err("missing component pose identity".into());
    }
    let mut result = WheelAlignmentReport {
        wheels: Vec::new(),
        joints: Vec::new(),
        max_rigid_attachment_error_m: 0.,
        segments_tested: 0,
        resolved_wheels: 0,
        rms_error_m: None,
        max_error_m: None,
    };
    let mut attachments = vec![vec![Vec3::ZERO]; bodies.len()];
    for j in &train.connections {
        let a = indices[j.component_a.as_str()];
        let b = indices[j.component_b.as_str()];
        let gap = bodies[a].world_point(j.anchor_a_m)? - bodies[b].world_point(j.anchor_b_m)?;
        let angular = if matches!(j.model, qx_document::train::mechanism::JointModel::Ball) {
            0.
        } else {
            crate::AngularJoint {
                body_a: a,
                body_b: b,
                model: j.model.clone(),
            }
            .error(&bodies)?
        };
        if !gap.is_finite() || !gap.length().is_finite() || !angular.is_finite() {
            return Err("joint diagnostic overflow".into());
        }
        result.joints.push(JointAlignment {
            id: j.id.clone(),
            anchor_gap_m: gap,
            angular_error_rad: angular,
        });
        attachments[a].push(j.anchor_a_m);
        attachments[b].push(j.anchor_b_m);
    }
    let mut ids = BTreeSet::new();
    let mut norm: f64 = 0.;
    let mut maximum: f64 = 0.;
    for w in wheels {
        if w.id.is_empty() || w.id.len() > 128 || !ids.insert(&w.id) {
            return Err("invalid or duplicate wheel identity".into());
        }
        let i = *indices
            .get(w.component_id.as_str())
            .ok_or_else(|| format!("wheel {} has unknown component {}", w.id, w.component_id))?;
        if !w.center_local_m.is_finite() || !unit(w.orientation_local) {
            return Err(format!("wheel {} has invalid local geometry", w.id));
        }
        let center = bodies[i].world_point(w.center_local_m)?;
        let frame = Frame::new(bodies[i].pose.orientation.compose(w.orientation_local))
            .map_err(|e| e.to_string())?;
        attachments[i].push(w.center_local_m);
        let target = match &w.target {
            WheelTarget::Unavailable(reason) => TargetObservation::Unavailable(reason.clone()),
            WheelTarget::Declared(path) => {
                let remaining = MAX_TOTAL_SEGMENTS - result.segments_tested;
                if remaining == 0 {
                    TargetObservation::BudgetExceeded
                } else {
                    let bounded = SearchOptions {
                        max_segments: options.max_segments.min(remaining),
                        ..options
                    };
                    match geometry.nearest(path, center, w.window, bounded) {
                        Ok(observation) => {
                            result.segments_tested += observation.segments_tested;
                            TargetObservation::Measured(observation)
                        }
                        Err(e) => {
                            // A failed search may have consumed its full budget.
                            result.segments_tested += bounded.max_segments;
                            TargetObservation::Invalid(e.to_string())
                        }
                    }
                }
            }
        };
        let mut rolling_direction_error_rad = None;
        if let TargetObservation::Measured(n) = &target
            && n.status == SearchStatus::Resolved
        {
            let c = &n.candidates[0];
            result.resolved_wheels += 1;
            norm = norm.hypot(c.perpendicular_error_m);
            maximum = maximum.max(c.perpendicular_error_m);
            rolling_direction_error_rad = c
                .sample
                .incoming_tangent
                .is_none()
                .then(|| frame.tangent().dot(c.sample.tangent).clamp(-1., 1.).acos());
        }
        result.wheels.push(WheelAlignment {
            id: w.id.clone(),
            component_id: w.component_id.clone(),
            role: w.role,
            center_world_m: center,
            wheel_frame_world: frame,
            target,
            rolling_direction_error_rad,
        });
    }
    let mut pairs = 0_usize;
    for (i, points) in attachments.iter().enumerate() {
        pairs = pairs
            .checked_add(points.len().saturating_mul(points.len().saturating_sub(1)) / 2)
            .ok_or("attachment count overflow")?;
        if pairs > MAX_ATTACHMENT_PAIRS {
            return Err("wheel attachment diagnostic budget exceeded".into());
        }
        let world = points
            .iter()
            .map(|p| bodies[i].world_point(*p))
            .collect::<Result<Vec<_>>>()?;
        for j in 0..points.len() {
            for k in 0..j {
                let error =
                    ((world[j] - world[k]).length() - (points[j] - points[k]).length()).abs();
                if !error.is_finite() {
                    return Err("wheel attachment diagnostic overflow".into());
                }
                result.max_rigid_attachment_error_m =
                    result.max_rigid_attachment_error_m.max(error);
            }
        }
    }
    if result.resolved_wheels == wheels.len() && !wheels.is_empty() {
        result.rms_error_m = Some(norm / (wheels.len() as f64).sqrt());
        result.max_error_m = Some(maximum);
    }
    Ok(result)
}
fn unit(q: Quaternion) -> bool {
    let n = q.w.hypot(q.x).hypot(q.y).hypot(q.z);
    n.is_finite() && (n - 1.).abs() <= 1e-8
}
