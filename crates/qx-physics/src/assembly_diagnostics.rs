//! Opt-in, non-persisted assembly observations. No timers, logging or velocity
//! projection. Normal inspection uses a compile-time uninstrumented solve.
use crate::{Pose, Result, TrackAttitude, TrackGuide, World};
use qx_document::train::{Train, mechanism::JointModel};
use qx_geometry::{
    math::{Frame, Vec3},
    solver::GeneratedTrack,
};
use std::collections::HashMap;

fn observe_max(maximum: &mut f64, value: f64) -> Result<()> {
    // f64::max alone ignores NaN, which would turn overflow into a false zero.
    if !value.is_finite() {
        return Err("constraint diagnostic overflow".into());
    }
    *maximum = maximum.max(value);
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChordTermination {
    #[default]
    NotRun,
    Converged,
    InvalidLength,
    NoBracket,
    TrackQueryFailed,
    InvalidTopology,
    SearchBudgetExceeded,
    ResidualNotMet,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChordDiagnostics {
    /// Owning car body when invoked by assembly; absent for direct chord probes.
    pub component_id: Option<String>,
    pub center_m: f64,
    pub attachment_distance_m: f64,
    pub max_span_m: f64,
    /// Hermite interval pairs visited (M1 counted uniform sign probes).
    pub bracket_probes: u32,
    /// Bisections across chord and derivative-root isolation, not fixed at 48.
    pub bisection_iterations: u32,
    pub polynomial_evaluations: u32,
    pub extrema_tested: u32,
    pub residual_tolerance_m: f64,
    pub circuit: bool,
    /// First monotone candidate interval, before refinement; may end at tangency.
    pub bracket_span_m: Option<[f64; 2]>,
    pub stations_m: Option<(f64, f64)>,
    /// Signed Euclidean chord length minus the requested length, metres.
    pub residual_m: Option<f64>,
    pub termination: ChordTermination,
    pub error: Option<String>,
}

/// Exercise the production chord search without assembling or editing a train.
/// Success requires a separately sampled metre residual within the reported
/// floating-point allowance. Open-end and four-length-span bounds are retained.
pub fn diagnose_chord(
    track: &GeneratedTrack,
    center_m: f64,
    attachment_distance_m: f64,
    diagnostics: &mut ChordDiagnostics,
) -> Result<(f64, f64)> {
    *diagnostics = ChordDiagnostics::default();
    let result =
        crate::assembly::chord::<true>(track, center_m, attachment_distance_m, diagnostics);
    if let Err(e) = &result {
        diagnostics.error = Some(e.clone());
    }
    result
}

/// Explicit topology-aware chord query. Circuit intent AND the existing
/// position/tangent/frame/join closure contract are required for wrapping.
/// Returns ordered, unwrapped stations about the normalized center, at most one
/// circuit of total span. This does not make open-track consist assembly seam-aware.
pub fn diagnose_chord_on_layout(
    track: &GeneratedTrack,
    center_m: f64,
    attachment_distance_m: f64,
    layout: qx_geometry::topology::LayoutMode,
    diagnostics: &mut ChordDiagnostics,
) -> Result<(f64, f64)> {
    *diagnostics = ChordDiagnostics::default();
    let circuit = layout == qx_geometry::topology::LayoutMode::Circuit;
    diagnostics.center_m = center_m;
    diagnostics.attachment_distance_m = attachment_distance_m;
    diagnostics.circuit = circuit;
    let result = (|| {
        if circuit {
            diagnostics.termination = ChordTermination::InvalidTopology;
            if !qx_geometry::topology::diagnose(track)
                .map_err(|e| e.to_string())?
                .valid
            {
                return Err("Circuit is incomplete: chord wrapping requires valid position, tangent, frame and joins".into());
            }
        }
        crate::chord::solve::<true>(track, center_m, attachment_distance_m, circuit, diagnostics)
    })();
    if let Err(e) = &result {
        diagnostics.error = Some(e.clone());
    }
    result
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssemblyPhase {
    #[default]
    PlacementSeed,
    ChordSeed,
    MountAndGuideSeed,
    Projection,
    Complete,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AssemblyTermination {
    #[default]
    NotRun,
    Converged,
    SweepLimit,
    Aborted,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssemblyResiduals {
    /// Completed projection sweeps; zero denotes the chord/mount seed.
    pub sweep: u32,
    /// Pairwise distance change between authored anchors, guide/wheel reference
    /// points and origin on each rigid component. This is not inter-body closure.
    pub max_rigid_attachment_error_m: f64,
    pub max_joint_anchor_gap_m: f64,
    /// Full distance to the solver's current guide station, including the pin.
    pub max_centerline_guide_error_m: f64,
    pub max_angular_joint_error_rad: f64,
    pub max_rail_attitude_error_rad: f64,
    /// Authored connection order and identity; no aggregate can hide a bad edge.
    pub connections: Vec<ConnectionResidual>,
    pub guides: Vec<GuideResidual>,
    /// Rigid component-origin snapshots in authored order, for read-only motion
    /// and translation/rotation propagation analysis between checkpoints.
    pub component_poses: Vec<Pose>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionResidual {
    pub id: String,
    pub gap_m: Vec3,
    pub angular_error_rad: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GuideResidual {
    pub component_id: String,
    pub local_point_m: Vec3,
    pub pinned: bool,
    pub station_m: f64,
    pub error_m: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssemblyDiagnostics {
    pub phase: AssemblyPhase,
    pub termination: AssemblyTermination,
    pub error: Option<String>,
    /// Completed sweeps. An abort within a sweep does not increment this count.
    pub projection_sweeps: u32,
    pub guide_count: usize,
    pub rail_attitude_count: usize,
    pub chords: Vec<ChordDiagnostics>,
    pub seed_residuals: Option<AssemblyResiduals>,
    /// Sweeps 1, 9, 17, ... and the final sweep on exhaustion (at most 301).
    pub convergence_checks: Vec<AssemblyResiduals>,
    /// Actual terminal poses, also after sweep 2400 on sweep exhaustion.
    /// Aborts before that point leave this absent; checks remain available.
    pub terminal_residuals: Option<AssemblyResiduals>,
    /// Observational failures never change the assembly Result or state.
    pub measurement_error: Option<String>,
}

pub(crate) fn measure(
    train: &Train,
    track: &GeneratedTrack,
    poses: &[Pose],
    guides: &[crate::assembly::Guide],
    attitudes: &[TrackAttitude],
    sweep: u32,
) -> Result<AssemblyResiduals> {
    let ids: HashMap<_, _> = train
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    let point = |i: usize, a: Vec3| -> Result<Vec3> {
        let p = poses[i];
        let v = p.position_m
            + Frame::new(p.orientation)
                .map_err(|e| e.to_string())?
                .local_to_world(a);
        if !v.is_finite() {
            return Err("assembly diagnostic transform overflow".into());
        }
        Ok(v)
    };
    let mut result = AssemblyResiduals {
        sweep,
        component_poses: poses.to_vec(),
        ..Default::default()
    };
    let mut attachments: Vec<Vec<Vec3>> = train
        .components
        .iter()
        .map(|c| {
            let mut points = c.guide_references_m.clone();
            points.extend_from_slice(&c.wheel_references_m);
            points.push(Vec3::ZERO);
            points
        })
        .collect();
    for j in &train.connections {
        let a = ids[j.component_a.as_str()];
        let b = ids[j.component_b.as_str()];
        attachments[a].push(j.anchor_a_m);
        attachments[b].push(j.anchor_b_m);
        let gap_m = point(a, j.anchor_a_m)? - point(b, j.anchor_b_m)?;
        observe_max(&mut result.max_joint_anchor_gap_m, gap_m.length())?;
        let mut angular_error_rad = 0.;
        if !matches!(j.model, JointModel::Ball) {
            // Same residual definition as playback, with no impulses applied.
            let bodies = [diagnostic_body(poses[a]), diagnostic_body(poses[b])];
            let joint = crate::AngularJoint {
                body_a: 0,
                body_b: 1,
                model: j.model.clone(),
            };
            angular_error_rad = joint.error(&bodies)?;
            observe_max(&mut result.max_angular_joint_error_rad, angular_error_rad)?;
        }
        result.connections.push(ConnectionResidual {
            id: j.id.clone(),
            gap_m,
            angular_error_rad,
        });
    }
    for (i, local) in attachments.iter().enumerate() {
        let world_points = local
            .iter()
            .map(|p| point(i, *p))
            .collect::<Result<Vec<_>>>()?;
        for (n, a) in local.iter().enumerate() {
            for (k, b) in local[..n].iter().enumerate() {
                let e = ((world_points[n] - world_points[k]).length() - (*a - *b).length()).abs();
                observe_max(&mut result.max_rigid_attachment_error_m, e)?;
            }
        }
    }
    for (index, g) in guides.iter().enumerate() {
        let target = track
            .sample_at_distance(g.station)
            .map_err(|e| e.to_string())?;
        let error_m = (point(g.body, g.local)? - target.position_m).length();
        observe_max(&mut result.max_centerline_guide_error_m, error_m)?;
        result.guides.push(GuideResidual {
            component_id: train.components[g.body].id.clone(),
            local_point_m: g.local,
            pinned: index == 0,
            station_m: g.station,
            error_m,
        });
    }
    for a in attitudes {
        let target = track
            .sample_at_distance(a.station_m)
            .map_err(|e| e.to_string())?;
        let rail = Frame::new(poses[a.body].orientation.compose(a.local_frame))
            .map_err(|e| e.to_string())?;
        observe_max(
            &mut result.max_rail_attitude_error_rad,
            crate::angular::roll_constraint(target.frame, rail)?.1.abs(),
        )?;
    }
    Ok(result)
}

fn diagnostic_body(pose: Pose) -> crate::Body {
    crate::Body {
        pose,
        mass: crate::MassProperties {
            mass_kg: 1.,
            principal_inertia_kg_m2: Vec3::new(1., 1., 1.),
        },
        linear_velocity_mps: Vec3::ZERO,
        angular_velocity_radps: Vec3::ZERO,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VelocityResiduals {
    pub max_joint_anchor_speed_mps: f64,
    pub max_guide_transverse_speed_mps: f64,
    pub max_angular_constraint_speed_radps: f64,
    /// Includes spatial bank derivative and the guide-point station Jacobian.
    pub max_rail_twist_speed_radps: f64,
}

/// Read-only velocity-level constraint checks. These are J*v, without positional
/// Baumgarte bias, applied forces, motors or velocity projection. Open track
/// queries match initialization. Hinge limits count only outward motion at a
/// reached bound; free hinge rotation is excluded.
pub fn velocity_residuals(
    world: &World,
    track: &GeneratedTrack,
    guides: &[TrackGuide],
    attitudes: &[TrackAttitude],
) -> Result<VelocityResiduals> {
    world.validate()?;
    let mut result = VelocityResiduals::default();
    for j in &world.joints {
        let a = &world.bodies[j.body_a];
        let b = &world.bodies[j.body_b];
        let v = b.point_velocity(b.frame().local_to_world(j.anchor_b_m))
            - a.point_velocity(a.frame().local_to_world(j.anchor_a_m));
        observe_max(&mut result.max_joint_anchor_speed_mps, v.length())?;
    }
    for j in &world.angular_joints {
        let (a, b) = (&world.bodies[j.body_a], &world.bodies[j.body_b]);
        let (frame_a, frame_b) = match j.model {
            JointModel::Fixed { frame_a, frame_b }
            | JointModel::Hinge {
                frame_a, frame_b, ..
            }
            | JointModel::RollLock { frame_a, frame_b } => (frame_a, frame_b),
            JointModel::Ball => unreachable!("validated angular model"),
        };
        let fa = Frame::new(a.pose.orientation.compose(frame_a)).map_err(|e| e.to_string())?;
        let fb = Frame::new(b.pose.orientation.compose(frame_b)).map_err(|e| e.to_string())?;
        let w = b.angular_velocity_radps - a.angular_velocity_radps;
        let e = match j.model {
            JointModel::Fixed { .. } => w.length(),
            JointModel::RollLock { .. } => crate::angular::roll_constraint(fa, fb)?.0.dot(w).abs(),
            JointModel::Hinge { limits, .. } => {
                let mut e = w.dot(fa.tangent()).hypot(w.dot(fa.normal()));
                if let Some(l) = limits {
                    let theta = j.angle_rad(&world.bodies)?;
                    let v = w.dot(fa.lateral());
                    if (theta <= l.min_rad && v < 0.) || (theta >= l.max_rad && v > 0.) {
                        e = e.max(v.abs());
                    }
                }
                e
            }
            JointModel::Ball => 0.,
        };
        observe_max(&mut result.max_angular_constraint_speed_radps, e)?;
    }
    for g in guides {
        let b = world
            .bodies
            .get(g.body)
            .ok_or("invalid diagnostic guide body")?;
        let r = b.frame().local_to_world(g.local_point_m);
        let s =
            crate::solver::project_station(track, b.world_point(g.local_point_m)?, g.station_m)?;
        let target = track.sample_at_distance(s).map_err(|e| e.to_string())?;
        let v = b.point_velocity(r);
        observe_max(
            &mut result.max_guide_transverse_speed_mps,
            v.dot(target.frame.normal())
                .hypot(v.dot(target.frame.lateral())),
        )?;
    }
    for a in attitudes {
        let b = world
            .bodies
            .get(a.body)
            .ok_or("invalid diagnostic attitude body")?;
        let r = b.frame().local_to_world(a.local_point_m);
        let s =
            crate::solver::project_station(track, b.world_point(a.local_point_m)?, a.station_m)?;
        let target = track.sample_at_distance(s).map_err(|e| e.to_string())?;
        let derivative = crate::solver::track_frame_derivative(track, s, false)?;
        let rail =
            Frame::new(b.pose.orientation.compose(a.local_frame)).map_err(|e| e.to_string())?;
        let (axis, _) = crate::angular::roll_constraint(target.frame, rail)?;
        let delta = b.pose.position_m + r - target.position_m;
        let projection = 1. - delta.dot(derivative.cross(target.frame.tangent()));
        if !projection.is_finite() || projection <= 1e-6 {
            return Err("diagnostic rail station projection is singular".into());
        }
        let linear = target.frame.tangent() * (-axis.dot(derivative) / projection);
        let angular = axis + r.cross(linear);
        observe_max(
            &mut result.max_rail_twist_speed_radps,
            (linear.dot(b.linear_velocity_mps) + angular.dot(b.angular_velocity_radps)).abs(),
        )?;
    }
    Ok(result)
}
