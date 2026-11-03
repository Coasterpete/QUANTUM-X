use qx_geometry::{
    profile::{Continuity, NurbsProfile},
    profile_plot::{bounds, sample},
    transition::{Function, Transition},
};

fn narrow(center: f64, step: f64) -> Transition {
    let mut knots = vec![0.; 4];
    knots.extend((-3..=3).map(|i| center + i as f64 * step));
    knots.extend([1.; 4]);
    let mut values = vec![0.; 11];
    values[5] = 1.;
    Transition::from_profile(
        1,
        1.,
        NurbsProfile {
            degree: 3,
            knots,
            values,
            weights: vec![1.; 11],
            incoming: Continuity::Free,
            interior: Continuity::C2,
        },
    )
}

fn verify(t: &Transition, range: [f64; 2], tolerance: f64, budget: usize) {
    let plot = sample(t, range, tolerance, budget).unwrap();
    assert!(plot.segments.len() <= budget);
    assert_eq!(plot.segments.first().unwrap().from[0], range[0]);
    assert_eq!(plot.segments.last().unwrap().to[0], range[1]);
    for pair in plot.segments.windows(2) {
        assert_eq!(pair[0].to, pair[1].from);
    }
    for s in &plot.segments {
        assert!(s.from[0] <= s.to[0]);
        for i in 0..=32 {
            let u = i as f64 / 32.;
            let x = s.from[0] + (s.to[0] - s.from[0]) * u;
            let value = t.evaluate(x).unwrap().value;
            let roundoff = 2e-12 * value.abs().max(1.);
            assert!(
                value >= s.bounds[0] - roundoff && value <= s.bounds[1] + roundoff,
                "value {value} outside {:?} at {x}",
                s.bounds
            );
            if s.resolved {
                let chord = s.from[1] + (s.to[1] - s.from[1]) * u;
                assert!(
                    (value - chord).abs() <= tolerance + roundoff,
                    "chord error {} > {tolerance} at {x}",
                    (value - chord).abs()
                );
            }
        }
    }
    let fit = bounds(t).unwrap();
    for s in plot.segments {
        for p in [s.from, s.to] {
            assert!(p[1] >= fit[0] - 2e-12 && p[1] <= fit[1] + 2e-12);
        }
    }
}

#[test]
fn narrow_c2_feature_missed_by_both_old_grids_is_recovered() {
    let t = narrow(0.5003, 0.0001);
    t.validate().unwrap();
    for count in [128, 512] {
        let old = (0..=count)
            .map(|i| t.evaluate(i as f64 / count as f64).unwrap().value)
            .fold(0_f64, f64::max);
        assert_eq!(old, 0.);
    }
    let peak = t.evaluate(0.5003).unwrap().value;
    assert!((peak - 2. / 3.).abs() < 1e-12);
    let plot = sample(&t, [0., 1.], 0.001, 2048).unwrap();
    assert!(!plot.limited);
    let max = plot
        .segments
        .iter()
        .flat_map(|s| [s.from[1], s.to[1]])
        .fold(0_f64, f64::max);
    assert!((max - peak).abs() < 1e-12);
    let fit = bounds(&t).unwrap();
    assert!(fit[0] <= 0. && fit[1] >= peak && fit[1] - peak < 1e-6);
    verify(&t, [0., 1.], 0.001, 2048);
}

#[test]
fn narrow_features_near_both_endpoints_and_at_small_scales() {
    for center in [0.000004, 0.999996] {
        let t = narrow(center, 0.000001);
        verify(&t, [0., 1.], 0.001, 2048);
        assert!(bounds(&t).unwrap()[1] >= t.evaluate(center).unwrap().value);
    }
    let t = narrow(0.5003, 1e-10);
    assert!(
        sample(&t, [0., 1.], 0.001, 2048)
            .unwrap()
            .segments
            .iter()
            .any(|s| s.to[1] > 0.66)
    );
}

#[test]
fn repeated_knots_and_clipped_nonuniform_spans_keep_one_sided_boundaries() {
    let t = Transition::from_profile(
        1,
        1.,
        NurbsProfile {
            degree: 3,
            knots: vec![0., 0., 0., 0., 0.001, 0.001, 0.001, 0.998, 1., 1., 1., 1.],
            values: vec![0., 2., -1., 0.5, 1., -2., 1., 0.],
            weights: vec![1., 2., 0.1, 3., 1., 0.5, 2., 1.],
            incoming: Continuity::Free,
            interior: Continuity::C0,
        },
    );
    verify(&t, [0., 1.], 0.002, 2048);
    verify(&t, [0.00097, 0.0012], 0.0001, 2048);
    let plot = sample(&t, [0., 1.], 0.002, 2048).unwrap();
    assert!(plot.segments.iter().any(|s| s.to[0] == 0.001));
}

#[test]
fn rational_chord_test_uses_parameter_x_not_weighted_control_abscissae() {
    let mut p = NurbsProfile::cubic(0., 1.);
    p.weights = vec![0.001, 1000., 0.01, 5.];
    p.values = vec![0., 1., -2., 0.5];
    let t = Transition::from_profile(1, 1., p);
    verify(&t, [0., 1.], 0.001, 2048);
    verify(&t, [0.000001, 0.02], 0.00001, 4096);
}

#[test]
fn endpoint_midpoint_agreement_is_not_a_flatness_proof() {
    let mut p = NurbsProfile::cubic(0., 0.);
    p.values = vec![0., 1., -1., 0.];
    let t = Transition::from_profile(1, 1., p);
    assert_eq!(t.evaluate(0.5).unwrap().value, 0.);
    let plot = sample(&t, [0., 1.], 0.001, 2048).unwrap();
    assert!(plot.segments.len() > 8);
    let fit = bounds(&t).unwrap();
    assert!(fit[0] < -0.28 && fit[1] > 0.28);
    verify(&t, [0., 1.], 0.001, 2048);
}

#[test]
fn all_legacy_functions_and_degrees_keep_evaluator_results() {
    for function in [
        Function::Linear {},
        Function::Quadratic {},
        Function::CubicSmoothstep {},
        Function::QuinticSmoothstep {},
        Function::Hermite {
            start_slope: 0.7,
            end_slope: -2.,
        },
        Function::Cosine {},
    ] {
        let t = Transition {
            id: 1,
            length: 2.,
            start: 0.4,
            end: -0.7,
            amplitude: 1.3,
            function,
        };
        let original = t.clone();
        let before: Vec<_> = (0..=100)
            .map(|i| t.evaluate(i as f64 / 50.).unwrap())
            .collect();
        verify(&t, [0., 2.], 0.001, 2048);
        assert_eq!(t, original);
        for (i, value) in before.into_iter().enumerate() {
            assert_eq!(value, t.evaluate(i as f64 / 50.).unwrap());
        }
    }
    for degree in [3, 4, 5] {
        let mut p = NurbsProfile::polynomial(&[0.2, -1., 2., -0.5], degree);
        p.weights = (0..=degree).map(|i| 1. + (i % 3) as f64).collect();
        verify(&Transition::from_profile(1, 1., p), [0., 1.], 0.001, 2048);
    }
}

#[test]
fn legacy_cosine_spans_include_interior_extrema_and_split_profiles() {
    let mut t = Transition::constant(1, 2., 0.);
    t.function = Function::Cosine {};
    t.end = 1.;
    t.amplitude = 2.;
    let (a, b) = t.split(0.3577, 2).unwrap();
    for part in [&a, &b] {
        verify(part, [0., part.length], 0.001, 2048);
    }
    let t = Transition {
        id: 1,
        length: 1.,
        start: 1.,
        end: 1.,
        amplitude: 0.,
        function: Function::CosineSpan {
            offset: 0.,
            scale: 1.,
            phase_start: 0.,
            phase_end: 20. * std::f64::consts::PI,
            polynomial: [0.; 5],
        },
    };
    verify(&t, [0., 1.], 0.001, 2048);
    let fit = bounds(&t).unwrap();
    assert!(fit[0] <= -1. && fit[1] >= 1.);
}

#[test]
fn budget_exhaustion_keeps_conservative_enclosures_for_every_interval() {
    let t = narrow(0.5003, 0.0001);
    for budget in [1, 4, 8, 16] {
        let plot = sample(&t, [0., 1.], 1e-10, budget).unwrap();
        assert!(plot.limited);
        assert!(
            plot.segments
                .iter()
                .any(|s| !s.resolved && s.bounds[1] > 0.66)
        );
        verify(&t, [0., 1.], 1e-10, budget);
    }
}

#[test]
fn maximum_control_count_and_extreme_positive_weights_have_bounded_work() {
    let degree = 3;
    let mut knots = vec![0.; 4];
    knots.extend((1..=252).map(|i| (i as f64 / 253.).powi(3)));
    knots.extend([1.; 4]);
    let p = NurbsProfile {
        degree,
        knots,
        values: (0..256).map(|i| (i as f64).sin()).collect(),
        weights: (0..256)
            .map(|i| if i % 2 == 0 { 1e-8 } else { 1e8 })
            .collect(),
        incoming: Continuity::Free,
        interior: Continuity::Free,
    };
    let t = Transition::from_profile(1, 1., p);
    let timer = std::time::Instant::now();
    let plot = sample(&t, [0., 1.], 1e-12, 256).unwrap();
    assert_eq!(plot.segments.len(), 256);
    assert!(plot.limited);
    let fit = bounds(&t).unwrap();
    assert!(fit.iter().all(|v| v.is_finite()));
    println!(
        "256-control adversarial profile: {} segments in {:?}",
        plot.segments.len(),
        timer.elapsed()
    );
}

#[test]
fn zoom_and_pan_clip_without_missing_internal_features() {
    let t = narrow(0.5003, 0.0001);
    for (range, tolerance) in [
        ([0., 1.], 0.01),
        ([0.50005, 0.50055], 0.001),
        ([0.50029, 0.50031], 1e-6),
        ([0.2, 0.3], 0.001),
    ] {
        verify(&t, range, tolerance, 2048);
        assert!(!sample(&t, range, tolerance, 2048).unwrap().limited);
    }
}

#[test]
fn constants_empty_intervals_and_invalid_profiles_are_explicit() {
    for value in [0., 0.1, -5., 1e12] {
        let t = Transition::constant(1, 1., value);
        assert_eq!(bounds(&t).unwrap(), [value; 2]);
        assert_eq!(sample(&t, [0., 1.], 0.001, 8).unwrap().segments.len(), 1);
        assert!(
            sample(&t, [0.5, 0.5], 0.001, 8)
                .unwrap()
                .segments
                .is_empty()
        );
    }
    let mut t = narrow(0.5, 0.0001);
    assert!(sample(&t, [0., 1.], 0., 8).is_err());
    assert!(sample(&t, [0., 1.], 0.1, 0).is_err());
    assert!(sample(&t, [-1., 1.], 0.1, 8).is_err());
    assert!(sample(&t, [0., f64::NAN], 0.1, 8).is_err());
    let Function::Nurbs { profile } = &mut t.function else {
        unreachable!()
    };
    profile.weights[0] = 0.;
    assert!(sample(&t, [0., 1.], 0.1, 8).is_err());
    let t = Transition::from_profile(
        1,
        1.,
        NurbsProfile {
            degree: 3,
            knots: vec![],
            values: vec![],
            weights: vec![],
            incoming: Continuity::Free,
            interior: Continuity::Free,
        },
    );
    assert!(bounds(&t).is_err());
}
