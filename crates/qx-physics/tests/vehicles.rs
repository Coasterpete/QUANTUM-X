use qx_document::train::{
    DevelopmentArrangement, Train,
    mechanism::{AngleLimits, JointModel, ProfileDomain, RotationDrive, RotationKey},
};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    math::{Quaternion, Vec3},
    solver::{GenerationOptions, generate},
};
use qx_physics::{AngularJoint, BallJoint, Body, MassProperties, Pose, TrackSimulation, World};
use std::sync::Arc;
fn joint_world(model: JointModel) -> World {
    let body = || Body {
        mass: MassProperties::uniform_box(1., Vec3::new(1., 1., 1.)).unwrap(),
        pose: Pose {
            position_m: Vec3::ZERO,
            orientation: Quaternion::IDENTITY,
        },
        linear_velocity_mps: Vec3::ZERO,
        angular_velocity_radps: Vec3::ZERO,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    World {
        bodies: vec![body(), body()],
        joints: vec![BallJoint {
            body_a: 0,
            body_b: 1,
            anchor_a_m: Vec3::ZERO,
            anchor_b_m: Vec3::ZERO,
        }],
        angular_joints: vec![AngularJoint {
            body_a: 0,
            body_b: 1,
            model,
        }],
        guides: vec![],
        gravity_mps2: Vec3::ZERO,
        iterations: 32,
        substeps: 4,
    }
}
fn hinge(limits: Option<AngleLimits>, drive: RotationDrive) -> JointModel {
    JointModel::Hinge {
        frame_a: Quaternion::IDENTITY,
        frame_b: Quaternion::IDENTITY,
        limits,
        drive,
    }
}
fn track() -> Arc<qx_geometry::solver::GeneratedTrack> {
    Arc::new(
        generate(
            &InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::Z, -0.2).unwrap(),
                ..Default::default()
            },
            &[Section::straight(1, "incline", 200.)],
            GenerationOptions::default(),
        )
        .unwrap(),
    )
}
#[test]
fn hinge_has_one_free_axis_and_does_not_animate_body_poses() {
    let mut w = joint_world(hinge(None, RotationDrive::Free));
    w.bodies[1].angular_velocity_radps = Vec3::new(0., 0., 4.);
    let mut torque: f64 = 0.;
    for _ in 0..720 {
        let d = w.step().unwrap();
        torque = torque.max(d.angular_torque_on_b_nm[0].length());
        assert!(d.max_angular_error_rad < 0.003);
    }
    assert!(torque < 1e-10);
    assert!((w.bodies[1].angular_velocity_radps.z - 4.).abs() < 1e-9);
    let relative = w.angular_joints[0].angle_rad(&w.bodies).unwrap();
    assert!(
        (relative - (12_f64 + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            + std::f64::consts::PI)
            .abs()
            < 1e-8
    );
    w.bodies[1].angular_velocity_radps = Vec3::X;
    assert!(w.step().unwrap().angular_torque_on_b_nm[0].length() > 1.);
}
#[test]
fn fixed_joint_transmits_torque_and_corrects_orientation() {
    let mut w = joint_world(JointModel::Fixed {
        frame_a: Quaternion::IDENTITY,
        frame_b: Quaternion::IDENTITY,
    });
    w.bodies[1].pose.orientation = Quaternion::axis_angle(Vec3::Y, 0.2).unwrap();
    w.bodies[1].torque_nm = Vec3::new(0., 0.5, 0.);
    for _ in 0..240 {
        w.step().unwrap();
    }
    assert!(w.step().unwrap().max_angular_error_rad < 1e-8);
    assert!(w.bodies[0].angular_velocity_radps.y > 1.4);
    let momentum =
        (w.bodies[0].angular_velocity_radps.y + w.bodies[1].angular_velocity_radps.y) / 6.;
    assert!((momentum - 0.5 * 241. / 240.).abs() < 1e-10);
}
#[test]
fn hinge_limits_are_unilateral_and_motor_torque_is_bounded() {
    let mut w = joint_world(hinge(
        Some(AngleLimits {
            min_rad: -0.4,
            max_rad: 0.4,
        }),
        RotationDrive::Free,
    ));
    w.bodies[1].angular_velocity_radps = Vec3::Z * 8.;
    let mut excursion: f64 = 0.;
    for _ in 0..240 {
        w.step().unwrap();
        excursion = excursion.max(w.angular_joints[0].angle_rad(&w.bodies).unwrap().abs());
    }
    assert!(excursion < 0.41, "{excursion}");
    // Reverse the angular velocities: a unilateral upper stop must let the hinge leave it.
    w.bodies[0].angular_velocity_radps = Vec3::Z;
    w.bodies[1].angular_velocity_radps = -Vec3::Z;
    let before = w.angular_joints[0].angle_rad(&w.bodies).unwrap();
    for _ in 0..12 {
        w.step().unwrap();
    }
    assert!(w.angular_joints[0].angle_rad(&w.bodies).unwrap() < before - 0.09);
    let mut m = joint_world(hinge(
        None,
        RotationDrive::TorqueMotor {
            target_rad: 0.4,
            stiffness_nm_per_rad: 5.,
            damping_nms_per_rad: 1.,
            max_torque_nm: 0.2,
        },
    ));
    for _ in 0..960 {
        let d = m.step().unwrap();
        assert!(d.angular_torque_on_b_nm[0].length() <= 0.200000001);
        assert!(
            (m.bodies[0].angular_velocity_radps.z + m.bodies[1].angular_velocity_radps.z).abs()
                < 1e-10
        );
    }
    assert!((m.angular_joints[0].angle_rad(&m.bodies).unwrap() - 0.4).abs() < 0.001);
}
#[test]
fn different_arrangements_replay_reset_and_render_origins_use_mass_centers() {
    for arrangement in [
        DevelopmentArrangement::Conventional,
        DevelopmentArrangement::BelowTrack,
        DevelopmentArrangement::RotatingSeats,
    ] {
        let train = Train::development_consist("fixture".into(), 3, arrangement).unwrap();
        let mut s = TrackSimulation::new(train.clone(), track(), 80., 2.).unwrap();
        let initial = s.snapshot().unwrap();
        let mut replay = s.clone();
        let mut max_joint: f64 = 0.;
        let mut max_guide: f64 = 0.;
        for _ in 0..240 {
            let d = s.step().unwrap();
            assert_eq!(d, replay.step().unwrap());
            max_joint = max_joint.max(d.max_joint_error_m);
            max_guide = max_guide.max(d.max_guide_error_m);
        }
        assert_eq!(s.world(), replay.world());
        assert_eq!(s.guides(), replay.guides());
        assert_eq!(s.ticks(), 240);
        assert!(
            max_joint < 0.02 && max_guide < 0.02,
            "{arrangement:?}: joint {max_joint} guide {max_guide}"
        );
        let end = s.snapshot().unwrap();
        assert_ne!(end.components[0].pose, initial.components[0].pose);
        if arrangement == DevelopmentArrangement::RotatingSeats {
            let i = train
                .components
                .iter()
                .position(|c| c.id == "seat-1")
                .unwrap();
            assert_ne!(
                initial.components[i].pose.orientation,
                end.components[i].pose.orientation
            );
            let b = &s.world().bodies[i];
            let com = b
                .world_point(-train.components[i].center_of_mass_m)
                .unwrap();
            assert!((com - end.components[i].pose.position_m).length() < 1e-10);
            assert_eq!(s.world().angular_joints.len(), 3);
        }
        s.reset();
        assert_eq!(s.ticks(), 0);
        assert_eq!(s.snapshot().unwrap(), initial);
    }
}
#[test]
fn controlled_mechanical_profile_is_preserved_and_rejected_by_unimplemented_solver() {
    let mut train = Train::development_consist(
        "4d-representation".into(),
        1,
        DevelopmentArrangement::RotatingSeats,
    )
    .unwrap();
    let j = train.connections.last_mut().unwrap();
    if let JointModel::Hinge { drive, .. } = &mut j.model {
        *drive = RotationDrive::MechanicalProfile {
            domain: ProfileDomain::TrackDistanceMetres,
            keys: vec![
                RotationKey {
                    input: 0.,
                    angle_rad: 0.,
                },
                RotationKey {
                    input: 10.,
                    angle_rad: 0.5,
                },
            ],
            transmission_id: "mechanical-rack".into(),
        };
    }
    train.validate().unwrap();
    assert!(qx_physics::consist::inspect(&train, &track(), 80.).is_ok());
    let error = TrackSimulation::new(train, track(), 80., 0.).unwrap_err();
    assert!(error.contains("transmission dynamics"), "{error}");
}
#[test]
fn failed_endpoint_step_retains_bodies_hints_and_clock() {
    let train = Train::development_fixture("endpoint".into());
    let mut s = TrackSimulation::new(train, track(), 196., 10.).unwrap();
    for _ in 0..240 {
        let (world, guides, ticks) = (s.world().clone(), s.guides().to_vec(), s.ticks());
        if s.step().is_err() {
            assert_eq!(s.world(), &world);
            assert_eq!(s.guides(), guides);
            assert_eq!(s.ticks(), ticks);
            return;
        }
    }
    panic!("expected the open endpoint to stop playback");
}

#[test]
fn revolute_frames_support_arbitrary_axes() {
    for axis in [
        Vec3::X,
        Vec3::Y,
        Vec3::new(1., 2., 3.).normalized().unwrap(),
    ] {
        let frame = qx_geometry::math::rotation_between(Vec3::Z, axis).unwrap();
        let mut w = joint_world(JointModel::Hinge {
            frame_a: frame,
            frame_b: frame,
            limits: None,
            drive: RotationDrive::Free,
        });
        w.bodies[1].angular_velocity_radps = axis * 3.;
        for _ in 0..120 {
            assert!(w.step().unwrap().max_angular_error_rad < 1e-10);
        }
        assert!((w.bodies[1].angular_velocity_radps - axis * 3.).length() < 1e-10);
        assert!((w.angular_joints[0].angle_rad(&w.bodies).unwrap() - 1.5).abs() < 1e-10);
    }
}

#[test]
fn mounts_resolve_out_of_order_and_never_replace_mechanical_joints() {
    let mut train = Train::development_consist(
        "initial-mounts".into(),
        1,
        DevelopmentArrangement::RotatingSeats,
    )
    .unwrap();
    train.components.reverse();
    let connected = qx_physics::consist::inspect(&train, &track(), 80.).unwrap();
    assert_eq!(connected.components[0].id, "seat-1");
    // Remove the physical hinge, retaining its mount. The seat must now fall independently.
    train
        .connections
        .retain(|j| !j.id.starts_with("seat-hinge"));
    let mut sim = TrackSimulation::new(train, track(), 80., 0.).unwrap();
    let initial = sim.world().bodies[0].clone();
    for _ in 0..240 {
        sim.step().unwrap();
    }
    let seat = &sim.world().bodies[0];
    assert!((seat.linear_velocity_mps.y + 9.81).abs() < 1e-10);
    // Semi-implicit Euler: N=240*4 substeps, y-y0 = -g*dt²*N*(N+1)/2.
    let expected = -4.905 * (1. + 1. / 960.);
    assert!((seat.pose.position_m.y - initial.pose.position_m.y - expected).abs() < 1e-10);
    assert_eq!(seat.pose.orientation, initial.pose.orientation);
}
