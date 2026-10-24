use qx_geometry::{
    authoring::{BoundaryPolicy, Domain, InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::{GenerationOptions, generate},
    transition::{Channel, Function, Transition, Unit},
};
fn options(step: f64) -> GenerationOptions {
    GenerationOptions {
        max_step_m: step,
        max_rotation_per_step_rad: 0.5,
        ..Default::default()
    }
}
fn force(length: f64, normal: f64, lateral: f64) -> Section {
    let mut s = Section::straight(1, "Force reference", length);
    s.kind = SectionKind::Force {
        normal_g: Channel::constant(Unit::G, length, normal),
        lateral_g: Channel::constant(Unit::G, length, lateral),
    };
    s
}
#[test]
fn one_g_level_and_banked_force_balance() {
    let initial = InitialConditions::default();
    let s = force(50.0, 1.0, 0.0);
    let track = generate(&initial, &[s], options(0.3)).unwrap();
    assert!(
        (track.samples.last().unwrap().position_m - Vec3::new(50.0, 0.0, 0.0)).length() < 1e-11
    );
    assert!(
        track
            .graphs
            .iter()
            .all(|g| (g.normal_g - 1.0).abs() < 1e-12 && g.lateral_g.abs() < 1e-12)
    );
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, std::f64::consts::FRAC_PI_2).unwrap(),
        ..initial
    };
    let track = generate(&initial, &[force(20.0, 0.0, -1.0)], options(0.2)).unwrap();
    assert!(
        (track.samples.last().unwrap().position_m - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-10
    );
}
#[test]
fn force_turn_matches_independent_horizontal_circle() {
    let initial = InitialConditions::default();
    let radius = 40.0;
    let length = 30.0;
    let lateral = initial.reference_speed_mps.powi(2) / (initial.gravity_mps2 * radius);
    let track = generate(&initial, &[force(length, 1.0, lateral)], options(0.1)).unwrap();
    let angle = length / radius;
    let expected = Vec3::new(radius * angle.sin(), 0.0, radius * (1.0 - angle.cos()));
    assert!((track.samples.last().unwrap().position_m - expected).length() < 1e-9);
    for g in &track.graphs {
        assert!((g.curvature_per_m - 1.0 / radius).abs() < 1e-8);
        assert!((g.lateral_g - lateral).abs() < 1e-6);
        assert!((g.normal_g - 1.0).abs() < 1e-10);
    }
    let s = 12.345;
    let query = track.sample_at_distance(s).unwrap();
    let exact = Vec3::new(
        radius * (s / radius).sin(),
        0.0,
        radius * (1.0 - (s / radius).cos()),
    );
    assert!((query.position_m - exact).length() < 1e-9);
    assert!(track.sample_at_distance(-1.0).is_err());
    assert!(track.sample_at_distance(f64::NAN).is_err());
}
#[test]
fn fourth_order_convergence_against_analytic_vertical_arc() {
    let mut s = Section::straight(1, "Circle", 10.0);
    s.kind = SectionKind::Arc {
        radius_m: 10.0,
        plane_angle_rad: 0.0,
    };
    let exact = Vec3::new(10.0 * 1.0_f64.sin(), 10.0 * (1.0 - 1.0_f64.cos()), 0.0);
    let errors: [f64; 3] = [2.0, 1.0, 0.5].map(|step| {
        let t = generate(&InitialConditions::default(), &[s.clone()], options(step)).unwrap();
        (t.samples.last().unwrap().position_m - exact).length()
    });
    assert!(errors[0] / errors[1] > 12.0, "{errors:?}");
    assert!(errors[1] / errors[2] > 12.0, "{errors:?}");
    assert!(errors[2] < 1e-7, "{errors:?}");
}
#[test]
fn world_and_rider_local_controls_have_distinct_frames() {
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, std::f64::consts::FRAC_PI_2).unwrap(),
        ..Default::default()
    };
    let mut s = Section::straight(1, "Pitch", 10.0);
    s.kind = SectionKind::Geometric {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10.0, 0.02),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10.0, 0.0),
    };
    let world = generate(&initial, &[s.clone()], options(0.1)).unwrap();
    s.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10.0, 0.02),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10.0, 0.0),
    };
    let local = generate(&initial, &[s], options(0.1)).unwrap();
    let wp = world.samples.last().unwrap().position_m;
    let lp = local.samples.last().unwrap().position_m;
    assert!((wp.y - 50.0 * (1.0 - 0.2_f64.cos())).abs() < 1e-9 && wp.z.abs() < 1e-10);
    assert!((lp.z - 50.0 * (1.0 - 0.2_f64.cos())).abs() < 1e-9 && lp.y.abs() < 1e-10);
}
#[test]
fn time_distance_domains_and_roll_are_consistent() {
    let initial = InitialConditions::default();
    let mut d = Section::straight(1, "Distance", 40.0);
    d.roll_rate = Channel::constant(Unit::RadiansPerMetre, 40.0, 0.02);
    let mut t = Section::straight(1, "Time", 2.0);
    t.domain = Domain::Time;
    t.roll_rate = Channel::constant(Unit::RadiansPerSecond, 2.0, 0.4);
    let a = generate(&initial, &[d], options(0.2)).unwrap();
    let b = generate(&initial, &[t], options(0.2)).unwrap();
    assert!((a.length_m() - 40.0).abs() < 1e-11 && (b.length_m() - 40.0).abs() < 1e-11);
    let fa = a.samples.last().unwrap().frame;
    let fb = b.samples.last().unwrap().frame;
    assert!((fa.normal() - fb.normal()).length() < 1e-12);
    assert!((fa.bank_rad().unwrap() - 0.8).abs() < 1e-11);
    assert!(
        a.graphs
            .iter()
            .all(|g| (g.roll_rate_rad_per_m - 0.02).abs() < 1e-11)
    );
}
#[test]
fn joins_share_pose_and_abrupt_transitions_use_one_sided_limits() {
    let mut a = Section::straight(1, "Roll jump", 10.0);
    a.roll_rate = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![
            Transition::constant(1, 5.0, 0.0),
            Transition::constant(2, 5.0, 0.02),
        ],
    };
    let mut b = Section::straight(2, "Next", 10.0);
    let track = generate(
        &InitialConditions::default(),
        &[a.clone(), b.clone()],
        options(0.7),
    )
    .unwrap();
    assert_eq!(track.sections[0].end_sample, track.sections[1].start_sample);
    assert!(
        track
            .samples
            .windows(2)
            .all(|p| p[0].distance_m < p[1].distance_m)
    );
    assert!((track.samples.last().unwrap().frame.bank_rad().unwrap() - 0.1).abs() < 1e-10);
    assert!(
        track
            .diagnostics
            .iter()
            .any(|d| d.code == "channel_discontinuity")
    );
    b.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
    assert!(generate(&InitialConditions::default(), &[a, b], options(0.7)).is_err());
}
#[test]
fn smooth_force_channels_are_deterministic_and_converge() {
    let mut s = force(30.0, 1.0, 0.0);
    if let SectionKind::Force { normal_g, .. } = &mut s.kind {
        normal_g.transitions[0] = Transition {
            id: 1,
            length: 30.0,
            start: 1.0,
            end: 2.0,
            amplitude: 0.3,
            function: Function::QuinticSmoothstep {},
        };
    }
    let initial = InitialConditions::default();
    let reference = generate(&initial, &[s.clone()], options(0.025)).unwrap();
    let fine = generate(&initial, &[s.clone()], options(0.25)).unwrap();
    let coarse = generate(&initial, &[s.clone()], options(0.5)).unwrap();
    assert_eq!(coarse, generate(&initial, &[s], options(0.5)).unwrap());
    let exact = reference.samples.last().unwrap().position_m;
    let ef = (fine.samples.last().unwrap().position_m - exact).length();
    let ec = (coarse.samples.last().unwrap().position_m - exact).length();
    assert!(ec > ef * 10.0 && ef < 1e-7, "coarse={ec}, fine={ef}");
    assert!(
        fine.graphs
            .iter()
            .all(|g| (g.normal_g - g.authored_normal_g.unwrap()).abs() < 1e-4)
    );
}
#[test]
fn invalid_and_unresolvable_inputs_return_errors() {
    let initial = InitialConditions::default();
    let s = Section::straight(1, "Valid", 5.0);
    assert!(generate(&initial, &[], options(0.1)).is_err());
    assert!(generate(&initial, &[s.clone(), s.clone()], options(0.1)).is_err());
    let mut bad = s.clone();
    bad.extent = f64::NAN;
    assert!(generate(&initial, &[bad], options(0.1)).is_err());
    let mut bad = s.clone();
    bad.roll_rate.unit = Unit::G;
    assert!(generate(&initial, &[bad], options(0.1)).is_err());
    let mut bad = s;
    bad.kind = SectionKind::Arc {
        radius_m: 0.001,
        plane_angle_rad: 0.0,
    };
    assert!(generate(&initial, &[bad], options(1.0)).is_err());
    let small = GenerationOptions {
        max_samples: 10,
        ..Default::default()
    };
    assert!(generate(&initial, &[Section::straight(1, "Budget", 5.0)], small).is_err());
    let vertical = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap(),
        ..initial
    };
    let mut g = Section::straight(1, "World", 1.0);
    g.kind = SectionKind::Geometric {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 1.0, 0.0),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 1.0, 0.0),
    };
    assert!(generate(&vertical, &[g], options(0.1)).is_err());
}

#[test]
fn roundoff_equivalent_channel_knots_do_not_create_tiny_spans() {
    let mut s = force(0.6, 1.0, 0.0);
    s.roll_rate = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![
            Transition::constant(1, 0.1, 0.1),
            Transition::constant(2, 0.2, 0.1),
            Transition::constant(3, 0.3, 0.1),
        ],
    };
    if let SectionKind::Force { normal_g, .. } = &mut s.kind {
        normal_g.transitions = vec![
            Transition::constant(1, 0.3, 1.0),
            Transition::constant(2, 0.3, 1.0),
        ];
    }
    let track = generate(&InitialConditions::default(), &[s], options(0.1)).unwrap();
    assert!(track.samples.len() < 20);
    assert!((track.length_m() - 0.6).abs() < 1e-14);
}

#[test]
fn rider_frame_survives_full_vertical_loop_and_vertical_straight() {
    let radius = 10.0;
    let length = std::f64::consts::TAU * radius;
    let mut s = Section::straight(1, "Loop", length);
    s.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, length, 1.0 / radius),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, length, 0.0),
    };
    let track = generate(&InitialConditions::default(), &[s], options(0.1)).unwrap();
    let end = track.samples.last().unwrap();
    assert!(end.position_m.length() < 1e-8);
    assert!((end.frame.tangent() - Vec3::X).length() < 1e-9);
    for sample in &track.samples {
        let f = sample.frame;
        assert!((f.tangent().length() - 1.0).abs() < 1e-12);
        assert!(f.tangent().dot(f.normal()).abs() < 1e-12);
        assert!((f.tangent().cross(f.normal()) - f.lateral()).length() < 1e-12);
    }
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap(),
        ..Default::default()
    };
    let t = generate(
        &initial,
        &[Section::straight(1, "Vertical", 2.0)],
        options(0.1),
    )
    .unwrap();
    assert!(
        t.graphs
            .iter()
            .all(|g| g.bank_rad.is_none() && g.yaw_rate_rad_per_m.is_none())
    );
    assert!(
        t.diagnostics
            .iter()
            .any(|d| d.code == "vertical_bank_undefined")
    );
}
