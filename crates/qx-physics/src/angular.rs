//! Sequential angular impulses for fixed and revolute joints. Linear anchors are
//! enforced by the companion ball joint. No body orientation is prescribed.
use crate::{Body, Result};
use qx_document::train::mechanism::{JointModel, RotationDrive};
use qx_geometry::math::{Frame, Quaternion, Vec3, rotation_between};

#[derive(Clone, Debug, PartialEq)]
pub struct AngularJoint {
    pub body_a: usize,
    pub body_b: usize,
    pub model: JointModel,
}
impl AngularJoint {
    pub(crate) fn validate(&self, count: usize) -> Result<()> {
        self.model.validate().map_err(|e| e.to_string())?;
        if self.body_a >= count
            || self.body_b >= count
            || self.body_a == self.body_b
            || matches!(self.model, JointModel::Ball)
        {
            return Err("invalid angular joint body or model".into());
        }
        if matches!(
            &self.model,
            JointModel::Hinge {
                drive: RotationDrive::MechanicalProfile { .. },
                ..
            }
        ) {
            return Err("mechanical rotation profiles are representable but their transmission dynamics are not implemented".into());
        }
        Ok(())
    }
    fn frames(&self, a: &Body, b: &Body) -> Result<(Frame, Frame)> {
        let (fa, fb) = match &self.model {
            JointModel::Fixed { frame_a, frame_b }
            | JointModel::Hinge {
                frame_a, frame_b, ..
            } => (*frame_a, *frame_b),
            JointModel::Ball => return Err("ball joint has no angular constraint".into()),
        };
        Ok((
            Frame::new(a.pose.orientation.compose(fa)).map_err(|e| e.to_string())?,
            Frame::new(b.pose.orientation.compose(fb)).map_err(|e| e.to_string())?,
        ))
    }
    /// Signed principal hinge angle, (-pi, pi]. Free rotation itself is unlimited;
    /// limits and position motors deliberately use a bounded principal range.
    pub fn angle_rad(&self, bodies: &[Body]) -> Result<f64> {
        self.validate(bodies.len())?;
        let (a, b) = self.frames(&bodies[self.body_a], &bodies[self.body_b])?;
        Ok(angle(a.orientation().conjugate().compose(b.orientation())))
    }
    pub(crate) fn motor(&self, bodies: &mut [Body], dt: f64) -> Result<Vec3> {
        let JointModel::Hinge {
            drive:
                RotationDrive::TorqueMotor {
                    target_rad,
                    stiffness_nm_per_rad,
                    damping_nms_per_rad,
                    max_torque_nm,
                },
            ..
        } = &self.model
        else {
            return Ok(Vec3::ZERO);
        };
        let (fa, fb) = self.frames(&bodies[self.body_a], &bodies[self.body_b])?;
        let n = fa.lateral();
        let theta = angle(fa.orientation().conjugate().compose(fb.orientation()));
        let relative = (bodies[self.body_b].angular_velocity_radps
            - bodies[self.body_a].angular_velocity_radps)
            .dot(n);
        let torque = (stiffness_nm_per_rad * (target_rad - theta) - damping_nms_per_rad * relative)
            .clamp(-max_torque_nm, *max_torque_nm);
        if !torque.is_finite() {
            return Err("motor torque overflow".into());
        }
        let impulse = n * (torque * dt);
        apply(bodies, self.body_a, self.body_b, impulse);
        Ok(impulse)
    }
    pub(crate) fn solve(
        &self,
        bodies: &mut [Body],
        dt: f64,
        limit_impulse: &mut f64,
    ) -> Result<Vec3> {
        let (fa, fb) = self.frames(&bodies[self.body_a], &bodies[self.body_b])?;
        let fixed = matches!(self.model, JointModel::Fixed { .. });
        let error = if fixed {
            fa.local_to_world(
                fa.orientation()
                    .conjugate()
                    .compose(fb.orientation())
                    .rotation_vector()
                    .map_err(|e| e.to_string())?,
            )
        } else {
            rotation_between(fa.lateral(), fb.lateral())
                .and_then(|q| q.rotation_vector())
                .map_err(|e| e.to_string())?
        };
        let mut total = Vec3::ZERO;
        for n in [fa.tangent(), fa.normal(), fa.lateral()]
            .into_iter()
            .take(if fixed { 3 } else { 2 })
        {
            let p = angular_impulse(bodies, self.body_a, self.body_b, n, error.dot(n), dt)?;
            apply(bodies, self.body_a, self.body_b, p);
            total = total + p;
        }
        if let JointModel::Hinge {
            limits: Some(l), ..
        } = self.model
        {
            let theta = angle(fa.orientation().conjugate().compose(fb.orientation()));
            let bound = if theta < l.min_rad {
                Some((l.min_rad, true))
            } else if theta > l.max_rad {
                Some((l.max_rad, false))
            } else {
                None
            };
            if let Some((bound, lower)) = bound {
                let n = fa.lateral();
                let candidate =
                    angular_impulse(bodies, self.body_a, self.body_b, n, theta - bound, dt)?.dot(n);
                let sum = if lower {
                    (*limit_impulse + candidate).max(0.)
                } else {
                    (*limit_impulse + candidate).min(0.)
                };
                let p = n * (sum - *limit_impulse);
                *limit_impulse = sum;
                apply(bodies, self.body_a, self.body_b, p);
                total = total + p;
            }
        }
        Ok(total)
    }
    pub(crate) fn error(&self, bodies: &[Body]) -> Result<f64> {
        let (a, b) = self.frames(&bodies[self.body_a], &bodies[self.body_b])?;
        Ok(match self.model {
            JointModel::Fixed { .. } => a
                .orientation()
                .conjugate()
                .compose(b.orientation())
                .rotation_vector()
                .map_err(|e| e.to_string())?
                .length(),
            JointModel::Hinge { limits, .. } => {
                let swing = rotation_between(a.lateral(), b.lateral())
                    .and_then(|q| q.rotation_vector())
                    .map_err(|e| e.to_string())?
                    .length();
                let theta = angle(a.orientation().conjugate().compose(b.orientation()));
                swing.max(limits.map_or(0., |l| (l.min_rad - theta).max(theta - l.max_rad).max(0.)))
            }
            JointModel::Ball => 0.,
        })
    }
}
fn angle(q: Quaternion) -> f64 {
    let a = 2. * q.z.atan2(q.w);
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
fn angular_impulse(
    bodies: &[Body],
    a: usize,
    b: usize,
    n: Vec3,
    error: f64,
    dt: f64,
) -> Result<Vec3> {
    let v = (bodies[b].angular_velocity_radps - bodies[a].angular_velocity_radps).dot(n);
    let mass = n.dot(bodies[a].inv_inertia(n) + bodies[b].inv_inertia(n));
    Ok(n * crate::solver::scalar_impulse(v, error, mass, dt)?)
}
fn apply(bodies: &mut [Body], a: usize, b: usize, p: Vec3) {
    bodies[a].angular_velocity_radps = bodies[a].angular_velocity_radps - bodies[a].inv_inertia(p);
    bodies[b].angular_velocity_radps = bodies[b].angular_velocity_radps + bodies[b].inv_inertia(p);
}
