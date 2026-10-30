use qx_geometry::{
    profile::{Continuity, NurbsProfile},
    transition::{Function, Side, Transition},
};

#[test]
fn direct_profile_split_at_repeated_knots_preserves_intentional_c0_and_c1() {
    for (rule, control) in [(Continuity::C0, 4), (Continuity::C1, 5)] {
        let mut p = NurbsProfile::cubic(0., 0.03);
        for _ in 0..3 {
            p.insert_knot(0.5).unwrap();
        }
        p.interior = rule;
        p.values[control] += 0.002;
        p.validate().unwrap();
        let (a, b) = p.split(0.5).unwrap();
        assert_eq!(b.incoming, rule);
        for i in 0..=2000 {
            let u = i as f64 / 2000.;
            let side = if u == 0.5 { Side::Left } else { Side::Right };
            let expected = p.evaluate(u, 10., side).unwrap();
            let actual = if u <= 0.5 {
                a.evaluate(u * 2., 5., side)
            } else {
                b.evaluate((u - 0.5) * 2., 5., side)
            }
            .unwrap();
            assert!((expected.value - actual.value).abs() < 1e-14);
            assert!((expected.derivative - actual.derivative).abs() < 1e-13);
            assert!((expected.second_derivative - actual.second_derivative).abs() < 1e-12);
        }
    }
}

fn legacy(function: Function, amplitude: f64) -> Transition {
    Transition {
        id: 7,
        length: 10.,
        start: 0.012,
        end: -0.025,
        amplitude,
        function,
    }
}
fn compare(
    a: &Transition,
    left: &Transition,
    right: &Transition,
    value_tol: f64,
    derivative_tol: f64,
) {
    for i in 0..=2000 {
        let x = a.length * i as f64 / 2000.;
        let e = a.evaluate(x).unwrap();
        let f = if x <= left.length {
            left.evaluate(x).unwrap()
        } else {
            right.evaluate((x - left.length).min(right.length)).unwrap()
        };
        assert!(
            (e.value - f.value).abs() < value_tol,
            "value {x}: {e:?} vs {f:?}"
        );
        assert!(
            (e.derivative - f.derivative).abs() < derivative_tol,
            "slope {x}: {e:?} vs {f:?}"
        );
        assert!(
            (e.second_derivative - f.second_derivative).abs() < derivative_tol,
            "second {x}: {e:?} vs {f:?}"
        );
    }
}
#[test]
fn legacy_polynomial_conversion_and_subdivision_are_exact_including_amplitude() {
    for function in [
        Function::Constant {},
        Function::Linear {},
        Function::Quadratic {},
        Function::CubicSmoothstep {},
        Function::QuinticSmoothstep {},
        Function::Hermite {
            start_slope: 0.01,
            end_slope: -0.008,
        },
    ] {
        for amplitude in [0., 0.04] {
            let mut t = legacy(function.clone(), amplitude);
            if matches!(function, Function::Constant {}) {
                t.end = t.start;
                t.amplitude = 0.;
            }
            let n = t.to_nurbs().unwrap();
            for i in 0..=1000 {
                let x = t.length * i as f64 / 1000.;
                let a = t.evaluate(x).unwrap();
                let b = n.evaluate(x).unwrap();
                assert!((a.value - b.value).abs() < 2e-15);
                assert!((a.derivative - b.derivative).abs() < 2e-15);
                assert!((a.second_derivative - b.second_derivative).abs() < 2e-14);
            }
            for fraction in [0.001, 0.3577, 0.5, 0.999] {
                let (a, b) = t.split(fraction, 8).unwrap();
                compare(&t, &a, &b, 2e-15, 2e-10);
                let x = a.evaluate(a.length).unwrap();
                let y = b.evaluate(0.).unwrap();
                assert!((x.value - y.value).abs() < 1e-15);
                assert!((x.derivative - y.derivative).abs() < 1e-13);
                assert!((x.second_derivative - y.second_derivative).abs() < 2e-10);
            }
        }
    }
}
#[test]
fn rational_nonuniform_multispan_profile_splits_with_real_knot_insertion() {
    let p = NurbsProfile {
        degree: 3,
        values: vec![0., 0.03, -0.01, 0.04, 0.01, -0.02, 0.],
        weights: vec![1., 0.7, 2., 1.3, 0.4, 1.1, 1.],
        knots: vec![0., 0., 0., 0., 0.2, 0.5, 0.83, 1., 1., 1., 1.],
        incoming: Continuity::Free,
        interior: Continuity::C2,
    };
    let t = Transition::from_profile(1, 10., p);
    let original_channel = qx_geometry::transition::Channel {
        unit: qx_geometry::transition::Unit::RadiansPerMetre,
        transitions: vec![t.clone()],
    };
    let original_breaks = original_channel.integration_knots().unwrap();
    for f in [0.001, 0.2, 0.3577, 0.5, 0.999] {
        let (a, b) = t.split(f, 2).unwrap();
        compare(&t, &a, &b, 1e-14, 1e-9);
        let channel = qx_geometry::transition::Channel {
            unit: original_channel.unit,
            transitions: vec![a, b],
        };
        let breaks = channel.integration_knots().unwrap();
        assert_eq!(
            breaks.len(),
            original_breaks.len(),
            "{f}: {breaks:?} vs {original_breaks:?}"
        );
        for (a, b) in breaks.iter().zip(&original_breaks) {
            assert!((a - b).abs() < 1e-13);
        }
    }
    let mut regions = vec![t.clone()];
    for i in 0..12 {
        let index = i % regions.len();
        let (a, b) = regions[index].split(0.3577, (i + 2) as u64).unwrap();
        regions.splice(index..=index, [a, b]);
    }
    for i in 0..=1000 {
        let x = t.length * i as f64 / 1000.;
        let mut cursor = 0.;
        let mut got = None;
        for (j, r) in regions.iter().enumerate() {
            if x <= cursor + r.length || j + 1 == regions.len() {
                got = Some(r.evaluate((x - cursor).clamp(0., r.length)).unwrap());
                break;
            }
            cursor += r.length;
        }
        assert!((got.unwrap().value - t.evaluate(x).unwrap().value).abs() < 2e-14);
    }
}
#[test]
fn genuine_short_c2_profile_features_remain_in_the_integration_grid() {
    use qx_geometry::{
        authoring::{InitialConditions, Section, SectionKind},
        solver::{GenerationOptions, generate},
        transition::{Channel, Unit},
    };
    let mut p = NurbsProfile {
        degree: 3,
        values: vec![0.; 11],
        knots: vec![
            0., 0., 0., 0., 0.4997, 0.4998, 0.4999, 0.5, 0.5001, 0.5002, 0.5003, 1., 1., 1., 1.,
        ],
        weights: vec![1.; 11],
        incoming: Continuity::C2,
        interior: Continuity::C2,
    };
    p.values[5] = 0.01;
    let mut s = Section::straight(1, "Narrow C2 test pulse", 10.);
    let pitch = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition::from_profile(1, 10., p)],
    };
    assert!(pitch.integration_knots().unwrap().len() > 4);
    s.kind = SectionKind::RiderLocal {
        pitch_rate: pitch,
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    let coarse = generate(
        &InitialConditions::default(),
        &[s.clone()],
        GenerationOptions::default(),
    )
    .unwrap();
    let fine = generate(
        &InitialConditions::default(),
        &[s],
        GenerationOptions {
            max_step_m: 0.01,
            ..Default::default()
        },
    )
    .unwrap();
    let a = coarse.samples.last().unwrap();
    let b = fine.samples.last().unwrap();
    assert!(
        a.frame.tangent().y > 1e-6,
        "short authored pulse was missed"
    );
    assert!((a.frame.tangent() - b.frame.tangent()).length() < 1e-10);
    assert!((a.position_m - b.position_m).length() < 1e-9);
}
#[test]
fn cosine_splits_are_analytic_and_never_approximated_as_cubic() {
    let t = legacy(Function::Cosine {}, 0.04);
    assert!(t.to_nurbs().is_err());
    let (a, b) = t.split(0.3577, 8).unwrap();
    compare(&t, &a, &b, 2e-15, 2e-13);
    let (c, d) = b.split(0.23, 9).unwrap();
    compare(&b, &c, &d, 2e-15, 2e-13);
}
#[test]
fn analytic_rational_derivatives_match_finite_differences_and_endpoint_jet_edits() {
    let mut p = NurbsProfile::cubic(0.01, 0.03);
    p.weights = vec![1., 0.7, 1.4, 1.];
    for u in [0.13, 0.39, 0.71] {
        let h = 1e-5;
        let a = p.evaluate(u - h, 1., Side::Right).unwrap();
        let b = p.evaluate(u + h, 1., Side::Right).unwrap();
        let m = p.evaluate(u, 1., Side::Right).unwrap();
        assert!(((b.value - a.value) / (2. * h) - m.derivative).abs() < 1e-9);
        assert!(((b.derivative - a.derivative) / (2. * h) - m.second_derivative).abs() < 1e-8);
    }
    let original_end = p.evaluate(1., 10., Side::Left).unwrap();
    let mut target = p.evaluate(0., 10., Side::Right).unwrap();
    target.value = 0.04;
    target.derivative = 0.002;
    target.second_derivative = -0.003;
    p.set_jet(false, 10., target, 2).unwrap();
    let a = p.evaluate(0., 10., Side::Right).unwrap();
    let b = p.evaluate(1., 10., Side::Left).unwrap();
    assert!(
        (a.value - target.value).abs() < 1e-14
            && (a.derivative - target.derivative).abs() < 1e-14
            && (a.second_derivative - target.second_derivative).abs() < 1e-14
    );
    assert!(
        (b.value - original_end.value).abs() < 1e-14
            && (b.derivative - original_end.derivative).abs() < 1e-14
            && (b.second_derivative - original_end.second_derivative).abs() < 1e-14
    );
}
#[test]
fn malformed_profiles_and_endpoint_splits_fail_transactionally() {
    let p = NurbsProfile::cubic(0., 1.);
    for invalid in 0..7 {
        let mut q = p.clone();
        match invalid {
            0 => q.weights[0] = 0.,
            1 => q.weights[1] = f64::NAN,
            2 => q.knots[4] = -0.1,
            3 => q.knots.pop().map(|_| ()).unwrap(),
            4 => q.values[0] = f64::INFINITY,
            5 => q.degree = 8,
            _ => q.knots[0] = 0.1,
        }
        assert!(q.validate().is_err());
    }
    let t = Transition::from_profile(1, 10., p);
    for f in [0., 1., f64::NAN, 1e-12] {
        assert!(t.split(f, 2).is_err());
    }
}
#[test]
fn conversion_retains_the_full_legacy_value_slope_and_amplitude_limits() {
    let mut t = legacy(
        Function::Hermite {
            start_slope: 1e12,
            end_slope: -1e12,
        },
        1e12,
    );
    t.length = 1e6;
    t.validate().unwrap();
    let n = t.to_nurbs().unwrap();
    n.validate().unwrap();
    for x in [0., 1e5, 5e5, 9e5, 1e6] {
        let a = t.evaluate(x).unwrap();
        let b = n.evaluate(x).unwrap();
        assert!(qx_geometry::approximately_equal(
            a.value, b.value, 1e-6, 1e-13
        ));
    }
    t.function = Function::Cosine {};
    let (a, b) = t.split(0.3577, 8).unwrap();
    a.validate().unwrap();
    b.validate().unwrap();
}
