//! EXPERIMENTAL ideal rail carriages and explicit bearings, not wheel contacts.
//! All geometry is declared before solving. No production assembly is invoked.
use super::guidance::{perturb, point, twist};
use qx_geometry::math::{Frame, Quaternion, Vec3, rotation_between};
use qx_physics::Pose;

#[derive(Clone, Copy, Debug)]
pub enum Path {
    Straight,
    Turn,
    Bank,
    Helix,
    Loop,
    Inverted,
}
impl Path {
    pub fn at(self, s: f64) -> Pose {
        let (p, q) = match self {
            Self::Straight => (Vec3::X * s, Quaternion::IDENTITY),
            Self::Inverted => (
                Vec3::X * s,
                Quaternion::axis_angle(Vec3::X, std::f64::consts::PI).unwrap(),
            ),
            Self::Loop => {
                let t = s / 8.;
                (
                    Vec3::new(8. * t.sin(), 8. * (1. - t.cos()), 0.),
                    Quaternion::axis_angle(Vec3::Z, t).unwrap(),
                )
            }
            _ => {
                let rise = if matches!(self, Self::Helix) {
                    0.3_f64
                } else {
                    0.
                };
                let t = s * (1. - rise * rise).sqrt() / 8.;
                let bank = match self {
                    Self::Bank => 0.02 * s + 0.001 * s * s,
                    Self::Helix => 0.4 + 0.04 * s,
                    _ => 0.,
                };
                (
                    Vec3::new(8. * t.sin(), rise * s, 8. * (1. - t.cos())),
                    Quaternion::axis_angle(Vec3::Y, -t)
                        .unwrap()
                        .compose(Quaternion::axis_angle(Vec3::Z, rise.asin()).unwrap())
                        .compose(Quaternion::axis_angle(Vec3::X, bank).unwrap()),
                )
            }
        };
        Pose {
            position_m: p,
            orientation: q,
        }
    }
    /// Independent explicit basis, no quaternion/Frame transformation.
    pub fn locus(self, s: f64, y: f64, z: f64) -> Vec3 {
        match self {
            Self::Straight => Vec3::new(s, y, z),
            Self::Inverted => Vec3::new(s, -y, -z),
            Self::Loop => {
                let t = s / 8.;
                Vec3::new(
                    8. * t.sin() - y * t.sin(),
                    8. * (1. - t.cos()) + y * t.cos(),
                    z,
                )
            }
            _ => {
                let rise = if matches!(self, Self::Helix) {
                    0.3_f64
                } else {
                    0.
                };
                let t = s * (1. - rise * rise).sqrt() / 8.;
                let bank = match self {
                    Self::Bank => 0.02 * s + 0.001 * s * s,
                    Self::Helix => 0.4 + 0.04 * s,
                    _ => 0.,
                };
                let n = Vec3::new(-rise * t.cos(), (1. - rise * rise).sqrt(), -rise * t.sin());
                let l = Vec3::new(-t.sin(), 0., t.cos());
                Vec3::new(8. * t.sin(), rise * s, 8. * (1. - t.cos()))
                    + n * (y * bank.cos() - z * bank.sin())
                    + l * (y * bank.sin() + z * bank.cos())
            }
        }
    }
}

fn root(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let negative = f(lo) < 0.;
    assert_ne!(negative, f(hi) < 0., "independent witness not bracketed");
    for _ in 0..64 {
        let mid = (lo + hi) * 0.5;
        let value = f(mid);
        if value.abs() < 2e-14 {
            return mid;
        }
        if (value < 0.) == negative {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

/// Fix a rigid chord and zero twist relative to its FRONT bearing. A rear
/// ball, rather than a second roll lock, accommodates varying rail bank.
fn bridge(a: Vec3, b: Vec3, reference: Quaternion, local_a: Vec3, local_b: Vec3) -> Pose {
    assert!((local_a - local_b).cross(Vec3::X).length() < 1e-12);
    let mut q = rotation_between(Frame::new(reference).unwrap().tangent(), a - b)
        .unwrap()
        .compose(reference);
    let theta = twist(reference, q);
    q = q
        .compose(Quaternion::axis_angle(Vec3::X, -theta).unwrap())
        .normalized()
        .unwrap();
    let p = Pose {
        position_m: a - Frame::new(q).unwrap().local_to_world(local_a),
        orientation: q,
    };
    assert!((point(p, local_b) - b).length() < 1e-12);
    assert!(twist(reference, q).abs() < 1e-12);
    p
}
pub struct Edge {
    pub a: usize,
    pub b: usize,
    pub local_a: Vec3,
    pub local_b: Vec3,
    pub roll: bool,
}
pub struct Mechanism {
    pub path: Path,
    pub poses: Vec<Pose>,
    pub axles: Vec<(usize, f64)>,
    pub edges: Vec<Edge>,
    pub bogies: Vec<usize>,
    pub cars: Vec<usize>,
}
impl Mechanism {
    fn bogie_pose(path: Path, center: f64) -> (Pose, [f64; 2]) {
        let h = root(0.59, 0.62, |h| {
            (path.at(center + h * 0.5).position_m - path.at(center - h * 0.5).position_m).length()
                - 0.6
        });
        let stations = [center + h * 0.5, center - h * 0.5];
        let a = path.at(stations[0]);
        let b = path.at(stations[1]);
        (
            bridge(
                a.position_m,
                b.position_m,
                a.orientation,
                Vec3::X * 0.3,
                Vec3::X * (-0.3),
            ),
            stations,
        )
    }
    /// One car/two bogies/four axle carriages, or two cars sharing ONE middle
    /// bogie (three bogies/six axle carriages). No duplicated shared identity.
    pub fn new(path: Path, lead: f64, suspended: bool, offset: Vec3, shared: bool) -> Self {
        let mut m = Self {
            path,
            poses: vec![],
            axles: vec![],
            edges: vec![],
            bogies: vec![],
            cars: vec![],
        };
        let mut center = lead;
        for i in 0..if shared { 3 } else { 2 } {
            if i > 0 {
                let prev = point(m.poses[*m.bogies.last().unwrap()], offset);
                center = root(center - 2.55, center - 2.25, |s| {
                    (point(Self::bogie_pose(path, s).0, offset) - prev).length() - 2.4
                });
            }
            let (p, stations) = Self::bogie_pose(path, center);
            let bogie = m.poses.len();
            m.bogies.push(bogie);
            m.poses.push(p);
            for (i, s) in stations.into_iter().enumerate() {
                let axle = m.poses.len();
                m.poses.push(path.at(s));
                m.axles.push((axle, s));
                m.edges.push(Edge {
                    a: axle,
                    b: bogie,
                    local_a: Vec3::ZERO,
                    local_b: Vec3::X * if i == 0 { 0.3 } else { -0.3 },
                    roll: i == 0,
                });
            }
        }
        let height = if suspended { -1.4 } else { 0.8 };
        for i in 0..if shared { 2 } else { 1 } {
            let a = m.bogies[i];
            let b = m.bogies[i + 1];
            let local_a = Vec3::new(1.2, -height, 0.);
            let local_b = Vec3::new(-1.2, -height, 0.);
            let car = m.poses.len();
            m.poses.push(bridge(
                point(m.poses[a], offset),
                point(m.poses[b], offset),
                m.poses[a].orientation,
                local_a,
                local_b,
            ));
            m.cars.push(car);
            m.edges.push(Edge {
                a,
                b: car,
                local_a: offset,
                local_b: local_a,
                roll: !suspended,
            });
            m.edges.push(Edge {
                a: b,
                b: car,
                local_a: offset,
                local_b,
                roll: false,
            });
        }
        m
    }
    pub fn coordinates(&self) -> usize {
        6 * self.poses.len() + self.axles.len()
    }
    pub fn residual(&self, d: &[f64]) -> Vec<f64> {
        let p: Vec<_> = self
            .poses
            .iter()
            .enumerate()
            .map(|(i, &p)| perturb(p, &d[i * 6..i * 6 + 6]))
            .collect();
        let mut r = vec![];
        for (i, &(body, s)) in self.axles.iter().enumerate() {
            let target = self.path.at(s + d[6 * self.poses.len() + i]);
            let e = p[body].position_m - target.position_m;
            let w = target
                .orientation
                .conjugate()
                .compose(p[body].orientation)
                .rotation_vector()
                .unwrap();
            r.extend([e.x, e.y, e.z, w.x, w.y, w.z]);
        }
        for e in &self.edges {
            let gap = point(p[e.b], e.local_b) - point(p[e.a], e.local_a);
            r.extend([gap.x, gap.y, gap.z]);
            if e.roll {
                r.push(twist(p[e.a].orientation, p[e.b].orientation));
            }
        }
        r
    }
}
