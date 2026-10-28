use crate::{Body, Result};
use qx_geometry::{math::Vec3, solver::GeneratedTrack};

pub const FIXED_STEP_SECONDS: f64 = 1.0 / 240.0;

/// A spherical mechanical connection: anchors coincide, all relative rotations are free.
#[derive(Clone, Debug, PartialEq)]
pub struct BallJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub anchor_a_m: Vec3,
    pub anchor_b_m: Vec3,
}

/// Bilateral point-on-infinite-line guide. This is an idealized captive guide,
/// not a wheel, unilateral contact, or a physical running surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Guide {
    pub body: usize,
    pub local_point_m: Vec3,
    pub line_origin_m: Vec3,
    pub line_direction: Vec3,
}

/// A captive point guided along a generated centerline. Station is a local
/// projection hint, not an independently integrated vehicle position.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackGuide {
    pub body: usize,
    pub local_point_m: Vec3,
    pub station_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    pub max_angular_error_rad: f64,
    /// Average angular constraint + motor torque on body B, in angular-joint order.
    pub angular_torque_on_b_nm: Vec<Vec3>,
    pub max_joint_error_m: f64,
    pub max_guide_error_m: f64,
    /// Average world-space constraint force on body B over the outer step.
    pub joint_force_on_b_n: Vec<Vec3>,
    /// Average world-space force on each guided body over the outer step.
    pub guide_force_n: Vec<Vec3>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub bodies: Vec<Body>,
    pub joints: Vec<BallJoint>,
    pub angular_joints: Vec<crate::AngularJoint>,
    pub guides: Vec<Guide>,
    pub gravity_mps2: Vec3,
    /// Deterministic sequential impulse iterations per substep, 1..=128.
    pub iterations: u32,
    /// Fixed outer step is always 1/240 s, divided into 1..=32 substeps.
    pub substeps: u32,
}

impl World {
    pub fn validate(&self) -> Result<()> {
        if self.bodies.is_empty()
            || self.bodies.len() > 1024
            || self.joints.len() > 4096
            || self.angular_joints.len() > 4096
            || self.guides.len() > 4096
            || !(1..=128).contains(&self.iterations)
            || !(1..=32).contains(&self.substeps)
            || !self.gravity_mps2.is_finite()
        {
            return Err("invalid world size, gravity, iteration count, or substep count".into());
        }
        for b in &self.bodies {
            b.validate()?;
        }
        for j in &self.angular_joints {
            j.validate(self.bodies.len())?;
        }
        for j in &self.joints {
            if j.body_a >= self.bodies.len()
                || j.body_b >= self.bodies.len()
                || j.body_a == j.body_b
                || !j.anchor_a_m.is_finite()
                || !j.anchor_b_m.is_finite()
            {
                return Err("invalid ball joint body or anchor".into());
            }
        }
        for g in &self.guides {
            if g.body >= self.bodies.len()
                || !g.local_point_m.is_finite()
                || !g.line_origin_m.is_finite()
                || !g.line_direction.is_finite()
                || (g.line_direction.length() - 1.0).abs() > 1e-8
            {
                return Err("invalid guide body, point, or unit direction".into());
            }
        }
        Ok(())
    }

    /// Advances exactly one fixed step. Invalid or nonfinite results leave the world unchanged.
    /// Persistent applied forces are held constant over the step; callers clear them explicitly.
    pub fn step(&mut self) -> Result<Diagnostics> {
        self.validate()?;
        let mut next = self.clone();
        let diagnostics = next.step_inner(None)?;
        next.validate()?;
        *self = next;
        Ok(diagnostics)
    }

    /// Transactionally step independent bodies with captive, frictionless point
    /// guides on the generated centerline. Open endpoints reject an attempted
    /// escape; the caller must provide a different boundary/operation model.
    pub fn step_on_track(
        &mut self,
        track: &GeneratedTrack,
        guides: &mut [TrackGuide],
    ) -> Result<Diagnostics> {
        self.validate()?;
        if track.samples.len() < 2
            || !track.length_m().is_finite()
            || track.length_m() <= 0.0
            || guides.len() > 4096
            || guides.iter().any(|g| {
                g.body >= self.bodies.len()
                    || !g.local_point_m.is_finite()
                    || !g.station_m.is_finite()
                    || g.station_m < 0.0
                    || g.station_m > track.length_m()
            })
        {
            return Err("invalid track guide or generated track".into());
        }
        if track
            .samples
            .iter()
            .any(|p| !p.distance_m.is_finite() || !p.position_m.is_finite())
            || track.samples[0].distance_m != 0.0
            || track
                .samples
                .windows(2)
                .any(|pair| pair[1].distance_m <= pair[0].distance_m)
        {
            return Err("invalid generated track samples".into());
        }
        let mut next = self.clone();
        let mut next_guides = guides.to_vec();
        let diagnostics = next.step_inner(Some((track, &mut next_guides)))?;
        next.validate()?;
        *self = next;
        guides.clone_from_slice(&next_guides);
        Ok(diagnostics)
    }

    fn step_inner(
        &mut self,
        mut track_guidance: Option<(&GeneratedTrack, &mut [TrackGuide])>,
    ) -> Result<Diagnostics> {
        let dt = FIXED_STEP_SECONDS / f64::from(self.substeps);
        let mut joint_impulse = vec![Vec3::ZERO; self.joints.len()];
        let mut angular_impulse = vec![Vec3::ZERO; self.angular_joints.len()];
        let track_count = track_guidance.as_ref().map_or(0, |(_, g)| g.len());
        let mut guide_impulse = vec![Vec3::ZERO; self.guides.len() + track_count];
        for _ in 0..self.substeps {
            for b in &mut self.bodies {
                b.integrate_forces(self.gravity_mps2, dt);
                b.validate()?;
            }
            let mut limit_impulses = vec![0.; self.angular_joints.len()];
            for (i, j) in self.angular_joints.iter().enumerate() {
                angular_impulse[i] = angular_impulse[i] + j.motor(&mut self.bodies, dt)?;
            }
            // Project from the previous station to avoid jumping between nearby
            // branches of a loop. Each body remains independently integrated.
            if let Some((track, guides)) = track_guidance.as_mut() {
                for g in guides.iter_mut() {
                    let point = self.bodies[g.body].world_point(g.local_point_m)?;
                    g.station_m = project_station(track, point, g.station_m)?;
                }
            }
            for _ in 0..self.iterations {
                for (i, j) in self.angular_joints.iter().enumerate() {
                    angular_impulse[i] = angular_impulse[i]
                        + j.solve(&mut self.bodies, dt, &mut limit_impulses[i])?;
                }
                for (index, j) in self.joints.iter().enumerate() {
                    let (a, b) = pair_mut(&mut self.bodies, j.body_a, j.body_b);
                    let ra = a.frame().local_to_world(j.anchor_a_m);
                    let rb = b.frame().local_to_world(j.anchor_b_m);
                    let error = (b.pose.position_m + rb) - (a.pose.position_m + ra);
                    for n in [Vec3::X, Vec3::Y, Vec3::Z] {
                        let velocity = b.point_velocity(rb) - a.point_velocity(ra);
                        let denom = a.effective_mass(ra, n) + b.effective_mass(rb, n);
                        let impulse = scalar_impulse(velocity.dot(n), error.dot(n), denom, dt)?;
                        let p = n * impulse;
                        a.impulse(ra, -p);
                        b.impulse(rb, p);
                        joint_impulse[index] = joint_impulse[index] + p;
                    }
                    a.validate()?;
                    b.validate()?;
                }
                for (index, g) in self.guides.iter().enumerate() {
                    let b = &mut self.bodies[g.body];
                    let r = b.frame().local_to_world(g.local_point_m);
                    let delta = b.pose.position_m + r - g.line_origin_m;
                    let axis = if g.line_direction.y.abs() < 0.9 {
                        Vec3::Y
                    } else {
                        Vec3::X
                    };
                    let n1 = g
                        .line_direction
                        .cross(axis)
                        .normalized()
                        .map_err(|e| e.to_string())?;
                    let n2 = g.line_direction.cross(n1);
                    for n in [n1, n2] {
                        let impulse = scalar_impulse(
                            b.point_velocity(r).dot(n),
                            delta.dot(n),
                            b.effective_mass(r, n),
                            dt,
                        )?;
                        let p = n * impulse;
                        b.impulse(r, p);
                        guide_impulse[index] = guide_impulse[index] + p;
                    }
                    b.validate()?;
                }
                if let Some((track, guides)) = track_guidance.as_mut() {
                    for (index, g) in guides.iter().enumerate() {
                        let sample = track
                            .sample_at_distance(g.station_m)
                            .map_err(|e| e.to_string())?;
                        let b = &mut self.bodies[g.body];
                        let r = b.frame().local_to_world(g.local_point_m);
                        let delta = b.pose.position_m + r - sample.position_m;
                        for n in [sample.frame.normal(), sample.frame.lateral()] {
                            let impulse = scalar_impulse(
                                b.point_velocity(r).dot(n),
                                delta.dot(n),
                                b.effective_mass(r, n),
                                dt,
                            )?;
                            let p = n * impulse;
                            b.impulse(r, p);
                            guide_impulse[self.guides.len() + index] =
                                guide_impulse[self.guides.len() + index] + p;
                        }
                        b.validate()?;
                    }
                }
            }
            for b in &mut self.bodies {
                b.advance(dt)?;
                b.validate()?;
            }
        }
        let mut max_joint_error_m: f64 = 0.0;
        let mut max_angular_error_rad: f64 = 0.;
        for j in &self.angular_joints {
            max_angular_error_rad = max_angular_error_rad.max(j.error(&self.bodies)?);
        }
        for j in &self.joints {
            max_joint_error_m = max_joint_error_m.max(
                (self.bodies[j.body_b].world_point(j.anchor_b_m)?
                    - self.bodies[j.body_a].world_point(j.anchor_a_m)?)
                .length(),
            );
        }
        let mut max_guide_error_m: f64 = 0.0;
        for g in &self.guides {
            let d = self.bodies[g.body].world_point(g.local_point_m)? - g.line_origin_m;
            max_guide_error_m =
                max_guide_error_m.max((d - g.line_direction * d.dot(g.line_direction)).length());
        }
        if let Some((track, guides)) = track_guidance {
            for g in guides.iter_mut() {
                let point = self.bodies[g.body].world_point(g.local_point_m)?;
                g.station_m = project_station(track, point, g.station_m)?;
                let sample = track
                    .sample_at_distance(g.station_m)
                    .map_err(|e| e.to_string())?;
                let delta = point - sample.position_m;
                // No invented end-stop or extrapolated rail at open endpoints.
                let along = delta.dot(sample.frame.tangent());
                if (g.station_m <= 1e-9 && along < -1e-7)
                    || (g.station_m >= track.length_m() - 1e-9 && along > 1e-7)
                {
                    return Err("track guide reached an open endpoint".into());
                }
                max_guide_error_m = max_guide_error_m.max(delta.length());
            }
        }
        if !max_joint_error_m.is_finite()
            || !max_angular_error_rad.is_finite()
            || angular_impulse.iter().any(|v| !v.is_finite())
            || !max_guide_error_m.is_finite()
            || joint_impulse
                .iter()
                .chain(&guide_impulse)
                .any(|v| !v.is_finite())
        {
            return Err("constraint diagnostics overflow".into());
        }
        Ok(Diagnostics {
            max_angular_error_rad,
            angular_torque_on_b_nm: angular_impulse
                .into_iter()
                .map(|p| p / FIXED_STEP_SECONDS)
                .collect(),
            max_joint_error_m,
            max_guide_error_m,
            joint_force_on_b_n: joint_impulse
                .into_iter()
                .map(|p| p / FIXED_STEP_SECONDS)
                .collect(),
            guide_force_n: guide_impulse
                .into_iter()
                .map(|p| p / FIXED_STEP_SECONDS)
                .collect(),
        })
    }
}

// Baumgarte stabilization corrects 20% of the current error per substep;
// it introduces artificial energy and is not a rigid contact model.
pub(crate) fn scalar_impulse(velocity: f64, error: f64, denominator: f64, dt: f64) -> Result<f64> {
    let value = -(velocity + 0.2 * error / dt) / denominator;
    if !denominator.is_finite() || denominator <= 1e-15 || !value.is_finite() {
        return Err("constraint solve failed: singular or nonfinite impulse".into());
    }
    Ok(value)
}
fn pair_mut(bodies: &mut [Body], a: usize, b: usize) -> (&mut Body, &mut Body) {
    if a < b {
        let (left, right) = bodies.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = bodies.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

// Local tangent projection retains branch identity at self-intersections. The
// finite iteration and displacement bounds deliberately reject lost tracking.
pub(crate) fn project_station(track: &GeneratedTrack, point: Vec3, hint: f64) -> Result<f64> {
    let mut s = hint;
    for _ in 0..12 {
        let sample = track.sample_at_distance(s).map_err(|e| e.to_string())?;
        let correction = (point - sample.position_m).dot(sample.frame.tangent());
        if !correction.is_finite() || correction.abs() > 2.0 {
            return Err("track guide projection lost its local branch".into());
        }
        let next = (s + correction).clamp(0.0, track.length_m());
        if (next - s).abs() < 1e-10 {
            return Ok(next);
        }
        s = next;
    }
    Err("track guide projection did not converge".into())
}
