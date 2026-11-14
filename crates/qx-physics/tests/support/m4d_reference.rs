//! Source-derived analytic development datums, NEVER a generated-track repair.
//! Scalar jets carry actual derivatives (not Taylor coefficients), in SI/f64.
use super::{
    mechanisms::Path,
    surface::{Failure, Jet, Result, Role, Surface},
};
use qx_geometry::math::{Frame, Vec3};
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug)]
struct D([f64; 4]);
impl D {
    fn constant(v: f64) -> Self {
        Self([v, 0., 0., 0.])
    }
    fn variable(v: f64) -> Self {
        Self([v, 1., 0., 0.])
    }
    fn sin(self) -> Self {
        let [v, d, dd, ddd] = self.0;
        Self([
            v.sin(),
            v.cos() * d,
            v.cos() * dd - v.sin() * d * d,
            v.cos() * ddd - 3. * v.sin() * d * dd - v.cos() * d * d * d,
        ])
    }
    fn cos(self) -> Self {
        (self + Self::constant(std::f64::consts::FRAC_PI_2)).sin()
    }
}
impl Add for D {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + b.0[i]))
    }
}
impl Sub for D {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        self + b * -1.
    }
}
impl Mul<f64> for D {
    type Output = Self;
    fn mul(self, b: f64) -> Self {
        Self(self.0.map(|x| x * b))
    }
}
impl Mul for D {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        let (a, b) = (self.0, b.0);
        Self([
            a[0] * b[0],
            a[1] * b[0] + a[0] * b[1],
            a[2] * b[0] + 2. * a[1] * b[1] + a[0] * b[2],
            a[3] * b[0] + 3. * a[2] * b[1] + 3. * a[1] * b[2] + a[0] * b[3],
        ])
    }
}
type V = [D; 3];
fn constant(v: Vec3) -> V {
    [D::constant(v.x), D::constant(v.y), D::constant(v.z)]
}
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn scale(a: V, b: D) -> V {
    a.map(|x| x * b)
}
fn derivative(a: V, i: usize) -> Vec3 {
    Vec3::new(a[0].0[i], a[1].0[i], a[2].0[i])
}

#[derive(Clone, Copy, Debug)]
pub struct Differential {
    pub jet: Jet,
    pub ddd: Vec3,
    pub ddn: Vec3,
    pub dk: Vec3,
}

/// These are the SAME explicit synthetic source paths used to seed M4A-C.
/// No interpolation of generated samples, normal averaging or fitting occurs.
pub fn differential(path: Path, surface: Surface, station: f64) -> Result<Differential> {
    if !station.is_finite()
        || station.abs() > 1000.
        || !surface.offset.is_finite()
        || surface.offset.length() > 2.
        || surface.rail.is_empty()
        || surface.face.is_empty()
    {
        return Err(Failure::Invalid);
    }
    let s = D::variable(station);
    let zero = D::constant(0.);
    let (c, t, n, l) = match path {
        Path::Straight | Path::Inverted => {
            let sign = if matches!(path, Path::Inverted) {
                -1.
            } else {
                1.
            };
            (
                [s, zero, zero],
                constant(Vec3::X),
                constant(Vec3::Y * sign),
                constant(Vec3::Z * sign),
            )
        }
        Path::Loop => {
            let a = s * 0.125;
            (
                [a.sin() * 8., (D::constant(1.) - a.cos()) * 8., zero],
                [a.cos(), a.sin(), zero],
                [a.sin() * -1., a.cos(), zero],
                constant(Vec3::Z),
            )
        }
        _ => {
            let rise = if matches!(path, Path::Helix) {
                0.3_f64
            } else {
                0.
            };
            let h = (1. - rise * rise).sqrt();
            let a = s * (h / 8.);
            let bank = match path {
                Path::Bank => s * 0.02 + s * s * 0.001,
                Path::Helix => D::constant(0.4) + s * 0.04,
                _ => zero,
            };
            let n0 = [a.cos() * -rise, D::constant(h), a.sin() * -rise];
            let l0 = [a.sin() * -1., zero, a.cos()];
            (
                [a.sin() * 8., s * rise, (D::constant(1.) - a.cos()) * 8.],
                [a.cos() * h, D::constant(rise), a.sin() * h],
                add(scale(n0, bank.cos()), scale(l0, bank.sin())),
                add(scale(n0, bank.sin() * -1.), scale(l0, bank.cos())),
            )
        }
    };
    let a = surface.offset;
    let p = add(
        c,
        add(
            scale(t, D::constant(a.x)),
            add(scale(n, D::constant(a.y)), scale(l, D::constant(a.z))),
        ),
    );
    let (position, d, dd, ddd) = (
        derivative(p, 0),
        derivative(p, 1),
        derivative(p, 2),
        derivative(p, 3),
    );
    let width = if surface.role == Role::Running { l } else { n };
    let (w, dw, ddw) = (
        derivative(width, 0),
        derivative(width, 1),
        derivative(width, 2),
    );
    let sign = if surface.role == Role::Running {
        1.
    } else {
        -1.
    };
    let b = w.cross(d) * sign;
    let db = (dw.cross(d) + w.cross(dd)) * sign;
    let ddb = (ddw.cross(d) + dw.cross(dd) * 2. + w.cross(ddd)) * sign;
    let length = b.length();
    if length < 1e-8 || d.length() < 1e-8 {
        return Err(Failure::Singular);
    }
    let normal = b / length;
    let dl = normal.dot(db);
    let dn = (db - normal * dl) / length;
    let ddl = dn.dot(db) + normal.dot(ddb);
    let ddn = (ddb - normal * ddl - dn * (2. * dl)) / length;
    let k = [t, n, l].into_iter().fold(Vec3::ZERO, |v, a| {
        v + derivative(a, 0).cross(derivative(a, 1)) * 0.5
    });
    let dk = [t, n, l].into_iter().fold(Vec3::ZERO, |v, a| {
        v + derivative(a, 0).cross(derivative(a, 2)) * 0.5
    });
    let frame = Frame::new(path.at(station).orientation).map_err(|_| Failure::Invalid)?;
    Ok(Differential {
        jet: Jet {
            p: position,
            d,
            dd,
            n: normal,
            dn,
            frame,
            k,
        },
        ddd,
        ddn,
        dk,
    })
}
