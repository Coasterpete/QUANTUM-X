//! Right-handed coordinates: initial forward +X, up +Y, lateral +Z.
use crate::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    pub fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y).hypot(self.z)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    pub fn normalized(self) -> Result<Self> {
        let length = self.length();
        ensure(
            length.is_finite() && length > 1e-14,
            "cannot normalize a zero, tiny, or nonfinite vector",
        )?;
        Ok(self / length)
    }
}
impl Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}
impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

/// Hamilton quaternion, active local-to-world rotation. Frame validates it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quaternion {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Default for Quaternion {
    fn default() -> Self {
        Self::IDENTITY
    }
}
impl Quaternion {
    pub const IDENTITY: Self = Self {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub fn axis_angle(axis: Vec3, radians: f64) -> Result<Self> {
        ensure(radians.is_finite(), "rotation angle must be finite")?;
        let a = axis.normalized()?;
        let (s, c) = (radians * 0.5).sin_cos();
        Ok(Self {
            w: c,
            x: a.x * s,
            y: a.y * s,
            z: a.z * s,
        })
    }
    pub fn normalized(self) -> Result<Self> {
        let n = self.w.hypot(self.x).hypot(self.y).hypot(self.z);
        ensure(n.is_finite() && n > 1e-14, "invalid quaternion norm")?;
        Ok(self.scaled(1.0 / n))
    }
    pub fn conjugate(self) -> Self {
        Self {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
    pub fn compose(self, b: Self) -> Self {
        Self {
            w: self.w * b.w - self.x * b.x - self.y * b.y - self.z * b.z,
            x: self.w * b.x + self.x * b.w + self.y * b.z - self.z * b.y,
            y: self.w * b.y - self.x * b.z + self.y * b.w + self.z * b.x,
            z: self.w * b.z + self.x * b.y - self.y * b.x + self.z * b.w,
        }
    }
    pub(crate) fn scaled(self, s: f64) -> Self {
        Self {
            w: self.w * s,
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
    pub(crate) fn plus(self, b: Self) -> Self {
        Self {
            w: self.w + b.w,
            x: self.x + b.x,
            y: self.y + b.y,
            z: self.z + b.z,
        }
    }
    pub(crate) fn rotate_unit(self, v: Vec3) -> Vec3 {
        let qv = Vec3::new(self.x, self.y, self.z);
        let t = qv.cross(v) * 2.0;
        v + t * self.w + qv.cross(t)
    }
    pub fn slerp(self, other: Self, t: f64) -> Result<Self> {
        ensure(
            t.is_finite() && (0.0..=1.0).contains(&t),
            "slerp fraction outside [0,1]",
        )?;
        let a = self.normalized()?;
        let mut b = other.normalized()?;
        let mut d = a.w * b.w + a.x * b.x + a.y * b.y + a.z * b.z;
        if d < 0.0 {
            b = b.scaled(-1.0);
            d = -d;
        }
        if d > 0.9995 {
            return a.scaled(1.0 - t).plus(b.scaled(t)).normalized();
        }
        let theta = d.clamp(-1.0, 1.0).acos();
        a.scaled(((1.0 - t) * theta).sin() / theta.sin())
            .plus(b.scaled((t * theta).sin() / theta.sin()))
            .normalized()
    }
    pub fn rotation_vector(self) -> Result<Vec3> {
        let mut q = self.normalized()?;
        if q.w < 0.0 {
            q = q.scaled(-1.0);
        }
        let v = Vec3::new(q.x, q.y, q.z);
        let n = v.length();
        if n < 1e-14 {
            Ok(v * 2.0)
        } else {
            Ok(v * (2.0 * n.atan2(q.w) / n))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    orientation: Quaternion,
}
impl Frame {
    pub fn new(orientation: Quaternion) -> Result<Self> {
        Ok(Self {
            orientation: orientation.normalized()?,
        })
    }
    pub fn orientation(self) -> Quaternion {
        self.orientation
    }
    pub fn tangent(self) -> Vec3 {
        self.local_to_world(Vec3::X)
    }
    pub fn normal(self) -> Vec3 {
        self.local_to_world(Vec3::Y)
    }
    pub fn lateral(self) -> Vec3 {
        self.local_to_world(Vec3::Z)
    }
    pub fn local_to_world(self, v: Vec3) -> Vec3 {
        self.orientation.rotate_unit(v)
    }
    pub fn world_to_local(self, v: Vec3) -> Vec3 {
        self.orientation.conjugate().rotate_unit(v)
    }
    /// Gravity-referenced up and lateral; undefined for a vertical tangent.
    pub fn horizontal_basis(self) -> Result<(Vec3, Vec3)> {
        let t = self.tangent();
        ensure(
            (1.0 - t.y * t.y).max(0.0).sqrt() > 1e-6,
            "gravity-referenced frame is undefined near vertical",
        )?;
        let up = (Vec3::Y - t * t.y).normalized()?;
        Ok((up, t.cross(up)))
    }
    /// Signed bank about the tangent, modulo 2π. Undefined at vertical tangents.
    pub fn bank_rad(self) -> Option<f64> {
        self.horizontal_basis()
            .ok()
            .map(|(u, l)| self.normal().dot(l).atan2(self.normal().dot(u)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_and_transform_invariants() {
        let q = Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap();
        let frame = Frame::new(q).unwrap();
        assert!((frame.tangent() - Vec3::Y).length() < 1e-14);
        let v = Vec3::new(3.0, -4.0, 8.0);
        assert!((frame.world_to_local(frame.local_to_world(v)) - v).length() < 1e-13);
        assert!((frame.tangent().cross(frame.normal()) - frame.lateral()).length() < 1e-14);
        assert!((q.compose(q.conjugate()).w - 1.0).abs() < 1e-14);
        assert!(Vec3::ZERO.normalized().is_err());
        assert!(Vec3::new(f64::NAN, 0.0, 0.0).normalized().is_err());
        assert!(
            Frame::new(Quaternion {
                w: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0
            })
            .is_err()
        );
    }
    #[test]
    fn shortest_quaternion_interpolation_has_no_sign_flip() {
        let q = Quaternion::axis_angle(Vec3::X, 1.0).unwrap();
        let mid = q.slerp(q.scaled(-1.0), 0.5).unwrap();
        assert!((mid.rotate_unit(Vec3::Y) - q.rotate_unit(Vec3::Y)).length() < 1e-14);
    }
}
