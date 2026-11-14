//! Experimental, velocity-only initial conditions for the production mechanism.
//! Sparse dual coordinate descent minimizes the mass metric at a frozen pose.
//! Equality rows have free multipliers; reached limits use nonnegative multipliers
//! for Jv >= b. The lead speed is an affine equality, never a rescaling operation.
use crate::{Body, Result, TrackAttitude, TrackGuide, World, constraint_geometry as geometry};
use qx_document::train::mechanism::JointModel;
use qx_geometry::{math::Vec3, solver::GeneratedTrack};

pub const MAX_BODIES: usize = 64;
pub const MAX_ROWS: usize = 1024;
pub const MAX_SWEEPS: u32 = 20_000;
pub const MAX_ROW_UPDATES: usize = 10_000_000;
/// Metre/s for point rows, rad/s for angular rows, metre/s for the speed row.
pub const RESIDUAL_TOLERANCE: f64 = 1e-9;
pub const STATIONARITY_TOLERANCE: f64 = 1e-8;
const SEATING_TOLERANCE: f64 = 1e-5;
const REACHED_LIMIT_TOLERANCE: f64 = 1e-8;

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub bodies: usize,
    pub rows: usize,
    pub sweeps: u32,
    pub seed_max_residual: f64,
    pub max_residual: f64,
    /// Reconstructed v_seed + M^-1 J^T lambda versus the accumulated velocities;
    /// maximum linear/angular component error (m/s or rad/s, respectively).
    pub max_stationarity_error: f64,
    /// Index in the supplied TrackGuide list. Its material point, not its COM,
    /// defines the signed tangent speed. Playback chooses the foremost running gear.
    pub reference_guide: usize,
    /// Frozen physical body/local point and projected station of the speed row.
    pub reference: TrackGuide,
    pub requested_speed_mps: f64,
    pub achieved_speed_mps: f64,
    /// 1/2 (v-v_seed)^T M (v-v_seed), not work by a physical device.
    pub correction_metric_j: f64,
}

#[derive(Clone)]
struct Term {
    body: usize,
    linear: Vec3,
    angular: Vec3,
    response_linear: Vec3,
    response_angular: Vec3,
}
impl Term {
    fn new(bodies: &[Body], body: usize, linear: Vec3, angular: Vec3) -> Self {
        Self {
            body,
            linear,
            angular,
            response_linear: linear / bodies[body].mass.mass_kg,
            response_angular: bodies[body].inv_inertia(angular),
        }
    }
    fn point(bodies: &[Body], body: usize, arm: Vec3, axis: Vec3) -> Self {
        Self::new(bodies, body, axis, arm.cross(axis))
    }
    fn value(&self, bodies: &[Body]) -> f64 {
        self.linear.dot(bodies[self.body].linear_velocity_mps)
            + self.angular.dot(bodies[self.body].angular_velocity_radps)
    }
}
struct Row {
    terms: Vec<Term>,
    target: f64,
    inequality: bool,
    label: String,
    denominator: f64,
    multiplier: f64,
}
impl Row {
    fn new(terms: Vec<Term>, target: f64, inequality: bool, label: String) -> Result<Self> {
        let denominator: f64 = terms
            .iter()
            .map(|t| t.linear.dot(t.response_linear) + t.angular.dot(t.response_angular))
            .sum();
        if !denominator.is_finite() || denominator <= 1e-15 {
            return Err(format!(
                "experimental velocity initialization: singular mass response at {label}"
            ));
        }
        Ok(Self {
            terms,
            target,
            inequality,
            label,
            denominator,
            multiplier: 0.,
        })
    }
    fn value(&self, bodies: &[Body]) -> f64 {
        self.terms.iter().map(|t| t.value(bodies)).sum::<f64>() - self.target
    }
    fn residual(&self, bodies: &[Body]) -> Result<f64> {
        let value = self.value(bodies);
        if !value.is_finite() || !self.multiplier.is_finite() {
            return Err(format!(
                "experimental velocity initialization overflow at {}",
                self.label
            ));
        }
        // Dual feasibility is maintained by clamping. Also require complementarity:
        // an inward velocity is admissible only when the limit impulse is zero.
        Ok(if self.inequality && self.multiplier == 0. {
            (-value).max(0.)
        } else {
            value.abs()
        })
    }
    fn update(&mut self, bodies: &mut [Body]) {
        let candidate = self.multiplier - self.value(bodies) / self.denominator;
        let next = if self.inequality {
            candidate.max(0.)
        } else {
            candidate
        };
        let impulse = next - self.multiplier;
        self.multiplier = next;
        for t in &self.terms {
            let b = &mut bodies[t.body];
            b.linear_velocity_mps = b.linear_velocity_mps + t.response_linear * impulse;
            b.angular_velocity_radps = b.angular_velocity_radps + t.response_angular * impulse;
        }
    }
}

fn seated(error: f64, label: &str) -> Result<()> {
    if !error.is_finite() || error.abs() > SEATING_TOLERANCE {
        return Err(format!(
            "experimental velocity initialization requires seated poses: {label} gap {error:e} exceeds {SEATING_TOLERANCE:e}"
        ));
    }
    Ok(())
}
fn maximum(rows: &[Row], bodies: &[Body]) -> Result<(f64, usize)> {
    let mut max = (0., 0);
    for (i, r) in rows.iter().enumerate() {
        let e = r.residual(bodies)?;
        if e > max.0 {
            max = (e, i);
        }
    }
    Ok(max)
}

/// Transactional bounded projection. Only velocities change on success; poses,
/// forces, torque, guide hints and attitude hints remain bit-identical. No dt is
/// accepted and no integration/motor/device code is called. Failure publishes no
/// candidate. Redundant rows are allowed; inconsistency/ill-conditioning that
/// cannot meet the residual budget returns an explicit error, not a best effort.
///
/// The caller supplies the lead running-gear guide identity. Nonzero prescribed
/// mechanism velocities are not currently a production joint capability;
/// MechanicalProfile remains rejected by World::validate. This solver nevertheless
/// solves the nonhomogeneous lead-speed row together with limits, without scaling.
pub fn initialize(
    world: &mut World,
    track: &GeneratedTrack,
    guides: &[TrackGuide],
    attitudes: &[TrackAttitude],
    reference_guide: usize,
    requested_speed_mps: f64,
    circuit: bool,
) -> Result<Report> {
    if world.bodies.len() > MAX_BODIES
        || guides.len() > MAX_ROWS / 2
        || attitudes.len() > MAX_BODIES
    {
        return Err(format!(
            "experimental velocity initialization bounded to {MAX_BODIES} bodies and {MAX_ROWS} rows"
        ));
    }
    world.validate()?;
    let row_bound = 3 * world.joints.len()
        + 4 * world.angular_joints.len()
        + 2 * (world.guides.len() + guides.len())
        + attitudes.len()
        + 1;
    if row_bound > MAX_ROWS {
        return Err(format!(
            "experimental velocity initialization row budget {row_bound} exceeds {MAX_ROWS}"
        ));
    }
    if !requested_speed_mps.is_finite() || requested_speed_mps.abs() > 100. {
        return Err("experimental lead speed must be finite and within +/-100 m/s".into());
    }
    if reference_guide >= guides.len()
        || track.samples.len() < 2
        || !track.length_m().is_finite()
        || track.length_m() <= 0.
        || track.samples[0].distance_m != 0.
        || track
            .samples
            .iter()
            .any(|s| !s.position_m.is_finite() || !s.distance_m.is_finite())
        || track
            .samples
            .windows(2)
            .any(|s| s[1].distance_m <= s[0].distance_m)
    {
        return Err("invalid experimental lead reference or generated track".into());
    }
    if circuit
        && !qx_geometry::topology::diagnose(track)
            .map_err(|e| e.to_string())?
            .valid
    {
        return Err("experimental circuit initialization requires verified closure".into());
    }
    let valid_point = |body: usize, local: Vec3, station: f64| -> Result<()> {
        if body >= world.bodies.len()
            || !local.is_finite()
            || !station.is_finite()
            || station < 0.
            || station > track.length_m()
        {
            return Err("invalid experimental guide/attitude body, point or station".into());
        }
        Ok(())
    };
    let mut rows = Vec::new();
    for (i, j) in world.joints.iter().enumerate() {
        let (a, b) = (&world.bodies[j.body_a], &world.bodies[j.body_b]);
        let ra = a.frame().local_to_world(j.anchor_a_m);
        let rb = b.frame().local_to_world(j.anchor_b_m);
        seated(
            ((b.pose.position_m + rb) - (a.pose.position_m + ra)).length(),
            &format!("ball {i}"),
        )?;
        for axis in geometry::BALL_AXES {
            rows.push(Row::new(
                vec![
                    Term::point(&world.bodies, j.body_a, ra, -axis),
                    Term::point(&world.bodies, j.body_b, rb, axis),
                ],
                0.,
                false,
                format!("ball {i}"),
            )?);
        }
    }
    for (i, j) in world.angular_joints.iter().enumerate() {
        seated(j.error(&world.bodies)?, &format!("angular {i}"))?;
        let (axes, count) = j.bilateral_rows(&world.bodies)?;
        let mut push = |axis: Vec3, inequality: bool| -> Result<()> {
            rows.push(Row::new(
                vec![
                    Term::new(&world.bodies, j.body_a, Vec3::ZERO, -axis),
                    Term::new(&world.bodies, j.body_b, Vec3::ZERO, axis),
                ],
                0.,
                inequality,
                format!(
                    "angular {i}{}",
                    if inequality { " reached limit" } else { "" }
                ),
            )?);
            Ok(())
        };
        for (axis, _) in axes.into_iter().take(count) {
            push(axis, false)?;
        }
        if let JointModel::Hinge {
            limits: Some(l), ..
        } = j.model
        {
            let theta = j.angle_rad(&world.bodies)?;
            if theta < l.min_rad - REACHED_LIMIT_TOLERANCE
                || theta > l.max_rad + REACHED_LIMIT_TOLERANCE
            {
                return Err(format!(
                    "experimental velocity initialization: angular {i} is outside its positional limits; re-seat the mechanism"
                ));
            }
            let axis = j
                .frames(&world.bodies[j.body_a], &world.bodies[j.body_b])?
                .0
                .lateral();
            if theta <= l.min_rad + REACHED_LIMIT_TOLERANCE {
                push(axis, true)?;
            }
            if theta >= l.max_rad - REACHED_LIMIT_TOLERANCE {
                push(-axis, true)?;
            }
        }
    }
    for (i, g) in world.guides.iter().enumerate() {
        let b = &world.bodies[g.body];
        let arm = b.frame().local_to_world(g.local_point_m);
        for n in geometry::line_normals(g.line_direction)? {
            seated(
                (b.pose.position_m + arm - g.line_origin_m).dot(n),
                &format!("line guide {i}"),
            )?;
            rows.push(Row::new(
                vec![Term::point(&world.bodies, g.body, arm, n)],
                0.,
                false,
                format!("line guide {i}"),
            )?);
        }
    }
    let mut reference = None;
    let mut reference_hint = guides[reference_guide].clone();
    let mut projected_guides = Vec::with_capacity(guides.len());
    for (i, g) in guides.iter().enumerate() {
        valid_point(g.body, g.local_point_m, g.station_m)?;
        let b = &world.bodies[g.body];
        let arm = b.frame().local_to_world(g.local_point_m);
        let point = b.pose.position_m + arm;
        let s = crate::solver::project_station_topology(track, point, g.station_m, circuit)?;
        let sample = track.sample_at_distance(s).map_err(|e| e.to_string())?;
        seated(
            (point - sample.position_m).length(),
            &format!("track guide {i}"),
        )?;
        projected_guides.push((g.body, arm, sample.frame.tangent(), s));
        for n in geometry::track_normals(sample.frame) {
            rows.push(Row::new(
                vec![Term::point(&world.bodies, g.body, arm, n)],
                0.,
                false,
                format!("track guide {i}"),
            )?);
        }
        if i == reference_guide {
            reference_hint.station_m = s;
            reference = Some(Term::point(
                &world.bodies,
                g.body,
                arm,
                sample.frame.tangent(),
            ));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for (i, a) in attitudes.iter().enumerate() {
        valid_point(a.body, a.local_point_m, a.station_m)?;
        let q = a.local_frame;
        if !seen.insert(a.body)
            || !q.w.hypot(q.x).hypot(q.y).hypot(q.z).is_finite()
            || (q.w.hypot(q.x).hypot(q.y).hypot(q.z) - 1.).abs() > 1e-8
        {
            return Err("invalid/duplicate experimental attitude frame".into());
        }
        let b = &world.bodies[a.body];
        let s = crate::solver::project_station_topology(
            track,
            b.world_point(a.local_point_m)?,
            a.station_m,
            circuit,
        )?;
        let sample = track.sample_at_distance(s).map_err(|e| e.to_string())?;
        let row = geometry::rail_twist(
            b,
            a,
            sample.frame,
            sample.position_m,
            crate::solver::track_frame_derivative(track, s, circuit)?,
        )?;
        seated(row.error, &format!("rail twist {i}"))?;
        rows.push(Row::new(
            vec![Term::new(&world.bodies, a.body, row.linear, row.angular)],
            0.,
            false,
            format!("rail twist {i}"),
        )?);
    }
    let reference = reference.ok_or("missing experimental speed reference")?;
    rows.push(Row::new(
        vec![reference.clone()],
        requested_speed_mps,
        false,
        "lead guide speed".into(),
    )?);
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "experimental velocity initialization exceeds {MAX_ROWS} rows"
        ));
    }
    let seed_max_residual = maximum(&rows, &world.bodies)?.0;
    let mut candidate = world.clone();
    let budget = MAX_SWEEPS.min((MAX_ROW_UPDATES / rows.len()) as u32);
    for sweep in 0..=budget {
        if sweep % 8 == 0 || sweep == budget {
            let (max_residual, worst) = maximum(&rows, &candidate.bodies)?;
            if max_residual <= RESIDUAL_TOLERANCE {
                candidate.validate()?;
                // Coordinate updates preserve stationarity algebraically. Check
                // it independently of their accumulated floating-point path,
                // especially for redundant/poorly conditioned mechanisms.
                let mut reconstructed: Vec<_> = world
                    .bodies
                    .iter()
                    .map(|b| (b.linear_velocity_mps, b.angular_velocity_radps))
                    .collect();
                for row in &rows {
                    for term in &row.terms {
                        let state = &mut reconstructed[term.body];
                        state.0 = state.0 + term.response_linear * row.multiplier;
                        state.1 = state.1 + term.response_angular * row.multiplier;
                    }
                }
                let mut max_stationarity_error = 0_f64;
                for ((linear, angular), body) in reconstructed.iter().zip(&candidate.bodies) {
                    for d in [
                        *linear - body.linear_velocity_mps,
                        *angular - body.angular_velocity_radps,
                    ] {
                        if !d.is_finite() {
                            return Err("experimental stationarity reconstruction overflow".into());
                        }
                        max_stationarity_error = max_stationarity_error
                            .max(d.x.abs())
                            .max(d.y.abs())
                            .max(d.z.abs());
                    }
                }
                if max_stationarity_error > STATIONARITY_TOLERANCE {
                    return Err(format!(
                        "experimental velocity initialization stationarity error {max_stationarity_error:e} exceeds {STATIONARITY_TOLERANCE:e}; poses unchanged"
                    ));
                }
                for (i, (body, arm, tangent, station)) in projected_guides.iter().enumerate() {
                    let along = candidate.bodies[*body].point_velocity(*arm).dot(*tangent);
                    if !circuit
                        && ((*station <= 1e-9 && along < -RESIDUAL_TOLERANCE)
                            || (*station >= track.length_m() - 1e-9 && along > RESIDUAL_TOLERANCE))
                    {
                        return Err(format!(
                            "experimental velocity initialization: track guide {i} would immediately leave an open endpoint; choose an interior station or an inward speed"
                        ));
                    }
                }
                let correction_metric_j = candidate
                    .bodies
                    .iter()
                    .zip(&world.bodies)
                    .map(|(b, seed)| {
                        let dv = b.linear_velocity_mps - seed.linear_velocity_mps;
                        let dw = b
                            .frame()
                            .world_to_local(b.angular_velocity_radps - seed.angular_velocity_radps);
                        let inertia = b.mass.principal_inertia_kg_m2;
                        0.5 * (b.mass.mass_kg * dv.dot(dv)
                            + inertia.x * dw.x * dw.x
                            + inertia.y * dw.y * dw.y
                            + inertia.z * dw.z * dw.z)
                    })
                    .sum::<f64>();
                if !correction_metric_j.is_finite() {
                    return Err("experimental correction metric overflow".into());
                }
                let report = Report {
                    bodies: world.bodies.len(),
                    rows: rows.len(),
                    sweeps: sweep,
                    seed_max_residual,
                    max_residual,
                    max_stationarity_error,
                    reference_guide,
                    reference: reference_hint,
                    requested_speed_mps,
                    achieved_speed_mps: reference.value(&candidate.bodies),
                    correction_metric_j,
                };
                *world = candidate;
                return Ok(report);
            }
            if sweep == budget {
                return Err(format!(
                    "experimental velocity initialization infeasible or unconverged: {} residual {max_residual:e} > {RESIDUAL_TOLERANCE:e} after {sweep} sweeps ({} bodies, {} rows); poses unchanged",
                    rows[worst].label,
                    world.bodies.len(),
                    rows.len()
                ));
            }
        }
        for row in &mut rows {
            row.update(&mut candidate.bodies);
        }
    }
    unreachable!("finite budget always returns a result")
}
