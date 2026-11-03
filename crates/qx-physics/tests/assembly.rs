use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Frame, Quaternion, Vec3},
    solver::{GeneratedTrack, GenerationOptions, generate},
};
use qx_physics::{
    TrackSimulation,
    consist::{initialize, inspect, interpolate},
};
use std::sync::Arc;

fn arc(radius: f64, bank: f64, plane: f64) -> GeneratedTrack {
    let mut section = Section::straight(1, "Synthetic assembly reference", radius.abs() * 7.);
    section.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: plane,
    };
    generate(
        &InitialConditions {
            orientation: Quaternion::axis_angle(Vec3::X, bank).unwrap(),
            ..Default::default()
        },
        &[section],
        GenerationOptions::default(),
    )
    .unwrap()
}
fn conventional(n: usize) -> Train {
    Train::development_consist(
        "arbitrary-consist".into(),
        n,
        DevelopmentArrangement::Conventional,
    )
    .unwrap()
}
#[test]
fn rigid_chords_close_on_crests_valleys_banked_turns_and_every_loop_quadrant() {
    for (radius, bank, plane) in [
        (8., 0., 0.),
        (8., 0., std::f64::consts::PI),
        (15., 0., std::f64::consts::FRAC_PI_2),
        (15., 0.7, std::f64::consts::FRAC_PI_2),
        (12., std::f64::consts::PI, 0.),
    ] {
        let t = arc(radius, bank, plane);
        let train = conventional(1);
        for fraction in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let s = t.length_m() * fraction;
            let result = inspect(&train, &t, s)
                .unwrap_or_else(|e| panic!("{radius}/{bank}/{plane}@{s}: {e}"));
            assert!(result.joints.iter().all(|j| j.error_m <= 1e-5));
            let gears: Vec<_> = result
                .components
                .iter()
                .filter(|c| c.id.starts_with("gear"))
                .collect();
            // Physical separation remains the 2.4 m authored chord, even on tight curves.
            assert!(
                ((gears[0].pose.position_m - gears[1].pose.position_m).length() - 2.4).abs() < 3e-5
            );
            assert!((gears[0].station_m - gears[1].station_m).abs() > 2.4);
            initialize(&train, &t, s, Vec3::ZERO, 32, 4).unwrap();
            assert_eq!(result, inspect(&train, &t, s).unwrap());
        }
    }
}
#[test]
fn consecutive_coupling_closes_without_changing_dimensions_or_authored_stations() {
    for plane in [0., std::f64::consts::FRAC_PI_2] {
        let t = arc(30., 0.4, plane);
        let train = conventional(3);
        let unchanged = train.clone();
        let p = inspect(&train, &t, 50.).unwrap();
        assert!(p.joints.iter().all(|j| j.error_m < 1e-5));
        assert_eq!(train, unchanged);
        let mut simulation = TrackSimulation::new(train, Arc::new(t), 50., 3.).unwrap();
        for _ in 0..120 {
            let d = simulation.step().unwrap();
            assert!(d.max_joint_error_m < 0.02 && d.max_guide_error_m < 0.02);
        }
    }
}
#[test]
fn unequal_cars_offset_pivots_and_arbitrary_ids_are_preserved() {
    let t = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "fixture", 100.)],
        GenerationOptions::default(),
    )
    .unwrap();
    let mut train = conventional(3);
    train.components[3].dimensions_m.x = 4.5;
    train.components[3].mass_kg = 975.;
    train
        .connections
        .iter_mut()
        .filter(|j| j.component_a == "car-2" && j.component_b.starts_with("gear"))
        .for_each(|j| j.anchor_a_m.x *= 1.3);
    // Shift both pivots asymmetrically within car 2; this changes the origin,
    // while the coupler search must still close the actual authored anchors.
    for j in &mut train.connections {
        if j.component_a == "car-2" && j.component_b.starts_with("gear") {
            j.anchor_a_m.x += 0.2;
        }
    }
    for c in &mut train.components {
        if c.id == "car-2" {
            c.id = "arbitrary body ID 723".into();
        }
    }
    train.cars[1].body_id = "arbitrary body ID 723".into();
    for j in &mut train.connections {
        if j.component_a == "car-2" {
            j.component_a = "arbitrary body ID 723".into();
        }
        if j.component_b == "car-2" {
            j.component_b = "arbitrary body ID 723".into();
        }
    }
    train.components.reverse();
    let p = inspect(&train, &t, 50.).unwrap();
    assert!(p.joints.iter().all(|j| j.error_m < 1e-5));
    assert!(p.components.iter().any(|c| c.id == "arbitrary body ID 723"));
}
#[test]
fn shared_single_and_more_than_two_gear_are_not_globally_rejected() {
    let t = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "fixture", 100.)],
        GenerationOptions::default(),
    )
    .unwrap();
    let mut train = conventional(2);
    // Shared identity stays a single body, with an explicit physical connection.
    train.cars[1].bogie_ids.push("gear-1-2".into());
    let mut j = train.connections[0].clone();
    j.id = "shared".into();
    j.component_a = "car-2".into();
    j.component_b = "gear-1-2".into();
    j.anchor_a_m = Vec3::new(2.8, -0.8, 0.);
    train.connections.push(j);
    let result = inspect(&train, &t, 50.).unwrap();
    assert_eq!(result.components.len(), 7); // Six vehicle bodies plus the independent drawbar.
    let mut single = conventional(1);
    single.cars[0].bogie_ids.truncate(1);
    single.components.retain(|c| c.id != "gear-1-2");
    single.connections.retain(|j| j.component_b != "gear-1-2");
    assert_eq!(inspect(&single, &t, 50.).unwrap().components.len(), 2);
    initialize(&single, &t, 50., Vec3::ZERO, 32, 4).unwrap();
}
#[test]
fn incompatible_guides_and_open_ends_are_errors_and_do_not_edit_input() {
    let t = arc(8., 0., 0.);
    let mut train = conventional(1);
    assert!(inspect(&train, &t, 0.).is_err());
    assert!(inspect(&train, &t, t.length_m()).is_err());
    train.components[1]
        .guide_references_m
        .push(Vec3::new(0., 1., 0.));
    let before = train.clone();
    let error = inspect(&train, &t, 20.).unwrap_err();
    assert!(
        error.contains("overconstrained") || error.contains("projection"),
        "{error}"
    );
    assert_eq!(before, train);
}
#[test]
fn off_axis_asymmetric_physical_pivots_close_on_a_hill() {
    let t = arc(12., 0.6, 0.);
    let mut train = conventional(1);
    train.connections[0].anchor_a_m = Vec3::new(1.5, -0.7, 0.1);
    train.connections[1].anchor_a_m = Vec3::new(-0.8, -0.9, -0.05);
    train.connections[0].anchor_b_m = Vec3::new(0.05, 0.15, 0.);
    train.connections[1].anchor_b_m = Vec3::new(-0.03, 0.1, 0.);
    let initial = train.clone();
    let result = inspect(&train, &t, 25.).unwrap();
    assert!(result.joints.iter().all(|j| j.error_m < 1e-5));
    initialize(&train, &t, 25., Vec3::ZERO, 32, 4).unwrap();
    assert_eq!(train, initial);
}
#[test]
fn committed_pair_and_visual_interpolation_do_not_advance_physics() {
    let t = Arc::new(arc(40., 0.3, 0.));
    let mut sim = TrackSimulation::new(conventional(1), t, 30., 3.).unwrap();
    let first = sim.snapshot().unwrap();
    sim.step().unwrap();
    assert_eq!(sim.previous_snapshot(), &first);
    let current = sim.snapshot().unwrap();
    assert_eq!(interpolate(&first, &current, 0.).unwrap(), first);
    assert_eq!(interpolate(&first, &current, 1.).unwrap(), current);
    for hz in [30, 60, 144] {
        for frame in 0..hz {
            let alpha = frame as f64 / hz as f64;
            let p = interpolate(&first, &current, alpha).unwrap();
            let a = &p.components[0];
            assert!(Frame::new(a.pose.orientation).is_ok());
        }
    }
    assert_eq!(sim.ticks(), 1);
    sim.reset();
    assert_eq!(sim.previous_snapshot(), &first);
    assert_eq!(sim.snapshot().unwrap(), first);
    assert!(interpolate(&first, &current, f64::NAN).is_err());
}

#[test]
fn twisting_assembly_sweeps_close_position_and_angular_residuals_at_original_tolerance() {
    use qx_geometry::transition::{Channel, Function, Transition, Unit};
    let mut max_position: f64 = 0.;
    let mut max_angular: f64 = 0.;
    for plane in [0., 0.25, std::f64::consts::FRAC_PI_2] {
        let mut section = Section::straight(1, "assembly regression", 160.);
        section.kind = SectionKind::Arc {
            radius_m: 45.,
            plane_angle_rad: plane,
        };
        section.roll_rate = Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![Transition {
                id: 1,
                length: 160.,
                start: 0.,
                end: 0.004,
                amplitude: 0.,
                function: Function::Linear {},
            }],
        };
        let t = generate(
            &InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::X, 0.25).unwrap(),
                ..Default::default()
            },
            &[section],
            GenerationOptions::default(),
        )
        .unwrap();
        for station in [20., 40., 60., 100.] {
            let mut train = conventional(3);
            // Joint ordering and endpoint naming do not choose the constrained body.
            if station == 60. {
                train.connections.reverse();
                for j in &mut train.connections {
                    std::mem::swap(&mut j.component_a, &mut j.component_b);
                    std::mem::swap(&mut j.anchor_a_m, &mut j.anchor_b_m);
                }
            }
            let (world, _, attitudes, inspection) =
                initialize(&train, &t, station, Vec3::ZERO, 32, 4)
                    .unwrap_or_else(|e| panic!("plane={plane} station={station}: {e}"));
            for j in &inspection.joints {
                max_position = max_position.max(j.error_m);
            }
            for j in &world.angular_joints {
                max_angular = max_angular.max(j.angle_rad(&world.bodies).unwrap().abs());
            }
            assert_eq!(attitudes.len(), 6);
            assert_eq!(world.angular_joints.len(), 3);
            assert_eq!(inspection, inspect(&train, &t, station).unwrap());
        }
    }
    println!(
        "12 twisting assembly sweeps: max joint residual={max_position:.9e} m; angular residual={max_angular:.9e} rad (limits unchanged at 1e-5)"
    );
    assert!(max_position <= 1e-5 && max_angular <= 1e-5);
}
