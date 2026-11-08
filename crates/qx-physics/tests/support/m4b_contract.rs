//! One position/velocity contract for assembly, reactions and time integration.
use super::{
    guidance::{AXES, point, point_row, roll_axis, row, twist},
    matrix::{Matrix, dot, max_abs, multiply},
    mechanisms::Mechanism,
    surface::{Failure, Rail, Result, Role, Surface},
};
use qx_geometry::math::{Frame, Quaternion, Vec3};
use qx_physics::Pose;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    B0,
    B1,
}
#[derive(Clone, Copy, Debug)]
pub struct Support {
    pub owner: usize,
    pub local: Vec3,
    pub surface: Surface,
    pub station: f64,
    pub half_window: f64,
}
pub struct Bearing {
    pub a: usize,
    pub b: usize,
    pub local_a: Vec3,
    pub local_b: Vec3,
    pub frame_a: Quaternion,
    pub frame_b: Quaternion,
    pub roll: bool,
    /// Synthetic travel envelope on relative rotation magnitude, not an extra
    /// equality or an invented hinge. An active bound rejects this experiment.
    pub max_swing: f64,
}
pub struct Model {
    pub poses: Vec<Pose>,
    pub axles: Vec<(usize, f64)>,
    pub bearings: Vec<Bearing>,
    pub supports: Vec<Support>,
}
pub struct Evaluation {
    pub c: Vec<f64>,
    pub j: Matrix,
    pub rail_rows: usize,
    pub branches: Vec<usize>,
}
pub fn perturb(p: Pose, d: &[f64]) -> Pose {
    let w = Vec3::new(d[3], d[4], d[5]);
    let a = w.length();
    let scale = if a < 1e-8 {
        0.5 - a * a / 48.
    } else {
        (a * 0.5).sin() / a
    };
    let q = Quaternion {
        w: (a * 0.5).cos(),
        x: w.x * scale,
        y: w.y * scale,
        z: w.z * scale,
    };
    Pose {
        position_m: p.position_m + Vec3::new(d[0], d[1], d[2]),
        orientation: q.compose(p.orientation).normalized().unwrap(),
    }
}
impl Model {
    pub fn new(m: Mechanism) -> Self {
        let mut supports = vec![];
        // Five authored bilateral support locations per rigid axle carrier.
        // .12 m longitudinal footprint; .6 m half-gauge. Not a wheel fit.
        for &(owner, station) in &m.axles {
            for (x, z, role) in [
                (-0.12, -0.6, Role::Running),
                (0.12, -0.6, Role::Running),
                (0., 0.6, Role::Running),
                (-0.12, 0.6, Role::Side),
                (0.12, 0.6, Role::Side),
            ] {
                let y = if role == Role::Side { -0.08 } else { 0. };
                supports.push(Support {
                    owner,
                    local: Vec3::new(x, y, z),
                    station: station + x,
                    half_window: 0.8,
                    surface: Surface {
                        rail: if z < 0. { "left" } else { "right" },
                        face: if role == Role::Running {
                            "running-plane"
                        } else {
                            "side-plane"
                        },
                        offset: Vec3::new(0., y, z),
                        role,
                    },
                });
            }
        }
        Self {
            poses: m.poses,
            axles: m.axles,
            supports,
            bearings: m
                .edges
                .into_iter()
                .map(|e| Bearing {
                    a: e.a,
                    b: e.b,
                    local_a: e.local_a,
                    local_b: e.local_b,
                    frame_a: Quaternion::IDENTITY,
                    frame_b: Quaternion::IDENTITY,
                    roll: e.roll,
                    max_swing: 0.7,
                })
                .collect(),
        }
    }
    pub fn shifted(&self, poses: &[Pose], d: &[f64]) -> Vec<Pose> {
        poses
            .iter()
            .enumerate()
            .map(|(i, &p)| perturb(p, &d[i * 6..i * 6 + 6]))
            .collect()
    }
    pub fn evaluate(&self, rail: &Rail, poses: &[Pose], kind: Kind) -> Result<Evaluation> {
        if poses.len() != self.poses.len() || poses.len() > 16 || self.supports.len() > 64 {
            return Err(Failure::Invalid);
        }
        let n = poses.len();
        let mut c = vec![];
        let mut j = vec![];
        let mut branches = vec![];
        if kind == Kind::B1 {
            for s in &self.supports {
                let p = *poses.get(s.owner).ok_or(Failure::Invalid)?;
                let x = point(p, s.local);
                let (station, t) = rail.project(s.surface, x, s.station, s.half_window)?;
                branches.push(
                    rail.track
                        .samples
                        .partition_point(|p| p.distance_m <= station.rem_euclid(rail.length())),
                );
                let delta = x - t.p;
                let d = t.d.dot(t.d) - delta.dot(t.dd);
                if d <= 1e-8 {
                    return Err(Failure::Singular);
                }
                let g = t.n + t.d * ((delta.dot(t.dn) - t.d.dot(t.n)) / d);
                c.push(delta.dot(t.n));
                j.push(point_row(n, s.owner, x - p.position_m, g));
            }
        } else {
            let surface = Surface {
                rail: "center-datum",
                face: "B0-datum",
                offset: Vec3::ZERO,
                role: Role::Running,
            };
            for &(owner, station) in &self.axles {
                let p = poses[owner];
                let (station, t) = rail.project(surface, p.position_m, station, 0.8)?;
                branches.push(
                    rail.track
                        .samples
                        .partition_point(|p| p.distance_m <= station.rem_euclid(rail.length())),
                );
                let delta = p.position_m - t.p;
                let ds = t.d / (t.d.dot(t.d) - delta.dot(t.dd));
                // Two normal coordinates of exact closest centerline displacement.
                for axis in [Vec3::Y, Vec3::Z] {
                    let normal = t.frame.local_to_world(axis);
                    let g = normal + ds * (delta.dot(t.k.cross(normal)) - t.d.dot(normal));
                    c.push(delta.dot(normal));
                    j.push(row(n, owner, g, Vec3::ZERO));
                }
                // Smooth local quaternion-vector chart (no rotation-vector FD).
                let q = p.orientation.compose(t.frame.orientation().conjugate());
                if q.w < 0.5 {
                    return Err(Failure::Singular);
                }
                let v = Vec3::new(q.x, q.y, q.z);
                for axis in AXES {
                    let linear = ds * axis.dot(t.k * (-q.w) + t.k.cross(v));
                    let angular = axis * q.w + v.cross(axis);
                    c.push(2. * v.dot(axis));
                    j.push(row(n, owner, linear, angular));
                }
            }
        }
        let rail_rows = c.len();
        for b in &self.bearings {
            let (a, p) = (poses[b.a], poses[b.b]);
            let (pa, pb) = (point(a, b.local_a), point(p, b.local_b));
            let gap = pb - pa;
            for axis in AXES {
                c.push(gap.dot(axis));
                let mut r = point_row(n, b.b, pb - p.position_m, axis);
                let other = point_row(n, b.a, pa - a.position_m, axis);
                for (r, o) in r.iter_mut().zip(other) {
                    *r -= o;
                }
                j.push(r);
            }
            let qa = a.orientation.compose(b.frame_a);
            let qb = p.orientation.compose(b.frame_b);
            if qa
                .conjugate()
                .compose(qb)
                .rotation_vector()
                .map_err(|_| Failure::Invalid)?
                .length()
                >= b.max_swing
            {
                return Err(Failure::BearingLimit);
            }
            if b.roll {
                let axis = roll_axis(Frame::new(qa).unwrap(), Frame::new(qb).unwrap());
                let mut r = row(n, b.b, Vec3::ZERO, axis);
                let other = row(n, b.a, Vec3::ZERO, axis);
                for (r, o) in r.iter_mut().zip(other) {
                    *r -= o;
                }
                c.push(twist(qa, qb));
                j.push(r);
            }
        }
        if c.iter().chain(j.iter().flatten()).any(|x| !x.is_finite()) {
            return Err(Failure::Invalid);
        }
        Ok(Evaluation {
            c,
            j,
            rail_rows,
            branches,
        })
    }
    pub fn inverse_mass(&self) -> Vec<f64> {
        (0..6 * self.poses.len())
            .map(|i| if i % 6 < 3 { 0.01 } else { 0.05 })
            .collect()
    }
    /// Same equations, unit numerical weights, no damping that creates rank.
    /// A minimum-norm step leaves the one travel gauge unpinned.
    pub fn initialize(&self, rail: &Rail, kind: Kind) -> Result<(Vec<Pose>, usize)> {
        let mut p = self.poses.clone();
        for iteration in 0..20 {
            let e = self.evaluate(rail, &p, kind)?;
            if max_abs(&e.c) < 2e-11 {
                return Ok((p, iteration));
            }
            let step = correction(
                &e.j,
                &vec![1.; 6 * p.len()],
                &e.c.iter().map(|c| -c).collect::<Vec<_>>(),
            )?
            .0;
            if max_abs(&step) > 0.2 {
                return Err(Failure::NoConvergence);
            }
            let mut accepted = false;
            for scale in [1., 0.5, 0.25, 0.125] {
                let next = self.shifted(&p, &step.iter().map(|x| x * scale).collect::<Vec<_>>());
                if let Ok(test) = self.evaluate(rail, &next, kind)
                    && max_abs(&test.c) < max_abs(&e.c)
                {
                    p = next;
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return Err(Failure::NoConvergence);
            }
        }
        Err(Failure::NoConvergence)
    }
    /// Jdot*v in the same spatial-velocity convention. Directional difference
    /// is intentionally research-only; h refinement is tested independently.
    pub fn curvature(
        &self,
        rail: &Rail,
        p: &[Pose],
        v: &[f64],
        kind: Kind,
        h: f64,
    ) -> Result<Vec<f64>> {
        let base = self.evaluate(rail, p, kind)?;
        for i in 0..12 {
            let h = h * 0.5_f64.powi(i);
            let plus = self.shifted(p, &v.iter().map(|x| x * h).collect::<Vec<_>>());
            let minus = self.shifted(p, &v.iter().map(|x| -x * h).collect::<Vec<_>>());
            let a = self.evaluate(rail, &plus, kind)?;
            let b = self.evaluate(rail, &minus, kind)?;
            if a.branches == base.branches && b.branches == base.branches {
                let a = multiply(&a.j, v);
                let b = multiply(&b.j, v);
                return Ok(a.iter().zip(b).map(|(a, b)| (a - b) / (2. * h)).collect());
            }
        }
        Err(Failure::Corner)
    }
}

/// Bounded dense Gram solve, deliberately rejects redundant/nonunique loads.
pub fn correction(j: &Matrix, inv: &[f64], rhs: &[f64]) -> Result<(Vec<f64>, Vec<f64>)> {
    let mut gram: Matrix = j
        .iter()
        .map(|a| {
            j.iter()
                .map(|b| a.iter().zip(b).zip(inv).map(|((a, b), m)| a * b * m).sum())
                .collect()
        })
        .collect();
    let mut b = rhs.to_vec();
    let n = j.len();
    if n == 0 || n > 128 || inv.len() > 96 || rhs.len() != n {
        return Err(Failure::Invalid);
    }
    for col in 0..n {
        let best = (col..n)
            .max_by(|&a, &b| gram[a][col].abs().total_cmp(&gram[b][col].abs()))
            .unwrap();
        gram.swap(col, best);
        b.swap(col, best);
        let pivot = gram[col][col];
        if !pivot.is_finite() || pivot.abs() < 1e-12 {
            return Err(Failure::Singular);
        }
        for x in &mut gram[col][col..] {
            *x /= pivot;
        }
        b[col] /= pivot;
        let r = gram[col].clone();
        let bc = b[col];
        for i in 0..n {
            if i != col {
                let k = gram[i][col];
                for (x, y) in gram[i][col..].iter_mut().zip(&r[col..]) {
                    *x -= k * y;
                }
                b[i] -= k * bc;
            }
        }
    }
    let dv = (0..inv.len())
        .map(|k| inv[k] * j.iter().zip(&b).map(|(r, b)| r[k] * b).sum::<f64>())
        .collect::<Vec<_>>();
    if dv.iter().chain(&b).any(|x| !x.is_finite()) {
        return Err(Failure::Singular);
    }
    if max_abs(
        &multiply(j, &dv)
            .iter()
            .zip(rhs)
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    ) > 1e-7 * (1. + max_abs(rhs))
    {
        return Err(Failure::Singular);
    }
    Ok((dv, b))
}
pub fn kinetic(v: &[f64], inv: &[f64]) -> f64 {
    0.5 * v.iter().zip(inv).map(|(v, m)| v * v / m).sum::<f64>()
}
pub fn work(j: &Matrix, v: &[f64], loads: &[f64]) -> f64 {
    dot(&multiply(j, v), loads)
}
