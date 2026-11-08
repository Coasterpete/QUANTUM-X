#[path = "support/articulation.rs"]
mod fixtures;

use fixtures::{arc, straight};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3, rotation_between},
    solver::{CenterlineSample, GeneratedTrack},
};
use qx_physics::assembly_diagnostics::{
    ChordDiagnostics, ChordTermination, diagnose_chord, diagnose_chord_on_layout,
};

fn solve(
    track: &GeneratedTrack,
    center: f64,
    length: f64,
) -> (qx_physics::Result<(f64, f64)>, ChordDiagnostics) {
    let mut d = ChordDiagnostics::default();
    let result = diagnose_chord(track, center, length, &mut d);
    println!("chord center={center} length={length}: {d:?}");
    (result, d)
}

#[test]
fn straight_lengths_and_endpoint_bounds_match_analytic_geometry() {
    let t = straight();
    for length in [0.01, 0.4, 2.4, 7., 20.] {
        for center in [
            length * 0.5 + 1e-8,
            200.,
            t.length_m() - length * 0.5 - 1e-8,
        ] {
            let ((a, b), d) = {
                let (r, d) = solve(&t, center, length);
                (r.unwrap(), d)
            };
            assert!((b - a - length).abs() < 2e-12);
            assert!(((a + b) * 0.5 - center).abs() < 1e-12);
            assert!(d.residual_m.unwrap().abs() < 2e-12);
            assert_eq!(d.termination, ChordTermination::Converged);
            assert!(d.bisection_iterations <= 64 * 21);
            assert!((1..=4096).contains(&d.bracket_probes));
            assert!(independent_residual(&t, (a, b), length, false).abs() < 2e-12);
        }
    }
    // M1 lost this valid endpoint to a 9.1e-15 m negative residual.
    let (r, d) = solve(&t, t.length_m() - 0.005, 0.01);
    let (a, b) = r.unwrap();
    assert_eq!(b, t.length_m());
    assert!((a - 599.99).abs() < 2e-12);
    assert!(independent_residual(&t, (a, b), 0.01, false).abs() < 2e-12);
    assert!(d.max_span_m < 0.01);
    for center in [0., 0.5, t.length_m() - 0.5, t.length_m()] {
        let (r, d) = solve(&t, center, 2.4);
        assert!(r.is_err());
        assert_eq!(d.termination, ChordTermination::NoBracket);
        assert_eq!(d.bisection_iterations, 0);
    }
}

#[test]
fn circular_chords_find_the_local_branch_and_expose_sampling_error() {
    for radius in [2., 8., 30.] {
        let t = arc(radius, 0., 12. * radius, false);
        for ratio in [0.05, 0.6, 1.9] {
            let length = radius * ratio;
            let ((a, b), d) = {
                let (r, d) = solve(&t, 6. * radius, length);
                (r.unwrap(), d)
            };
            let analytic_span = 2. * radius * (length / (2. * radius)).asin();
            let error = (b - a - analytic_span).abs();
            println!(
                "circle R={radius} L={length}: analytic_span_m={analytic_span:.12e} span_error_m={error:.12e}"
            );
            // Generation + Hermite interpolation error, not bisection tolerance.
            assert!(error < 2e-6, "{error:e}");
            assert!(b - a < std::f64::consts::PI * radius);
            assert!(d.residual_m.unwrap().abs() < 1e-12);
        }
    }
}

#[test]
fn near_tangency_is_detected_and_infeasibility_remains_explicit() {
    let radius = 10.;
    let t = arc(radius, 0., 100., false);
    for length in [19.9999, 20., 20.1] {
        let (r, d) = solve(&t, 40., length);
        if length < 20. {
            let stations = r.unwrap();
            let analytic = 2. * radius * (length / (2. * radius)).asin();
            // Close to tangency, interpolation error amplifies station error.
            assert!((stations.1 - stations.0 - analytic).abs() < 2e-7);
            assert!(independent_residual(&t, stations, length, false).abs() < 1e-12);
            let half = std::f64::consts::PI * radius * 0.5;
            let a = t.sample_at_distance(40. - half).unwrap();
            let b = t.sample_at_distance(40. + half).unwrap();
            // Independent positive witness BETWEEN the production probes.
            let witness = (b.position_m - a.position_m).length() - length;
            println!("M1 missed bracket: positive witness residual_m={witness:.12e}");
            assert!(witness > 9e-5);
            assert!(2. * radius * (length / (2. * radius)).asin() < d.max_span_m);
        } else {
            // The generated/Hermite circle is slightly inside the ideal circle
            // at its maximum; exact tangency is tested independently below.
            assert!(r.is_err());
            assert_eq!(d.termination, ChordTermination::NoBracket);
        }
    }
}

#[test]
fn circuit_chord_queries_cross_seams_only_with_explicit_valid_topology() {
    let t = arc(10., 0., std::f64::consts::TAU * 10., false);
    assert!(qx_geometry::topology::diagnose(&t).unwrap().valid);
    for center in [0., 0.2, t.length_m() - 0.2, t.length_m()] {
        let (r, d) = solve(&t, center, 2.4);
        assert!(r.is_err());
        assert_eq!(d.termination, ChordTermination::NoBracket);
        let mut circuit = ChordDiagnostics::default();
        let stations = diagnose_chord_on_layout(
            &t,
            center,
            2.4,
            qx_geometry::topology::LayoutMode::Circuit,
            &mut circuit,
        )
        .unwrap();
        let analytic = 20. * (2.4_f64 / 20.).asin();
        assert!((stations.1 - stations.0 - analytic).abs() < 2e-7);
        assert!(independent_residual(&t, stations, 2.4, true).abs() < 1e-12);
        assert!(circuit.circuit);
        println!("circuit center={center}: {circuit:?}");
        for layout in [
            qx_geometry::topology::LayoutMode::Open,
            qx_geometry::topology::LayoutMode::Shuttle,
        ] {
            assert!(diagnose_chord_on_layout(&t, center, 2.4, layout, &mut circuit).is_err());
            assert!(!circuit.circuit);
        }
    }
    assert!(qx_physics::consist::inspect(&fixtures::conventional(1), &t, 0.).is_err());
    assert!(solve(&t, t.length_m() * 0.5, 2.4).0.is_ok());
    let mut d = ChordDiagnostics::default();
    assert!(
        diagnose_chord_on_layout(
            &straight(),
            10.,
            2.4,
            qx_geometry::topology::LayoutMode::Circuit,
            &mut d
        )
        .is_err()
    );
    assert_eq!(d.termination, ChordTermination::InvalidTopology);
    // Proximity alone is insufficient: a closed position with wrong tangent fails.
    let mut invalid = t.clone();
    invalid.samples.last_mut().unwrap().frame = Frame::new(Quaternion::IDENTITY).unwrap();
    invalid.samples[0].frame = Frame::new(Quaternion::axis_angle(Vec3::Y, 0.1).unwrap()).unwrap();
    assert!(
        diagnose_chord_on_layout(
            &invalid,
            0.,
            2.4,
            qx_geometry::topology::LayoutMode::Circuit,
            &mut d
        )
        .is_err()
    );
    assert_eq!(d.termination, ChordTermination::InvalidTopology);
}

#[test]
fn invalid_input_and_query_failures_remain_explicit_and_reports_reset() {
    let t = straight();
    for length in [0., 1e-9, -1., f64::NAN, f64::INFINITY, f64::MAX] {
        let (r, d) = solve(&t, 10., length);
        assert!(r.is_err());
        assert_eq!(d.termination, ChordTermination::InvalidLength);
        assert_eq!(d.bracket_probes, 0);
    }
    let mut d = ChordDiagnostics::default();
    diagnose_chord(&t, 10., 2.4, &mut d).unwrap();
    assert!(diagnose_chord(&t, f64::NAN, 2.4, &mut d).is_err());
    assert_eq!(d.termination, ChordTermination::TrackQueryFailed);
    assert_eq!(d.bisection_iterations, 0);
    assert!(d.stations_m.is_none() && d.error.is_some());
}

/// Deliberately folded, unit-speed diagnostic polyline. The two sides have equal
/// Y, so the symmetric chord is the X separation. Corners are isolated from the
/// independently known roots. This is a branch-search fixture, not ride design.
fn folded_track() -> GeneratedTrack {
    let mut t = straight();
    t.samples.clear();
    for n in 0..=8000 {
        let s = n as f64 * 0.001;
        let u = s - 4.;
        let span = 2. * u.abs();
        let (chord, y, dx) = if span < 0.02 {
            (0., span * 0.5, 0.)
        } else if span < 1.025 {
            (span - 0.02, 0.01, 1.)
        } else if span < 1.04 {
            (2.03 - span, 0.01, -1.)
        } else {
            (span - 0.05, 0.01, 1.)
        };
        let tangent = if dx == 0. {
            Vec3::Y * u.signum()
        } else {
            Vec3::X * dx
        };
        let q = if u == 0. {
            Quaternion::IDENTITY
        } else {
            rotation_between(Vec3::X, tangent).unwrap()
        };
        t.samples.push(CenterlineSample {
            distance_m: s,
            design_time_s: s,
            position_m: Vec3::new(u.signum() * chord * 0.5, y, 0.),
            frame: Frame::new(q).unwrap(),
        });
    }
    t
}

#[test]
fn narrow_interval_selects_the_first_of_multiple_intersections() {
    let t = folded_track();
    let ((a, b), d) = {
        let (r, d) = solve(&t, 4., 1.);
        (r.unwrap(), d)
    };
    let first_span = 1.02;
    let p = t.sample_at_distance(4. - first_span * 0.5).unwrap();
    let q = t.sample_at_distance(4. + first_span * 0.5).unwrap();
    assert!(((q.position_m - p.position_m).length() - 1.).abs() < 1e-12);
    assert!((b - a - first_span).abs() < 1e-12);
    assert!(independent_residual(&t, (a, b), 1., false).abs() < 1e-12);
    assert!(d.bracket_span_m.unwrap()[0] < first_span);
    println!(
        "nearest branch: first_span_m={first_span} returned_span_m={:.12}",
        b - a
    );
}

fn independent_residual(
    t: &GeneratedTrack,
    stations: (f64, f64),
    length: f64,
    circuit: bool,
) -> f64 {
    let query = |s: f64| {
        t.sample_at_distance(if circuit {
            s.rem_euclid(t.length_m())
        } else {
            s
        })
        .unwrap()
        .position_m
    };
    (query(stations.1) - query(stations.0)).length() - length
}

/// Exact quadratic Hermite fixture: on spans [0,4], chord = span-span^2/4,
/// with maximum 1 at span 2. Endpoint tangents have unit magnitude, while the
/// interpolated curve deliberately folds/stalls inside the intervals.
fn quadratic_fold() -> GeneratedTrack {
    let mut t = straight();
    t.samples = [
        (0., 2., -1.),
        (2., 0., -1.),
        (4., 0., 1.),
        (6., 0., -1.),
        (8., -2., -1.),
    ]
    .into_iter()
    .map(|(s, x, dx)| CenterlineSample {
        distance_m: s,
        design_time_s: s,
        position_m: Vec3::X * x,
        frame: Frame::new(rotation_between(Vec3::X, Vec3::X * dx).unwrap()).unwrap(),
    })
    .collect();
    t
}

#[test]
fn polynomial_extrema_detect_exact_and_near_tangency_without_dividing_by_slope() {
    let t = quadratic_fold();
    for length in [0.8, 1. - 1e-8, 1. - 1e-12, 1.] {
        let (r, d) = solve(&t, 4., length);
        let stations = r.unwrap();
        let expected = 2. - 2. * (1. - length).sqrt();
        // Conditioning grows without bound at tangency; the exact tangent
        // location remains independently known in this polynomial fixture.
        assert!((stations.1 - stations.0 - expected).abs() < 3e-9);
        assert!(independent_residual(&t, stations, length, false).abs() < 2e-14);
        assert!(d.extrema_tested > 0);
    }
    for length in [1. + 1e-10, 1.0001] {
        let (r, d) = solve(&t, 4., length);
        assert!(r.is_err());
        assert_eq!(d.termination, ChordTermination::NoBracket);
    }
}

#[test]
fn small_large_chords_endpoint_slack_and_out_of_range_are_distinct() {
    let t = straight();
    for length in [1e-8, 1e-6, 0.01, 100., 500.] {
        let (r, d) = solve(&t, 300., length);
        let stations = r.unwrap();
        assert!((stations.1 - stations.0 - length).abs() < 2e-12);
        assert!(independent_residual(&t, stations, length, false).abs() <= d.residual_tolerance_m);
    }
    for center in [0.005, 599.995] {
        let (r, _) = solve(&t, center, 0.01);
        assert!(independent_residual(&t, r.unwrap(), 0.01, false).abs() < 2e-12);
        // A 20 nm shortage is physical, far above roundoff, and must fail.
        assert!(solve(&t, center, 0.01 + 2e-8).0.is_err());
    }
    for center in [-1e-12, 600. + 1e-12, f64::NEG_INFINITY, f64::INFINITY] {
        let (r, d) = solve(&t, center, 0.01);
        assert!(r.is_err());
        assert_eq!(d.termination, ChordTermination::TrackQueryFailed);
    }
}

#[test]
fn invalid_degenerate_geometry_and_work_exhaustion_fail_explicitly() {
    let mut t = straight();
    t.samples.clear();
    assert!(solve(&t, 0., 1.).0.is_err());
    let mut t = quadratic_fold();
    t.samples[2].position_m.x = f64::NAN;
    assert!(solve(&t, 4., 1.).0.is_err());
    let mut t = quadratic_fold();
    t.samples[3].position_m.x = f64::INFINITY;
    assert!(solve(&t, 4., 1.).0.is_err());
    let mut t = quadratic_fold();
    t.samples[3].position_m.x = 1e308;
    assert!(solve(&t, 4., 1.).0.is_err());
    let mut t = quadratic_fold();
    t.samples.last_mut().unwrap().distance_m = 0.;
    assert!(solve(&t, 0., 1.).0.is_err());
    let mut t = quadratic_fold();
    for p in &mut t.samples {
        p.position_m = Vec3::ZERO;
    }
    assert!(solve(&t, 4., 3.).0.is_err());
    // A dense track can exhaust the bounded interval traversal. It must not
    // silently skip the unresolved prefix and return a later branch.
    let mut t = folded_track();
    for p in &mut t.samples {
        p.distance_m *= 0.1;
    }
    let (r, d) = solve(&t, 0.4, 100.);
    assert!(r.is_err());
    assert_eq!(d.termination, ChordTermination::SearchBudgetExceeded);
    assert_eq!(d.bracket_probes, 4096);
    let mut t = straight();
    for p in &mut t.samples {
        p.position_m.x += 1e9;
    }
    let (r, d) = solve(&t, 300., 2.4);
    assert!(r.is_err());
    assert_eq!(d.termination, ChordTermination::ResidualNotMet);
}

#[test]
fn cubic_fold_resolves_two_roots_inside_one_sampler_interval() {
    // Every pair of coincident samples has +X tangents. On spans [0,4],
    // D(span)=span-3*span^2/4+span^3/8. This deliberately pathological diagnostic
    // exercises a genuine degree-six squared-distance polynomial.
    let mut t = quadratic_fold();
    for p in &mut t.samples {
        p.position_m = Vec3::ZERO;
        p.frame = Frame::new(Quaternion::IDENTITY).unwrap();
    }
    let (r, _) = solve(&t, 4., 0.375);
    let stations = r.unwrap();
    let first = (5. - 13_f64.sqrt()) / 2.;
    assert!((stations.1 - stations.0 - first).abs() < 1e-12);
    assert!(independent_residual(&t, stations, 0.375, false).abs() < 1e-14);
    let peak_span = 2. - 2. / 3_f64.sqrt();
    let peak_length = 2. / (3. * 3_f64.sqrt());
    let length = peak_length - 1e-10;
    // Independent monotone analytic cubic reference, never a production helper.
    let (mut a, mut b) = (0., peak_span);
    for _ in 0..80 {
        let x = (a + b) * 0.5;
        if x - 0.75 * x * x + 0.125 * x * x * x < length {
            a = x;
        } else {
            b = x;
        }
    }
    let (r, d) = solve(&t, 4., length);
    let stations = r.unwrap();
    assert!((stations.1 - stations.0 - (a + b) * 0.5).abs() < 1e-9);
    assert!(independent_residual(&t, stations, length, false).abs() < 1e-14);
    assert_eq!(d.bracket_probes, 1);
    assert!(d.extrema_tested > 0);
    assert!(solve(&t, 4., peak_length + 1e-8).0.is_err());
}
