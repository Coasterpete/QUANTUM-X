//! Kinematic support gate and independent existing-constraint observations.
use super::FitSeed;
use crate::{
    Pose, Result,
    assembly_diagnostics::AssemblyResiduals,
    consist::{ConsistInspection, JointResidual},
    wheel_geometry::WheelAlignmentReport,
};
use qx_document::train::{
    Train,
    mechanism::{JointModel, RotationDrive},
};
use qx_geometry::math::{Frame, Quaternion, Vec3};

fn point(p: Pose, local: Vec3) -> Result<Vec3> {
    Ok(p.position_m
        + Frame::new(p.orientation)
            .map_err(|e| e.to_string())?
            .local_to_world(local))
}
pub(super) struct Coordinate {
    pub component_id: String,
    child: usize,
    parent_frame: Quaternion,
    child_frame: Quaternion,
    anchor_child: Vec3,
    pub pivot: Vec3,
    pub axis: Vec3,
    pub initial_angle: f64,
    pub min_angle: f64,
    pub max_angle: f64,
}
impl Coordinate {
    pub fn new(train: &Train, original: &ConsistInspection, id: &str) -> Result<Self> {
        let joint = train
            .connections
            .iter()
            .find(|j| j.id == id)
            .ok_or("unknown fitting connection")?;
        let JointModel::Hinge {
            frame_a,
            frame_b,
            limits: Some(limits),
            drive: RotationDrive::Free,
        } = joint.model
        else {
            return Err("prototype requires an authored, free, limited hinge; ball/roll-lock/fixed or driven joints are not fitted".into());
        };
        if train
            .connections
            .iter()
            .filter(|j| j.component_a == joint.component_b || j.component_b == joint.component_b)
            .count()
            != 1
        {
            return Err("moving carrier must be a mechanical leaf; shared attachments, moving trees and closed loops need coupled closure".into());
        }
        let child = original
            .components
            .iter()
            .position(|c| c.id == joint.component_b)
            .ok_or("missing child pose")?;
        let parent = original
            .components
            .iter()
            .find(|c| c.id == joint.component_a)
            .ok_or("missing parent pose")?
            .pose;
        let parent_frame = parent.orientation.compose(frame_a);
        let frame = Frame::new(parent_frame).map_err(|e| e.to_string())?;
        let relative = parent_frame
            .conjugate()
            .compose(original.components[child].pose.orientation.compose(frame_b));
        let theta = (2. * relative.z.atan2(relative.w) + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        // Seed may pass the assembly tolerance while fractionally outside a
        // limit. Do not silently clamp the baseline or enlarge its limits.
        if theta < limits.min_rad || theta > limits.max_rad {
            return Err("M2B seed is outside the exact authored hinge interval".into());
        }
        Ok(Self {
            component_id: joint.component_b.clone(),
            child,
            parent_frame,
            child_frame: frame_b,
            anchor_child: joint.anchor_b_m,
            pivot: point(parent, joint.anchor_a_m)?,
            axis: frame.lateral(),
            initial_angle: theta,
            min_angle: limits.min_rad,
            max_angle: limits.max_rad,
        })
    }
    pub fn pose(&self, original: &ConsistInspection, theta: f64) -> Result<ConsistInspection> {
        let mut result = original.clone();
        let orientation = self
            .parent_frame
            .compose(Quaternion::axis_angle(Vec3::Z, theta).map_err(|e| e.to_string())?)
            .compose(self.child_frame.conjugate())
            .normalized()
            .map_err(|e| e.to_string())?;
        let frame = Frame::new(orientation).map_err(|e| e.to_string())?;
        result.components[self.child].pose = Pose {
            position_m: self.pivot - frame.local_to_world(self.anchor_child),
            orientation,
        };
        Ok(result)
    }
}

pub(super) fn invariant_guides(
    seed: &FitSeed<'_>,
    coordinate: &Coordinate,
) -> Result<Option<String>> {
    let terminal = seed
        .diagnostics
        .terminal_residuals
        .as_ref()
        .ok_or("missing assembly evidence")?;
    let pose = seed.original.components[coordinate.child].pose;
    for g in &terminal.guides {
        if g.component_id == coordinate.component_id {
            // Test authored geometry relative to the child anchor, not the
            // seed's small inter-body closure error. A guide at that anchor is
            // invariant even when M2B left a micrometre of anchor gap.
            let radius = Frame::new(pose.orientation)
                .map_err(|e| e.to_string())?
                .local_to_world(g.local_point_m - coordinate.anchor_child)
                .cross(coordinate.axis)
                .length();
            // Only guides on the actual rotation axis remain invariant for
            // every iterate. Never spend their 1e-5 m tolerance as fitting travel.
            if radius > 1e-10 {
                return Ok(Some("carrier guide lies off the hinge axis; independent fitting would change existing guidance".into()));
            }
        }
    }
    Ok(None)
}

pub(super) fn measure(
    seed: &FitSeed<'_>,
    candidate: &ConsistInspection,
) -> Result<AssemblyResiduals> {
    let terminal = seed
        .diagnostics
        .terminal_residuals
        .as_ref()
        .ok_or("missing assembly evidence")?;
    let poses: Vec<_> = candidate.components.iter().map(|c| c.pose).collect();
    let mut measured = terminal.clone();
    measured.component_poses = poses.clone();
    measured.max_centerline_guide_error_m = 0.;
    for (index, g) in measured.guides.iter_mut().enumerate() {
        let body = candidate
            .components
            .iter()
            .find(|c| c.id == g.component_id)
            .ok_or("missing guide owner")?;
        let p = point(body.pose, g.local_point_m)?;
        if !g.pinned {
            g.station_m = crate::solver::project_station(seed.track, p, g.station_m)?;
        }
        let component = seed
            .train
            .components
            .iter()
            .find(|c| c.id == g.component_id)
            .ok_or("missing guide component")?;
        let initial = seed
            .diagnostics
            .seed_residuals
            .as_ref()
            .ok_or("missing guide seed")?
            .guides[index]
            .station_m;
        let radius = (component.dimensions_m.length() * 0.5).max(0.25);
        if (g.station_m - initial).abs() > (radius * 8.).max(4.) {
            return Err("candidate guide left its M2B station neighborhood".into());
        }
        let target = seed
            .track
            .sample_at_distance(g.station_m)
            .map_err(|e| e.to_string())?;
        g.error_m = (p - target.position_m).length();
        if !g.error_m.is_finite() {
            return Err("guide measurement overflow".into());
        }
        measured.max_centerline_guide_error_m =
            measured.max_centerline_guide_error_m.max(g.error_m);
    }
    for (index, c) in seed.train.components.iter().enumerate() {
        if poses[index] == seed.original.components[index].pose {
            continue;
        }
        let mut points = c.guide_references_m.clone();
        points.extend_from_slice(&c.wheel_references_m);
        points.extend([Vec3::ZERO, c.dimensions_m * 0.5, c.dimensions_m * (-0.5)]);
        for (i, &a) in points.iter().enumerate() {
            for &b in &points[..i] {
                let e = ((point(poses[index], a)? - point(poses[index], b)?).length()
                    - (a - b).length())
                .abs();
                if !e.is_finite() {
                    return Err("rigid dimension measurement overflow".into());
                }
                measured.max_rigid_attachment_error_m =
                    measured.max_rigid_attachment_error_m.max(e);
            }
        }
    }
    measured.max_rail_attitude_error_rad = 0.;
    let stations: Vec<_> = seed
        .original
        .components
        .iter()
        .map(|c| c.station_m)
        .collect();
    for mut a in crate::consist::rail_attitudes(seed.train, &stations) {
        a.station_m = crate::solver::project_station(
            seed.track,
            point(poses[a.body], a.local_point_m)?,
            a.station_m,
        )?;
        let track = seed
            .track
            .sample_at_distance(a.station_m)
            .map_err(|e| e.to_string())?;
        let rail = Frame::new(poses[a.body].orientation.compose(a.local_frame))
            .map_err(|e| e.to_string())?;
        let e = crate::angular::roll_constraint(track.frame, rail)?.1.abs();
        if !e.is_finite() {
            return Err("rail attitude measurement overflow".into());
        }
        measured.max_rail_attitude_error_rad = measured.max_rail_attitude_error_rad.max(e);
    }
    Ok(measured)
}

pub(super) fn accept_constraints(
    measured: &AssemblyResiduals,
    wheels: &WheelAlignmentReport,
) -> Option<String> {
    // Wheel diagnostics re-evaluate every joint and all wheel/anchor rigid pairs.
    if wheels
        .joints
        .iter()
        .any(|j| j.anchor_gap_m.length() > 1e-5 || j.angular_error_rad > 1e-5)
        || measured.max_centerline_guide_error_m > 1e-5
        || measured.max_rail_attitude_error_rad > 1e-5
        || wheels.max_rigid_attachment_error_m > 1e-10
        || measured.max_rigid_attachment_error_m > 1e-10
    {
        Some("candidate violates joint closure, angular limits, guide, rail attitude or rigid dimensions".into())
    } else {
        None
    }
}

/// Refresh derived connection fields from measured poses, never stale M2B gaps.
pub(super) fn refresh(
    candidate: &mut ConsistInspection,
    measured: &mut AssemblyResiduals,
    wheels: &WheelAlignmentReport,
    train: &Train,
) -> Result<()> {
    candidate.joints.clear();
    for c in &mut candidate.components {
        if let Some(g) = measured.guides.iter().find(|g| g.component_id == c.id) {
            c.station_m = g.station_m;
        }
    }
    measured.max_joint_anchor_gap_m = 0.;
    measured.max_angular_joint_error_rad = 0.;
    measured.max_rigid_attachment_error_m = measured
        .max_rigid_attachment_error_m
        .max(wheels.max_rigid_attachment_error_m);
    for ((j, row), connection) in train
        .connections
        .iter()
        .zip(&wheels.joints)
        .zip(&mut measured.connections)
    {
        let a = candidate
            .components
            .iter()
            .find(|c| c.id == j.component_a)
            .ok_or("missing joint A")?;
        let b = candidate
            .components
            .iter()
            .find(|c| c.id == j.component_b)
            .ok_or("missing joint B")?;
        let pa = point(a.pose, j.anchor_a_m)?;
        let pb = point(b.pose, j.anchor_b_m)?;
        connection.gap_m = row.anchor_gap_m;
        connection.angular_error_rad = row.angular_error_rad;
        measured.max_joint_anchor_gap_m = measured
            .max_joint_anchor_gap_m
            .max(row.anchor_gap_m.length());
        measured.max_angular_joint_error_rad = measured
            .max_angular_joint_error_rad
            .max(row.angular_error_rad);
        candidate.joints.push(JointResidual {
            id: j.id.clone(),
            anchor_a_m: pa,
            anchor_b_m: pb,
            error_m: (pa - pb).length(),
        });
    }
    Ok(())
}
