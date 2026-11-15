//! Developer capture utility. Always writes a NEW directory for deliberate review.
#[path = "../tests/support/replay_baseline.rs"]
mod baseline;
use baseline::{Golden, Input, run};
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
    topology::LayoutMode,
    transition::{Channel, Unit},
};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a NEW output directory")?,
    );
    fs::create_dir(&output)?;
    for name in [
        "level",
        "grade",
        "launch",
        "brake_reverse",
        "banked_curve",
        "rotating_seats",
        "circuit_seam",
    ] {
        let mut input = Input {
            name: name.into(),
            initial: InitialConditions::default(),
            sections: vec![Section::straight(1, "Development regression track", 200.)],
            train: Train::development_consist(
                "replay-train".into(),
                3,
                DevelopmentArrangement::Conventional,
            )?,
            operations: Operations::default(),
            station_m: 80.,
            checkpoints: vec![0, 1, 12, 60, 120, 240],
        };
        input.operations.physical.initial_speed_mps = 3.;
        match name {
            "grade" => {
                input.initial.orientation = Quaternion::axis_angle(Vec3::Z, -10_f64.to_radians())?
            }
            "launch" => {
                let mut d = TrackDevice::new(7, DeviceKind::Launch, 65., 80.);
                d.acceleration_mps2 = 2.;
                d.maximum_force_n = 1000.; // Partial occupancy and an active force cap.
                input.operations.devices.push(d);
            }
            "brake_reverse" => {
                input.operations.physical.initial_speed_mps = -3.;
                input.operations.physical.rolling_resistance_coefficient = 0.01;
                input.operations.physical.drag_area_m2 = 1.5;
                input
                    .operations
                    .devices
                    .push(TrackDevice::new(9, DeviceKind::Brake, 0., 200.));
            }
            "banked_curve" | "rotating_seats" => {
                input.sections[0].extent = 160.;
                input.sections[0].kind = SectionKind::Arc {
                    radius_m: 80.,
                    plane_angle_rad: 1.2,
                };
                input.sections[0].roll_rate = Channel::constant(Unit::RadiansPerMetre, 160., 0.01);
                input.initial.orientation = Quaternion::axis_angle(Vec3::X, 0.2)?;
                input.checkpoints = vec![0, 1, 12, 60, 120];
                if name == "rotating_seats" {
                    input.train = Train::development_consist(
                        "replay-train".into(),
                        3,
                        DevelopmentArrangement::RotatingSeats,
                    )?;
                    for j in &mut input.train.connections {
                        if let JointModel::Hinge { drive, .. } = &mut j.model {
                            *drive = RotationDrive::TorqueMotor {
                                target_rad: 0.1,
                                stiffness_nm_per_rad: 100.,
                                damping_nms_per_rad: 10.,
                                max_torque_nm: 80.,
                            };
                        }
                    }
                }
            }
            "circuit_seam" => {
                let length = 30. * std::f64::consts::TAU;
                input.sections[0].extent = length;
                input.sections[0].roll_rate = Channel::constant(Unit::RadiansPerMetre, length, 0.);
                input.sections[0].kind = SectionKind::Arc {
                    radius_m: 30.,
                    plane_angle_rad: std::f64::consts::FRAC_PI_2,
                };
                input.train = Train::development_consist(
                    "replay-train".into(),
                    1,
                    DevelopmentArrangement::Conventional,
                )?;
                input.operations.layout = LayoutMode::Circuit;
                input.operations.physical.initial_speed_mps = 5.;
                input.operations.devices.push(TrackDevice::new(
                    11,
                    DeviceKind::Launch,
                    length - 10.,
                    10.,
                ));
                input.station_m = length - 5.;
                input.checkpoints = vec![0, 1, 12, 120, 240, 360];
            }
            "level" => {}
            _ => unreachable!(),
        }
        let states = run(&input);
        let golden = Golden {
            format_version: 1,
            fixed_step_seconds: qx_physics::FIXED_STEP_SECONDS,
            input,
            states,
        };
        fs::write(
            output.join(format!("{name}.json")),
            format!("{}\n", serde_json::to_string_pretty(&golden)?),
        )?;
        println!("captured {name}");
    }
    Ok(())
}
