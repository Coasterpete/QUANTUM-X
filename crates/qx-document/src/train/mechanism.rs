//! Component origins, mass centers and joint frames are separate from visual GLB corrections.
use crate::{Result, require};
use qx_geometry::math::{Quaternion, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalFrame {
    pub translation_m: Vec3,
    pub rotation: Quaternion,
}
impl LocalFrame {
    pub fn validate(&self) -> Result<()> {
        require(
            self.translation_m.is_finite(),
            "nonfinite mounting translation",
        )?;
        unit(self.rotation)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Placement {
    /// Signed station relative to the lead origin; local frame is relative to the track frame.
    Track {
        offset_m: f64,
        frame: LocalFrame,
    },
    Mounted {
        parent: String,
        frame: LocalFrame,
    },
}
impl Placement {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Track { offset_m, frame } => {
                require(offset_m.is_finite(), "nonfinite placement station offset")?;
                frame.validate()
            }
            Self::Mounted { parent, frame } => {
                require(super::valid_id(parent), "invalid mount parent")?;
                frame.validate()
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AngleLimits {
    pub min_rad: f64,
    pub max_rad: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDomain {
    TimeSeconds,
    TrackDistanceMetres,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationKey {
    pub input: f64,
    pub angle_rad: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum RotationDrive {
    #[default]
    Free,
    /// Finite equal-and-opposite torque; this does not prescribe a body orientation.
    TorqueMotor {
        target_rad: f64,
        stiffness_nm_per_rad: f64,
        damping_nms_per_rad: f64,
        max_torque_nm: f64,
    },
    /// Representable controlled mechanism. Requires a future mechanical transmission solver.
    MechanicalProfile {
        domain: ProfileDomain,
        keys: Vec<RotationKey>,
        transmission_id: String,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum JointModel {
    #[default]
    Ball,
    Fixed {
        frame_a: Quaternion,
        frame_b: Quaternion,
    },
    /// Joint-local Z is the hinge axis; X defines zero angle. Frames are component-local.
    Hinge {
        frame_a: Quaternion,
        frame_b: Quaternion,
        limits: Option<AngleLimits>,
        drive: RotationDrive,
    },
}
fn unit(q: Quaternion) -> Result<()> {
    let n = q.w.hypot(q.x).hypot(q.y).hypot(q.z);
    require(
        n.is_finite() && (n - 1.).abs() < 1e-8,
        "mount/joint rotation must be a unit quaternion",
    )
}
impl JointModel {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Ball => Ok(()),
            Self::Fixed { frame_a, frame_b } => {
                unit(*frame_a)?;
                unit(*frame_b)
            }
            Self::Hinge {
                frame_a,
                frame_b,
                limits,
                drive,
            } => {
                unit(*frame_a)?;
                unit(*frame_b)?;
                if let Some(l) = limits {
                    require(
                        l.min_rad.is_finite()
                            && l.max_rad.is_finite()
                            && l.min_rad < l.max_rad
                            && l.min_rad > -std::f64::consts::PI
                            && l.max_rad < std::f64::consts::PI,
                        "hinge limits must increase strictly inside (-pi, pi)",
                    )?;
                }
                match drive {
                    RotationDrive::Free => Ok(()),
                    RotationDrive::TorqueMotor {
                        target_rad,
                        stiffness_nm_per_rad,
                        damping_nms_per_rad,
                        max_torque_nm,
                    } => require(
                        target_rad.is_finite()
                            && target_rad.abs() < std::f64::consts::PI
                            && limits.is_none_or(|l| {
                                *target_rad >= l.min_rad && *target_rad <= l.max_rad
                            })
                            && stiffness_nm_per_rad.is_finite()
                            && *stiffness_nm_per_rad >= 0.
                            && damping_nms_per_rad.is_finite()
                            && *damping_nms_per_rad >= 0.
                            && max_torque_nm.is_finite()
                            && *max_torque_nm > 0.,
                        "invalid hinge torque motor",
                    ),
                    RotationDrive::MechanicalProfile {
                        keys,
                        transmission_id,
                        ..
                    } => require(
                        super::valid_id(transmission_id)
                            && keys.len() >= 2
                            && keys.len() <= 4096
                            && keys.iter().all(|k| {
                                k.input.is_finite()
                                    && k.input >= 0.
                                    && k.angle_rad.is_finite()
                                    && limits.is_none_or(|l| {
                                        k.angle_rad >= l.min_rad && k.angle_rad <= l.max_rad
                                    })
                            })
                            && keys.windows(2).all(|p| p[0].input < p[1].input),
                        "invalid mechanical rotation profile",
                    ),
                }
            }
        }
    }
}
