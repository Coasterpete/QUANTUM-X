//! Frozen legacy control, also compiled against untouched M4I for M4J verification.
use qx_document::{
    operations::{DeviceKind, Operations, TrackDevice},
    train::{
        DevelopmentArrangement, Train,
        mechanism::{JointModel, RotationDrive},
    },
};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::generate,
    transition::{Channel, Unit},
};
use qx_physics::TrackSimulation;
use std::sync::Arc;

#[test]
fn legacy_initialization_and_stepping_fingerprint() {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut record = |text: String| {
        for byte in text.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    };
    for curved in [false, true] {
        for arrangement in [
            DevelopmentArrangement::Conventional,
            DevelopmentArrangement::BelowTrack,
            DevelopmentArrangement::RotatingSeats,
        ] {
            let initial = InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::X, 0.2).unwrap(),
                ..Default::default()
            };
            let mut section = Section::straight(1, "Legacy frozen control", 160.);
            if curved {
                section.kind = SectionKind::Arc {
                    radius_m: 80.,
                    plane_angle_rad: 1.2,
                };
                section.roll_rate = Channel::constant(Unit::RadiansPerMetre, 160., 0.01);
            }
            let track = Arc::new(generate(&initial, &[section], Default::default()).unwrap());
            let mut train = Train::development_consist("legacy".into(), 3, arrangement).unwrap();
            for j in &mut train.connections {
                if let JointModel::Hinge { drive, .. } = &mut j.model {
                    *drive = RotationDrive::TorqueMotor {
                        target_rad: 0.1,
                        stiffness_nm_per_rad: 100.,
                        damping_nms_per_rad: 10.,
                        max_torque_nm: 80.,
                    };
                }
            }
            let mut op = Operations::default();
            op.physical.initial_speed_mps = -6.;
            op.devices
                .push(TrackDevice::new(1, DeviceKind::Launch, 0., 160.));
            let mut sim = TrackSimulation::with_operations(train, track, 80., op).unwrap();
            let initial_world = sim.world().clone();
            assert!(
                initial_world
                    .bodies
                    .iter()
                    .all(|b| b.angular_velocity_radps == Vec3::ZERO)
            );
            record(format!(
                "{:?}{:?}{:?}",
                sim.world(),
                sim.guides(),
                sim.telemetry()
            ));
            for _ in 0..12 {
                let d = sim.step().unwrap();
                record(format!(
                    "{:?}{:?}{:?}{:?}",
                    sim.world(),
                    sim.guides(),
                    sim.telemetry(),
                    d
                ));
            }
            sim.reset();
            assert_eq!(sim.world(), &initial_world);
            record(format!(
                "{:?}{:?}{:?}",
                sim.world(),
                sim.guides(),
                sim.telemetry()
            ));
        }
    }
    println!("M4J_LEGACY_FINGERPRINT {hash:016x}");
}
