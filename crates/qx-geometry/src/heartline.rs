//! Reference-point geometry and design-speed kinematics, independent of vehicles.
//! Local offset XYZ means longitudinal T, normal N, lateral L (metres).
use crate::{
    Result,
    authoring::SectionId,
    ensure,
    math::{Frame, Vec3},
    solver::{CenterlineSample, GeneratedTrack},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredReference {
    #[default]
    TrackCenterline,
    /// Requires an explicit orientation field and a compatible inverse solution.
    RiderReference,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OffsetProfile {
    Constant {
        offset_m: Vec3,
    },
    /// Smooth C2 offsets over the entire authored path's arc coordinate s/L.
    Quintic {
        start_m: Vec3,
        end_m: Vec3,
    },
}
impl Default for OffsetProfile {
    fn default() -> Self {
        Self::Constant {
            offset_m: Vec3::ZERO,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct OffsetJet {
    pub value_m: Vec3,
    pub first: Vec3,
    pub second_per_m: Vec3,
}
impl OffsetProfile {
    pub fn validate(&self) -> Result<()> {
        let values = match *self {
            Self::Constant { offset_m } => [offset_m, offset_m],
            Self::Quintic { start_m, end_m } => [start_m, end_m],
        };
        ensure(
            values.iter().all(|v| v.is_finite() && v.length() <= 100.),
            "reference offsets must be finite with norm <= 100 m",
        )
    }
    pub fn evaluate(&self, s: f64, length: f64) -> Result<OffsetJet> {
        self.validate()?;
        ensure(
            length.is_finite() && length > 0. && s.is_finite() && (0.0..=length).contains(&s),
            "offset coordinate must lie in a positive finite path length",
        )?;
        Ok(match *self {
            Self::Constant { offset_m } => OffsetJet {
                value_m: offset_m,
                first: Vec3::ZERO,
                second_per_m: Vec3::ZERO,
            },
            Self::Quintic { start_m, end_m } => {
                let u = s / length;
                let delta = end_m - start_m;
                OffsetJet {
                    value_m: start_m + delta * (u.powi(3) * (10. + u * (-15. + 6. * u))),
                    first: delta * (30. * u.powi(2) * (1. - u).powi(2) / length),
                    second_per_m: delta * (60. * u * (1. - u) * (1. - 2. * u) / length.powi(2)),
                }
            }
        })
    }
}

/// Derivatives with respect to the authored reference arc coordinate s.
#[derive(Clone, Copy, Debug)]
pub struct PathJet {
    pub position_m: Vec3,
    pub first: Vec3,
    pub second_per_m: Vec3,
}
#[derive(Clone, Copy, Debug)]
pub struct OrientationJet {
    pub frame: Frame,
    pub omega_per_m: Vec3,
    pub omega_derivative_per_m2: Vec3,
}
pub fn offset_path(
    path: PathJet,
    orientation: OrientationJet,
    offset: OffsetJet,
) -> Result<PathJet> {
    let r = orientation.frame.local_to_world(offset.value_m);
    let dr = orientation.frame.local_to_world(offset.first);
    let ddr = orientation.frame.local_to_world(offset.second_per_m);
    let w = orientation.omega_per_m;
    let dw = orientation.omega_derivative_per_m2;
    let result = PathJet {
        position_m: path.position_m + r,
        first: path.first + w.cross(r) + dr,
        second_per_m: path.second_per_m
            + dw.cross(r)
            + w.cross(w.cross(r))
            + w.cross(dr) * 2.
            + ddr,
    };
    validate_jet(result)?;
    Ok(result)
}
fn validate_jet(jet: PathJet) -> Result<()> {
    ensure(
        [jet.position_m, jet.first, jet.second_per_m]
            .iter()
            .all(|v| v.is_finite()),
        "reference path derivatives overflow or contain nonfinite values",
    )?;
    ensure(
        jet.position_m.length() <= 1e9,
        "reference position exceeds 1e9 m",
    )?;
    ensure(
        jet.first.length() > 1e-9,
        "offset reference path is singular (zero/tiny tangent); reduce offsets or revise curvature/twist",
    )
}
/// Exact algebra with a *supplied* orientation jet. It does not infer banking
/// from a rider curve. Track tangent and frame must remain mechanically compatible.
pub fn track_from_rider(
    rider: PathJet,
    orientation: OrientationJet,
    offset: OffsetJet,
) -> Result<PathJet> {
    let negative = OffsetJet {
        value_m: -offset.value_m,
        first: -offset.first,
        second_per_m: -offset.second_per_m,
    };
    let track = offset_path(rider, orientation, negative)?;
    let t = track.first.normalized()?;
    ensure(
        (t - orientation.frame.tangent()).length() <= 1e-6,
        "rider-to-track inverse conflicts with supplied orientation: resulting track tangent is not frame T; supply a compatible orientation field or author the track centerline",
    )?;
    let dt = (track.second_per_m - t * t.dot(track.second_per_m)) / track.first.length();
    ensure(
        (dt - orientation.omega_per_m.cross(t)).length() <= 1e-5,
        "rider-to-track inverse curvature conflicts with supplied orientation; constrained solve unavailable",
    )?;
    Ok(track)
}

#[derive(Clone, Copy, Debug)]
pub struct PointMotion {
    pub velocity_mps: Vec3,
    pub acceleration_mps2: Vec3,
}
/// Rigid-point equation, including relative motion expressed in the rotating frame.
/// Local relative velocity/acceleration must be actual time derivatives of r.
pub fn point_motion(
    reference: PointMotion,
    frame: Frame,
    omega_radps: Vec3,
    alpha_radps2: Vec3,
    offset_m: Vec3,
    relative_velocity_mps: Vec3,
    relative_acceleration_mps2: Vec3,
) -> Result<PointMotion> {
    let r = frame.local_to_world(offset_m);
    let v = frame.local_to_world(relative_velocity_mps);
    let a = frame.local_to_world(relative_acceleration_mps2);
    let result = PointMotion {
        velocity_mps: reference.velocity_mps + omega_radps.cross(r) + v,
        acceleration_mps2: reference.acceleration_mps2
            + alpha_radps2.cross(r)
            + omega_radps.cross(omega_radps.cross(r))
            + omega_radps.cross(v) * 2.
            + a,
    };
    ensure(
        result.velocity_mps.is_finite() && result.acceleration_mps2.is_finite(),
        "nonfinite point kinematics",
    )?;
    Ok(result)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReferencePoint {
    pub section_id: SectionId,
    pub source_distance_m: f64,
    pub path_distance_m: f64,
    pub position_m: Vec3,
    pub tangent: Vec3,
    pub curvature_per_m: f64,
    pub speed_mps: f64,
    pub acceleration_mps2: Vec3,
    pub normal_g: f64,
    pub lateral_g: f64,
    pub longitudinal_g: f64,
    pub frame: Frame,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceTrack {
    pub role: ReferencePathRole,
    /// Duplicate source stations at knots intentionally retain both side limits.
    pub points: Vec<ReferencePoint>,
    pub length_m: f64,
    pub warnings: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferencePathRole {
    RiderReference,
    TrackCenterline,
}

/// Differentiate a degree-four local interpolant of frame axes. All stencils
/// remain inside one smooth solver range. Coordinates are scaled to control
/// roundoff; world position is never differentiated. Smooth endpoints are one-sided.
fn frame_jet(samples: &[CenterlineSample], s: f64) -> Result<(Vec3, Vec3, OrientationJet)> {
    ensure(
        samples.len() >= 5,
        "reference derivatives require at least five smooth samples",
    )?;
    let i = samples.partition_point(|p| p.distance_m < s);
    let start = i.saturating_sub(2).min(samples.len() - 5);
    let nodes = &samples[start..start + 5];
    let scale = nodes[4].distance_m - nodes[0].distance_m;
    ensure(
        scale > 1e-10,
        "reference derivative interval too small; increase sampling step or move origin",
    )?;
    let xs: [f64; 5] = std::array::from_fn(|i| (nodes[i].distance_m - s) / scale);
    let mut axes = [[Vec3::ZERO; 3]; 3];
    for j in 0..5 {
        // Polynomial coefficients for the jth Lagrange basis, ascending order.
        let mut c = [0.; 5];
        c[0] = 1.;
        let mut degree = 0;
        let mut denominator = 1.;
        for k in 0..5 {
            if j == k {
                continue;
            }
            denominator *= xs[j] - xs[k];
            let mut next = [0.; 5];
            for (power, coefficient) in c.iter().enumerate().take(degree + 1) {
                next[power] -= coefficient * xs[k];
                next[power + 1] += coefficient;
            }
            c = next;
            degree += 1;
        }
        ensure(
            denominator.abs() > 1e-16,
            "ill-conditioned reference derivative stencil",
        )?;
        let weights = [
            c[0] / denominator,
            c[1] / (denominator * scale),
            2. * c[2] / (denominator * scale * scale),
        ];
        let frame = nodes[j].frame;
        for (a, value) in [frame.tangent(), frame.normal(), frame.lateral()]
            .into_iter()
            .enumerate()
        {
            for order in 0..3 {
                axes[a][order] = axes[a][order] + value * weights[order];
            }
        }
    }
    let left = i.saturating_sub(1).min(samples.len() - 2);
    let fraction = ((s - samples[left].distance_m)
        / (samples[left + 1].distance_m - samples[left].distance_m))
        .clamp(0., 1.);
    let frame = Frame::new(
        samples[left]
            .frame
            .orientation()
            .slerp(samples[left + 1].frame.orientation(), fraction)?,
    )?;
    let mut w = Vec3::ZERO;
    let mut dw = Vec3::ZERO;
    for axis in axes {
        w = w + axis[0].cross(axis[1]) * 0.5;
        dw = dw + axis[0].cross(axis[2]) * 0.5;
    }
    Ok((
        frame.tangent(),
        axes[0][1],
        OrientationJet {
            frame,
            omega_per_m: w,
            omega_derivative_per_m2: dw,
        },
    ))
}

pub fn generate_reference(
    track: &GeneratedTrack,
    profile: &OffsetProfile,
    authored: AuthoredReference,
    gravity_mps2: f64,
) -> Result<ReferenceTrack> {
    profile.validate()?;
    ensure(
        gravity_mps2.is_finite() && gravity_mps2 > 0.,
        "reference gravity must be positive and finite",
    )?;
    ensure(
        track.length_m() > 0. && !track.smooth_ranges.is_empty(),
        "reference calculation requires generated smooth ranges",
    )?;
    let mut result = ReferenceTrack {
        role: match authored {
            AuthoredReference::TrackCenterline => ReferencePathRole::RiderReference,
            AuthoredReference::RiderReference => ReferencePathRole::TrackCenterline,
        },
        points: vec![],
        length_m: 0.,
        warnings: vec![],
    };
    for range in &track.smooth_ranges {
        ensure(
            range.start_sample < range.end_sample && range.end_sample < track.samples.len(),
            "invalid reference sample range",
        )?;
        let samples = &track.samples[range.start_sample..=range.end_sample];
        let speed = (samples[1].distance_m - samples[0].distance_m)
            / (samples[1].design_time_s - samples[0].design_time_s);
        let jet_at = |s: f64, position_m: Vec3| -> Result<(PathJet, OrientationJet)> {
            let (t, dt, orientation) = frame_jet(samples, s)?;
            let path = PathJet {
                position_m,
                first: t,
                second_per_m: dt,
            };
            let offset = profile.evaluate(s.min(track.length_m()), track.length_m())?;
            let jet = match authored {
                AuthoredReference::TrackCenterline => offset_path(path, orientation, offset)?,
                AuthoredReference::RiderReference => track_from_rider(path, orientation, offset)?,
            };
            Ok((jet, orientation))
        };
        let mut previous_speed_factor = None;
        for (index, sample) in samples.iter().enumerate() {
            let (jet, orientation) = jet_at(sample.distance_m, sample.position_m)?;
            let factor = jet.first.length();
            if let Some(previous_factor) = previous_speed_factor {
                let a = &samples[index - 1];
                let midpoint = (a.distance_m + sample.distance_m) * 0.5;
                let midpoint_factor = jet_at(midpoint, (a.position_m + sample.position_m) * 0.5)?
                    .0
                    .first
                    .length();
                result.length_m += (sample.distance_m - a.distance_m)
                    * (previous_factor + 4. * midpoint_factor + factor)
                    / 6.;
            }
            previous_speed_factor = Some(factor);
            let acceleration = jet.second_per_m * speed.powi(2);
            let specific = acceleration + Vec3::Y * gravity_mps2;
            result.points.push(ReferencePoint {
                section_id: range.section_id,
                source_distance_m: sample.distance_m,
                path_distance_m: result.length_m,
                position_m: jet.position_m,
                tangent: jet.first / factor,
                curvature_per_m: jet.first.cross(jet.second_per_m).length() / factor.powi(3),
                speed_mps: speed * factor,
                acceleration_mps2: acceleration,
                normal_g: specific.dot(orientation.frame.normal()) / gravity_mps2,
                lateral_g: specific.dot(orientation.frame.lateral()) / gravity_mps2,
                longitudinal_g: specific.dot(orientation.frame.tangent()) / gravity_mps2,
                frame: orientation.frame,
            });
        }
    }
    for connection in &track.connections {
        if connection.reference_speed_gap_mps.abs() > 1e-8
            || connection.curvature_vector_gap_per_m > 1e-8
            || connection.twist_rate_gap_rad_per_m.abs() > 1e-8
        {
            result.warnings.push(format!("section {}: rate/speed jump; reference derivatives and design loads are one-sided, impulsive boundary acceleration is undefined", connection.section_id.0));
        }
    }
    if track
        .diagnostics
        .iter()
        .any(|d| d.code == "channel_discontinuity" || d.code == "spline_curvature_jump")
    {
        result.warnings.push("internal channel/curve rate discontinuities: one-sided derivatives; no finite impulsive load is claimed".into());
    }
    Ok(result)
}
