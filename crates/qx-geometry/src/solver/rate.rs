//! The one source angular-rate expression, evaluated on values or station jets.
use super::*;
use std::ops::{Add, Div, Mul};

pub(super) trait RateVector:
    Copy + Add<Output = Self> + Mul<Self::Scalar, Output = Self> + Div<f64, Output = Self>
{
    type Scalar: From<f64>
        + Copy
        + Add<Output = Self::Scalar>
        + Mul<f64, Output = Self::Scalar>
        + Div<f64, Output = Self::Scalar>;
    fn constant(v: Vec3) -> Self;
    fn dot(self, b: Self) -> Self::Scalar;
    fn cross(self, b: Self) -> Self;
}
impl RateVector for Vec3 {
    type Scalar = f64;
    fn constant(v: Vec3) -> Self {
        v
    }
    fn dot(self, b: Self) -> f64 {
        self.dot(b)
    }
    fn cross(self, b: Self) -> Self {
        self.cross(b)
    }
}

impl Context<'_> {
    pub(super) fn omega_axes<V: RateVector>(
        self,
        t: V,
        n: V,
        l: V,
        up: Option<V>,
        value: impl Fn(&crate::transition::Channel) -> Result<V::Scalar>,
    ) -> Result<V> {
        let scale = self.section.domain.metres_per_input(self.speed);
        let twist = value(&self.section.roll_rate)? / scale;
        let dt = match &self.section.kind {
            SectionKind::Spline { .. } => {
                return Err(GeometryError("spline uses its analytic evaluator".into()));
            }
            SectionKind::Straight {} => V::constant(Vec3::ZERO),
            SectionKind::Arc {
                radius_m,
                plane_angle_rad,
            } => {
                (
                    // Constant scalar multiplication expressed without changing the
                    // value path's arithmetic order.
                    n * V::Scalar::from(plane_angle_rad.cos())
                        + l * V::Scalar::from(plane_angle_rad.sin())
                ) / *radius_m
            }
            SectionKind::Force {
                normal_g,
                lateral_g,
            } => {
                let grav = V::constant(Vec3::new(0.0, -self.gravity, 0.0));
                n * ((value(normal_g)? * self.gravity + grav.dot(n)) / (self.speed * self.speed))
                    + l * ((value(lateral_g)? * self.gravity + grav.dot(l))
                        / (self.speed * self.speed))
            }
            SectionKind::RiderLocal {
                pitch_rate,
                yaw_rate,
            } => n * (value(pitch_rate)? / scale) + l * (value(yaw_rate)? / scale),
            SectionKind::Geometric {
                pitch_rate,
                yaw_rate,
            } => {
                up.ok_or_else(|| GeometryError("world-geometric jets unsupported".into()))?
                    * (value(pitch_rate)? / scale)
                    + t.cross(V::constant(Vec3::Y)) * (value(yaw_rate)? / scale)
            }
        };
        Ok(t.cross(dt) + t * twist)
    }
}
