//! Explicit mechanical research fixtures. No existing production guides are removed.
use qx_document::train::{
    Connection, DevelopmentArrangement, Train,
    mechanism::{AngleLimits, JointModel, LocalFrame, Placement, RotationDrive},
};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    rail_geometry::{RailReference, StationWindow, WheelCenterPath},
};
use qx_physics::{
    consist::ConsistInspection,
    wheel_geometry::{WheelGeometry, WheelRole, WheelTarget},
};

pub const HINGE: &str = "carrier-bearing";
pub const CARRIER: &str = "wheel-carrier";
pub const PIVOT: Vec3 = Vec3::new(0.07, 0.0, -0.03);
pub const TARGET_ANGLE: f64 = 0.12;

/// An additional, explicitly authored yaw-hinged carrier on the front gear.
/// Its single guide IS the bearing pivot; its rail attitude still constrains
/// roll. The parent's original paired guides and all suspension edges remain.
pub fn carrier(suspended: bool) -> Train {
    let mut train = Train::development_consist(
        "m3b-carrier".into(),
        1,
        if suspended {
            DevelopmentArrangement::BelowTrack
        } else {
            DevelopmentArrangement::Conventional
        },
    )
    .unwrap();
    let mut c = train.components[1].clone();
    c.id = CARRIER.into();
    c.guide_references_m = vec![PIVOT];
    c.wheel_references_m.clear();
    c.placement = Some(Placement::Mounted {
        parent: "gear-1-1".into(),
        frame: LocalFrame {
            translation_m: -PIVOT,
            ..Default::default()
        },
    });
    train.components.push(c);
    let joint_frame = Quaternion::axis_angle(Vec3::X, -std::f64::consts::FRAC_PI_2).unwrap();
    train.connections.push(Connection {
        id: HINGE.into(),
        component_a: "gear-1-1".into(),
        component_b: CARRIER.into(),
        anchor_a_m: Vec3::ZERO,
        anchor_b_m: PIVOT,
        model: JointModel::Hinge {
            frame_a: joint_frame,
            frame_b: joint_frame,
            limits: Some(AngleLimits {
                min_rad: -0.25,
                max_rad: 0.25,
            }),
            drive: RotationDrive::Free,
        },
    });
    train.validate().unwrap();
    train
}

pub fn wheels(train: &Train, poses: &ConsistInspection, carrier_only: bool) -> Vec<WheelGeometry> {
    let mut wheels = vec![];
    for c in &train.components {
        if !c.kind.is_running_gear() || (carrier_only && c.id != CARRIER) {
            continue;
        }
        let station = poses
            .components
            .iter()
            .find(|p| p.id == c.id)
            .unwrap()
            .station_m;
        for x in [-0.3, 0.3] {
            for z in [-0.6, 0.6] {
                let offset = Vec3::new(x, 0.18, z);
                let center_local_m = if c.id == CARRIER {
                    PIVOT
                        + Frame::new(Quaternion::axis_angle(Vec3::Y, -TARGET_ANGLE).unwrap())
                            .unwrap()
                            .local_to_world(offset)
                } else {
                    offset
                };
                wheels.push(WheelGeometry {
                    id: format!("{}-{x}-{z}", c.id),
                    component_id: c.id.clone(),
                    center_local_m,
                    orientation_local: if c.id == CARRIER {
                        Quaternion::axis_angle(Vec3::Y, -TARGET_ANGLE).unwrap()
                    } else {
                        Quaternion::IDENTITY
                    },
                    role: Some(WheelRole::Running),
                    target: WheelTarget::Declared(WheelCenterPath {
                        rail: RailReference {
                            id: format!("test-rail-{z}"),
                            offset_m: Vec3::new(0., 0., z),
                        },
                        center_offset_m: Vec3::Y * 0.18,
                    }),
                    window: StationWindow {
                        min_m: station + x - 1.,
                        max_m: station + x + 1.,
                    },
                });
            }
        }
    }
    wheels
}
