use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    heartline::*,
    math::{Frame, Quaternion, Vec3},
    solver::{GenerationOptions, generate},
    transition::{Channel, Unit},
};
fn frame() -> Frame {
    Frame::new(Quaternion::IDENTITY).unwrap()
}
fn circle(step: f64, offset: Vec3) -> ReferenceTrack {
    let mut section = Section::straight(1, "analytic circle", 20. * std::f64::consts::TAU);
    section.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: 0.,
    };
    let track = generate(
        &InitialConditions::default(),
        &[section],
        GenerationOptions {
            max_step_m: step,
            ..Default::default()
        },
    )
    .unwrap();
    generate_reference(
        &track,
        &OffsetProfile::Constant { offset_m: offset },
        AuthoredReference::TrackCenterline,
        9.80665,
    )
    .unwrap()
}
#[test]
fn zero_and_straight_offsets_preserve_exact_positions_and_lengths() {
    let track = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "line", 30.)],
        GenerationOptions::default(),
    )
    .unwrap();
    for offset in [Vec3::ZERO, Vec3::new(2., -1.2, 0.4)] {
        let reference = generate_reference(
            &track,
            &OffsetProfile::Constant { offset_m: offset },
            AuthoredReference::TrackCenterline,
            9.80665,
        )
        .unwrap();
        assert!((reference.length_m - track.length_m()).abs() < 1e-12);
        for (p, source) in reference.points.iter().zip(&track.samples) {
            assert_eq!(p.position_m, source.position_m + offset);
            assert!((p.tangent - Vec3::X).length() < 1e-12);
            assert!(p.curvature_per_m < 1e-12);
            assert!((p.normal_g - 1.).abs() < 1e-10);
        }
    }
}
#[test]
fn analytic_inward_offset_circle_has_distinct_curvature_length_and_speed() {
    let result = circle(0.1, Vec3::new(0., 2., 0.));
    assert!((result.length_m - 18. * std::f64::consts::TAU).abs() < 1e-6);
    for p in &result.points {
        assert!((p.curvature_per_m - 1. / 18.).abs() < 2e-7);
        assert!((p.speed_mps - 18.).abs() < 1e-7);
        // Circle center (0,20,0); inward normal offset reduces radius to 18.
        assert!(((p.position_m - Vec3::new(0., 20., 0.)).length() - 18.).abs() < 1e-8);
        let radial = (Vec3::new(0., 20., 0.) - p.position_m) / 18.;
        assert!((p.acceleration_mps2 - radial * 18.).length() < 1e-4);
    }
}
#[test]
fn rigid_motion_includes_alpha_centripetal_coriolis_and_relative_acceleration() {
    let motion = point_motion(
        PointMotion {
            velocity_mps: Vec3::X * 5.,
            acceleration_mps2: Vec3::Y * 2.,
        },
        frame(),
        Vec3::Z * 3.,
        Vec3::Z * 4.,
        Vec3::X * 2.,
        Vec3::Y * 7.,
        Vec3::X * 11.,
    )
    .unwrap();
    assert_eq!(motion.velocity_mps, Vec3::new(5., 13., 0.));
    assert_eq!(motion.acceleration_mps2, Vec3::new(-49., 10., 0.));
}
#[test]
fn spatial_and_time_kinematics_agree_for_variable_offsets() {
    let orientation = OrientationJet {
        frame: frame(),
        omega_per_m: Vec3::new(0.3, -0.2, 0.1),
        omega_derivative_per_m2: Vec3::new(-0.1, 0.04, 0.08),
    };
    let path = PathJet {
        position_m: Vec3::ZERO,
        first: Vec3::X,
        second_per_m: orientation.omega_per_m.cross(Vec3::X),
    };
    let offset = OffsetProfile::Quintic {
        start_m: Vec3::new(1., 2., -1.),
        end_m: Vec3::new(2., 3., 2.),
    }
    .evaluate(7., 30.)
    .unwrap();
    let jet = offset_path(path, orientation, offset).unwrap();
    let speed = 12.;
    let motion = point_motion(
        PointMotion {
            velocity_mps: path.first * speed,
            acceleration_mps2: path.second_per_m * speed * speed,
        },
        frame(),
        orientation.omega_per_m * speed,
        orientation.omega_derivative_per_m2 * speed * speed,
        offset.value_m,
        offset.first * speed,
        offset.second_per_m * speed * speed,
    )
    .unwrap();
    assert!((motion.velocity_mps - jet.first * speed).length() < 1e-12);
    assert!((motion.acceleration_mps2 - jet.second_per_m * speed * speed).length() < 1e-12);
    // Independent finite differences of the quintic local offset.
    let profile = OffsetProfile::Quintic {
        start_m: Vec3::ZERO,
        end_m: Vec3::new(1., 2., 3.),
    };
    let a = profile.evaluate(7. - 0.0001, 30.).unwrap();
    let b = profile.evaluate(7. + 0.0001, 30.).unwrap();
    let m = profile.evaluate(7., 30.).unwrap();
    assert!(((b.value_m - a.value_m) / 0.0002 - m.first).length() < 1e-9);
    assert!(((b.first - a.first) / 0.0002 - m.second_per_m).length() < 1e-10);
}
#[test]
fn rolling_straight_forms_analytic_helix_and_converges() {
    let solve = |step| {
        let mut section = Section::straight(1, "roll", 12.);
        section.roll_rate = Channel::constant(Unit::RadiansPerMetre, 12., 0.4);
        let track = generate(
            &InitialConditions::default(),
            &[section],
            GenerationOptions {
                max_step_m: step,
                ..Default::default()
            },
        )
        .unwrap();
        generate_reference(
            &track,
            &OffsetProfile::Constant {
                offset_m: Vec3::Y * 2.,
            },
            AuthoredReference::TrackCenterline,
            9.80665,
        )
        .unwrap()
    };
    let error = |r: &ReferenceTrack| {
        r.points
            .iter()
            .map(|p| {
                (p.curvature_per_m - (2. * 0.4_f64.powi(2) / (1. + 4. * 0.4_f64.powi(2)))).abs()
            })
            .fold(0., f64::max)
    };
    let coarse = solve(0.3);
    let fine = solve(0.15);
    assert!(error(&fine) < error(&coarse) / 5.);
    assert!(error(&fine) < 3e-5);
    let expected_length = 12. * (1. + 4. * 0.4_f64.powi(2)).sqrt();
    assert!((fine.length_m - expected_length).abs() < 1e-4);
    for p in &fine.points {
        assert!(
            (p.position_m
                - Vec3::new(
                    p.source_distance_m,
                    2. * (0.4 * p.source_distance_m).cos(),
                    2. * (0.4 * p.source_distance_m).sin()
                ))
            .length()
                < 1e-6
        );
    }
}
#[test]
fn loops_and_spatial_inversions_keep_frames_and_boundary_positions() {
    let mut a = Section::straight(1, "spatial inversion", 40.);
    a.kind = SectionKind::Arc {
        radius_m: 8.,
        plane_angle_rad: 0.,
    };
    a.roll_rate = Channel::constant(Unit::RadiansPerMetre, 40., 0.15);
    let mut b = a.clone();
    b.id.0 = 2;
    let track = generate(
        &InitialConditions::default(),
        &[a, b],
        GenerationOptions::default(),
    )
    .unwrap();
    let r = generate_reference(
        &track,
        &OffsetProfile::Constant { offset_m: Vec3::Y },
        AuthoredReference::TrackCenterline,
        9.80665,
    )
    .unwrap();
    let join: Vec<_> = r
        .points
        .iter()
        .filter(|p| (p.source_distance_m - 40.).abs() < 1e-8)
        .collect();
    assert_eq!(join.len(), 2);
    assert_eq!(join[0].position_m, join[1].position_m);
    assert!((join[0].acceleration_mps2 - join[1].acceleration_mps2).length() < 0.002);
    assert!(
        r.points
            .iter()
            .all(|p| p.frame.normal().length().is_finite() && p.acceleration_mps2.is_finite())
    );
}
#[test]
fn singular_and_invalid_offsets_reject_with_context() {
    assert!(
        OffsetProfile::Constant {
            offset_m: Vec3::new(f64::NAN, 0., 0.)
        }
        .validate()
        .is_err()
    );
    assert!(
        OffsetProfile::Constant {
            offset_m: Vec3::Y * 101.
        }
        .validate()
        .is_err()
    );
    let path = PathJet {
        position_m: Vec3::ZERO,
        first: Vec3::X,
        second_per_m: Vec3::Y / 20.,
    };
    let orientation = OrientationJet {
        frame: frame(),
        omega_per_m: Vec3::Z / 20.,
        omega_derivative_per_m2: Vec3::ZERO,
    };
    assert!(
        offset_path(
            path,
            orientation,
            OffsetJet {
                value_m: Vec3::Y * 20.,
                first: Vec3::ZERO,
                second_per_m: Vec3::ZERO
            }
        )
        .unwrap_err()
        .to_string()
        .contains("singular")
    );
}
#[test]
fn inverse_requires_explicit_compatible_orientation_and_never_invents_a_solution() {
    let orientation = OrientationJet {
        frame: frame(),
        omega_per_m: Vec3::ZERO,
        omega_derivative_per_m2: Vec3::ZERO,
    };
    let rider = PathJet {
        position_m: Vec3::Y * 2.,
        first: Vec3::X,
        second_per_m: Vec3::ZERO,
    };
    let offset = OffsetJet {
        value_m: Vec3::Y * 2.,
        first: Vec3::ZERO,
        second_per_m: Vec3::ZERO,
    };
    assert_eq!(
        track_from_rider(rider, orientation, offset)
            .unwrap()
            .position_m,
        Vec3::ZERO
    );
    let rolling = OrientationJet {
        omega_per_m: Vec3::X,
        ..orientation
    };
    assert!(
        track_from_rider(rider, rolling, offset)
            .unwrap_err()
            .to_string()
            .contains("orientation")
    );
}
#[test]
fn constrained_inverse_circle_recovers_a_distinct_track_radius() {
    let mut section = Section::straight(1, "Authored rider circle", 20. * std::f64::consts::TAU);
    section.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: 0.,
    };
    let source = generate(
        &InitialConditions::default(),
        &[section],
        GenerationOptions::default(),
    )
    .unwrap();
    let track = generate_reference(
        &source,
        &OffsetProfile::Constant {
            offset_m: Vec3::Y * 2.,
        },
        AuthoredReference::RiderReference,
        9.80665,
    )
    .unwrap();
    assert!((track.length_m - 22. * std::f64::consts::TAU).abs() < 1e-6);
    for point in track.points {
        assert!((point.curvature_per_m - 1. / 22.).abs() < 2e-7);
    }
}
#[test]
fn abrupt_roll_rate_keeps_both_reference_side_limits_without_smoothing() {
    let a = Section::straight(1, "unrolled", 10.);
    let mut b = Section::straight(2, "rolling", 10.);
    b.roll_rate = Channel::constant(Unit::RadiansPerMetre, 10., 0.2);
    let source = generate(
        &InitialConditions::default(),
        &[a, b],
        GenerationOptions::default(),
    )
    .unwrap();
    let reference = generate_reference(
        &source,
        &OffsetProfile::Constant { offset_m: Vec3::Y },
        AuthoredReference::TrackCenterline,
        9.80665,
    )
    .unwrap();
    let boundary: Vec<_> = reference
        .points
        .iter()
        .filter(|p| (p.source_distance_m - 10.).abs() < 1e-9)
        .collect();
    assert_eq!(boundary.len(), 2);
    assert_eq!(boundary[0].position_m, boundary[1].position_m);
    assert!((boundary[0].tangent - boundary[1].tangent).length() > 0.1);
    assert!(!reference.warnings.is_empty());
}
