use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::{GeneratedTrack, GenerationOptions, generate},
};
use qx_physics::{
    BallJoint, Body, MassProperties, Pose, TrackGuide, World,
    inspection::{InspectionReference, inspect},
};

fn track(rotation: Quaternion) -> GeneratedTrack {
    let mut section = Section::straight(1, "arc", 45.0);
    section.kind = SectionKind::Arc {
        radius_m: 40.0,
        plane_angle_rad: 0.0,
    };
    generate(
        &InitialConditions {
            orientation: rotation,
            ..Default::default()
        },
        &[section],
        GenerationOptions::default(),
    )
    .unwrap()
}
fn world(track: &GeneratedTrack) -> (World, Vec<TrackGuide>) {
    let a = track.sample_at_distance(5.0).unwrap();
    let b = track.sample_at_distance(7.0).unwrap();
    let make = |position, mass| Body {
        mass: MassProperties::uniform_box(mass, Vec3::new(1., 1., 1.)).unwrap(),
        pose: Pose {
            position_m: position,
            orientation: Quaternion::IDENTITY,
        },
        linear_velocity_mps: Vec3::ZERO,
        angular_velocity_radps: Vec3::ZERO,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    let half = (b.position_m - a.position_m) * 0.5;
    (
        World {
            angular_joints: vec![],
            bodies: vec![make(a.position_m, 400.), make(b.position_m, 600.)],
            joints: vec![BallJoint {
                body_a: 0,
                body_b: 1,
                anchor_a_m: half,
                anchor_b_m: -half,
            }],
            guides: vec![],
            gravity_mps2: Vec3::new(0., -9.81, 0.),
            iterations: 32,
            substeps: 2,
        },
        vec![
            TrackGuide {
                body: 0,
                local_point_m: Vec3::ZERO,
                station_m: 5.,
            },
            TrackGuide {
                body: 1,
                local_point_m: Vec3::ZERO,
                station_m: 7.,
            },
        ],
    )
}
#[test]
fn connected_bodies_travel_on_curved_track_deterministically() {
    let t = track(Quaternion::axis_angle(Vec3::Z, -0.6).unwrap());
    let (mut w, mut guides) = world(&t);
    let (mut replay, mut replay_guides) = (w.clone(), guides.clone());
    let initial = guides[0].station_m;
    let mut joint: f64 = 0.;
    let mut guide: f64 = 0.;
    for _ in 0..240 {
        let d = w.step_on_track(&t, &mut guides).unwrap();
        assert_eq!(d, replay.step_on_track(&t, &mut replay_guides).unwrap());
        joint = joint.max(d.max_joint_error_m);
        guide = guide.max(d.max_guide_error_m);
        assert_eq!(d.guide_force_n.len(), 2);
    }
    assert_eq!(w, replay);
    assert_eq!(guides, replay_guides);
    assert!(guides[0].station_m > initial + 0.5);
    assert!(joint < 0.01 && guide < 0.01, "joint={joint} guide={guide}");
}
#[test]
fn banked_inspection_and_open_endpoint_rollback() {
    let t = track(Quaternion::axis_angle(Vec3::X, 2.8).unwrap());
    let poses = inspect(
        &t,
        10.,
        &[InspectionReference {
            component_id: "bogie".into(),
            station_offset_m: 1.,
            local_offset_m: Vec3::Y,
        }],
    )
    .unwrap();
    let sample = t.sample_at_distance(11.).unwrap();
    assert!(
        (poses[0].pose.position_m - sample.position_m - sample.frame.normal()).length() < 1e-12
    );
    assert!(
        inspect(
            &t,
            0.,
            &[InspectionReference {
                component_id: "x".into(),
                station_offset_m: -1.,
                local_offset_m: Vec3::ZERO
            }]
        )
        .is_err()
    );
    let (mut w, mut guides) = world(&t);
    w.bodies.truncate(1);
    w.joints.clear();
    guides.truncate(1);
    guides[0].station_m = 0.;
    w.bodies[0].pose.position_m = t.samples[0].position_m;
    w.bodies[0].linear_velocity_mps = -t.samples[0].frame.tangent() * 10.;
    let old = w.clone();
    let old_guides = guides.clone();
    assert!(w.step_on_track(&t, &mut guides).is_err());
    assert_eq!(w, old);
    assert_eq!(guides, old_guides);
}

#[test]
fn inverted_loop_reports_centripetal_reaction_and_bounded_energy_drift() {
    let radius = 10.0;
    let mut section = Section::straight(1, "loop", std::f64::consts::TAU * radius);
    section.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: 0.0,
    };
    let t = generate(
        &InitialConditions::default(),
        &[section],
        GenerationOptions::default(),
    )
    .unwrap();
    let sample = t.sample_at_distance(std::f64::consts::PI * radius).unwrap();
    let mass = 100.;
    let speed = 8.;
    let body = Body {
        mass: MassProperties::uniform_box(mass, Vec3::new(1., 1., 1.)).unwrap(),
        pose: Pose {
            position_m: sample.position_m,
            orientation: sample.frame.orientation(),
        },
        linear_velocity_mps: sample.frame.tangent() * speed,
        angular_velocity_radps: Vec3::ZERO,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    let mut w = World {
        angular_joints: vec![],
        bodies: vec![body],
        joints: vec![],
        guides: vec![],
        gravity_mps2: Vec3::ZERO,
        iterations: 16,
        substeps: 2,
    };
    let mut guides = vec![TrackGuide {
        body: 0,
        local_point_m: Vec3::ZERO,
        station_m: sample.distance_m,
    }];
    let mut max_error: f64 = 0.;
    let mut force = 0.;
    for _ in 0..240 {
        let d = w.step_on_track(&t, &mut guides).unwrap();
        max_error = max_error.max(d.max_guide_error_m);
        force += d.guide_force_n[0].length() / 240.;
    }
    let drift = (w.bodies[0].linear_velocity_mps.length() / speed - 1.).abs();
    eprintln!("loop max error {max_error:.8}, speed drift {drift:.8}, mean reaction {force:.3} N");
    assert!(sample.frame.normal().y < -0.99);
    assert!(max_error < 0.01 && drift < 0.01);
    assert!((force - mass * speed * speed / radius).abs() < 20.);
}
