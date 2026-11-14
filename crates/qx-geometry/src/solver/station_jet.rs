//! Ordinary derivatives (not Taylor coefficients) through second station order.
use super::{Context, Result, rate::RateVector};
use crate::math::Quaternion;
use crate::{math::Vec3, transition::Side};
use std::ops::{Add, Div, Mul};

#[derive(Clone, Copy)]
pub(super) struct Scalar(pub [f64; 3]);
impl From<f64> for Scalar {
    fn from(v: f64) -> Self {
        Self([v, 0., 0.])
    }
}
impl Add for Scalar {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + b.0[i]))
    }
}
impl Mul<f64> for Scalar {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self(self.0.map(|v| v * s))
    }
}
impl Div<f64> for Scalar {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self(self.0.map(|v| v / s))
    }
}
#[derive(Clone, Copy)]
pub(super) struct Vector(pub [Vec3; 3]);
impl Add for Vector {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(std::array::from_fn(|i| self.0[i] + b.0[i]))
    }
}
impl Mul<Scalar> for Vector {
    type Output = Self;
    fn mul(self, b: Scalar) -> Self {
        let [v, d, dd] = self.0;
        let [a, da, dda] = b.0;
        Self([v * a, d * a + v * da, dd * a + d * (2. * da) + v * dda])
    }
}
impl Div<f64> for Vector {
    type Output = Self;
    fn div(self, s: f64) -> Self {
        Self(self.0.map(|v| v / s))
    }
}
impl RateVector for Vector {
    type Scalar = Scalar;
    fn constant(v: Vec3) -> Self {
        Self([v, Vec3::ZERO, Vec3::ZERO])
    }
    fn dot(self, b: Self) -> Scalar {
        let [v, d, dd] = self.0;
        let [w, e, ee] = b.0;
        Scalar([
            v.dot(w),
            d.dot(w) + v.dot(e),
            dd.dot(w) + 2. * d.dot(e) + v.dot(ee),
        ])
    }
    fn cross(self, b: Self) -> Self {
        let [v, d, dd] = self.0;
        let [w, e, ee] = b.0;
        Self([
            v.cross(w),
            d.cross(w) + v.cross(e),
            dd.cross(w) + d.cross(e) * 2. + v.cross(ee),
        ])
    }
}
impl Context<'_> {
    /// Differentiate the *same* rate expression along the source solution.
    /// First compute k' with R'=k×R, then k'' with R''=k'×R+k×(k×R).
    pub(super) fn omega_jet(self, x: f64, q: Quaternion, side: Side) -> Result<[Vec3; 3]> {
        let frame = crate::math::Frame::new(q)?;
        let k = self.omega(x, q, side)?;
        let scale = self.section.domain.metres_per_input(self.speed);
        let controls = |c: &crate::transition::Channel| {
            let e = c.evaluate_side(x, side)?;
            Ok(Scalar([
                e.value,
                e.derivative / scale,
                e.second_derivative / (scale * scale),
            ]))
        };
        let axes = [frame.tangent(), frame.normal(), frame.lateral()];
        let first = axes.map(|v| Vector([v, k.cross(v), Vec3::ZERO]));
        let dk = self
            .omega_axes(first[0], first[1], first[2], None, controls)?
            .0[1];
        let full = axes.map(|v| Vector([v, k.cross(v), dk.cross(v) + k.cross(k.cross(v))]));
        let result = self
            .omega_axes(full[0], full[1], full[2], None, controls)?
            .0;
        crate::ensure(
            result.iter().all(|v| v.is_finite()),
            "nonfinite source angular jet",
        )?;
        Ok(result)
    }
}
