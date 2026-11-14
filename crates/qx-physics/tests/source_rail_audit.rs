//! M4E-A capability audit of real authoring output. This is deliberately NOT a
//! continuous-source adapter: the required source solution/jets are unavailable.
//! The preserved M4C evaluator below measures only the actual sampled geometry.
#[allow(dead_code)]
#[path = "support/m4c_surface.rs"]
mod sampled;

use qx_document::sample;
use qx_geometry::{
    arc_length::{ArcLengthOptions, ArcLengthTable},
    authoring::{BoundaryPolicy, InitialConditions, Section, SectionId, SectionKind},
    heartline::AuthoredReference,
    math::{Quaternion, Vec3},
    rail_geometry::{PhysicalRailGeometry, RailReference, WheelCenterPath},
    solver::{GeneratedTrack, GenerationOptions, generate},
    supports,
    topology::{self, LayoutMode},
    transition::{Channel, Function, Side, Transition, Unit},
};
use sampled::{Jet, Rail, Role, Surface};

fn surface(offset: Vec3, role: Role) -> Surface {
    Surface {
        rail: "explicit-audit-rail",
        face: "M4B-synthetic-guide-datum",
        offset,
        role,
    }
}

// Restrict the preserved evaluator to one ACTUAL generated piece. Rebase only
// station, retaining its exact endpoints, frames and width; no fitted geometry.
fn piece(track: &GeneratedTrack, i: usize) -> Rail {
    let a = &track.samples[i];
    let b = &track.samples[i + 1];
    let mut points = vec![a.clone(), b.clone()];
    points[0].distance_m = 0.;
    points[1].distance_m = b.distance_m - a.distance_m;
    Rail::new(
        GeneratedTrack {
            samples: points,
            graphs: vec![],
            sections: vec![],
            diagnostics: vec![],
            connections: vec![],
            numerical: vec![],
            smooth_ranges: vec![],
        },
        false,
    )
    .unwrap()
}

fn jumps(a: Jet, b: Jet) -> [f64; 6] {
    [
        (a.p - b.p).length(),
        (a.d - b.d).length(),
        (a.dd - b.dd).length(),
        (a.k - b.k).length(),
        (a.n - b.n).length(),
        (a.dn - b.dn).length(),
    ]
}

fn arc(id: u64, radius: f64, length: f64, plane: f64) -> Section {
    let mut s = Section::straight(id, "M4E-A generated arc audit", length);
    s.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: plane,
    };
    s
}

#[test]
fn real_authoring_matrix_reports_sampled_limits_and_regular_local_charts() {
    let initial = InitialConditions::default();
    let banked = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, 0.6).unwrap(),
        ..initial.clone()
    };
    let turn = arc(1, 12., 80., std::f64::consts::FRAC_PI_2);
    let mut changing = turn.clone();
    changing.roll_rate.transitions[0] = Transition {
        id: 1,
        length: 80.,
        start: 0.,
        end: 0.004,
        amplitude: 0.,
        function: Function::Linear {},
    };
    // Constant rider curvature + twist gives a helix about a tilted fixed axis.
    // This fixture goes through generate(), unlike M4D's separate Path::Helix.
    let mut helix = arc(1, 8., 60., 0.);
    helix.roll_rate = Channel::constant(Unit::RadiansPerMetre, 60., 0.04);
    let mut rolling = Section::straight(1, "Existing heartline rolling-line case", 12.);
    rolling.roll_rate = Channel::constant(Unit::RadiansPerMetre, 12., 0.4);
    let foundation = sample::foundation_project();
    let cases = [
        (
            "straight-M1",
            initial.clone(),
            vec![Section::straight(1, "M1 straight fixture", 600.)],
        ),
        (
            "force-hill",
            initial.clone(),
            foundation.sections[..2].to_vec(),
        ),
        ("tight-turn-M1", banked.clone(), vec![turn]),
        (
            "tight-horizontal-turn",
            initial.clone(),
            vec![arc(1, 8., 60., std::f64::consts::FRAC_PI_2)],
        ),
        ("changing-bank-M1", banked, vec![changing]),
        ("curvature-twist-helix", initial.clone(), vec![helix]),
        ("rolling-line-rail-helix", initial.clone(), vec![rolling]),
        (
            "vertical-loop",
            initial.clone(),
            vec![arc(1, 12., 12. * std::f64::consts::TAU, 0.)],
        ),
        (
            "foundation-regions",
            foundation.initial,
            foundation.sections,
        ),
        (
            "mixed-authoring",
            initial.clone(),
            sample::mixed_authoring_project().sections,
        ),
        (
            "native-numerical",
            initial,
            sample::numerical_reference_project().sections,
        ),
    ];
    for (name, initial, sections) in cases {
        let source_before = (initial.clone(), sections.clone());
        let track = generate(&initial, &sections, GenerationOptions::default()).unwrap();
        let before = track.clone();
        assert_eq!(
            track,
            generate(&initial, &sections, GenerationOptions::default()).unwrap()
        );
        assert_eq!(
            track.sections.iter().map(|r| r.id).collect::<Vec<_>>(),
            sections.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        assert!(
            track
                .samples
                .windows(2)
                .all(|p| p[0].distance_m.is_finite() && p[1].distance_m > p[0].distance_m)
        );
        for query in [-1e-9, track.length_m() + 1e-9, f64::NAN, f64::INFINITY] {
            assert!(track.sample_at_distance(query).is_err());
        }
        assert!(track.sample_at_distance(0.).is_ok());
        assert!(track.sample_at_distance(track.length_m()).is_ok());
        let mut worst = [0_f64; 6];
        let mut join_worst = [0_f64; 6];
        let mut minimum_d = f64::INFINITY;
        let mut position_error = 0_f64;
        for i in 0..track.samples.len() - 1 {
            let right = piece(&track, i);
            for offset in [
                Vec3::ZERO,
                Vec3::new(0., -0.08, 0.6),
                Vec3::new(0.1, -0.08, -0.6),
            ] {
                for role in [Role::Running, Role::Side] {
                    let datum = surface(offset, role);
                    if i > 0 {
                        let left = piece(&track, i - 1);
                        let errors = jumps(
                            left.jet(datum, left.length()).unwrap(),
                            right.jet(datum, 0.).unwrap(),
                        );
                        for (max, error) in worst.iter_mut().zip(errors) {
                            *max = max.max(error);
                        }
                        if track.sections.iter().skip(1).any(|r| r.start_sample == i) {
                            for (max, error) in join_worst.iter_mut().zip(errors) {
                                *max = max.max(error);
                            }
                        }
                    }
                    let local = right.length() * 0.37;
                    let jet = right.jet(datum, local).unwrap();
                    let station = track.samples[i].distance_m + local;
                    let old = track.sample_at_distance(station).unwrap();
                    position_error = position_error
                        .max((jet.p - old.position_m - old.frame.local_to_world(offset)).length());
                    // Local finite-residual regularity, NOT a whole-interval or
                    // branch-uniqueness certificate. B1's original 1e-8 gate.
                    let d = jet.d.dot(jet.d) - (jet.n * 0.002).dot(jet.dd);
                    assert!(d.is_finite() && d > 1e-8, "{name} at {station}: D={d}");
                    minimum_d = minimum_d.min(d);
                    if i == 1 || i == track.samples.len() / 2 {
                        let h = right.length() * 1e-3;
                        let a = right.jet(datum, local - h).unwrap();
                        let b = right.jet(datum, local + h).unwrap();
                        assert!(((b.p - a.p) / (2. * h) - jet.d).length() < 2e-7, "{name}");
                        assert!(((b.d - a.d) / (2. * h) - jet.dd).length() < 2e-7, "{name}");
                        assert!(((b.n - a.n) / (2. * h) - jet.dn).length() < 2e-7, "{name}");
                        let x = jet.p + jet.n * 0.002;
                        let projected = right
                            .project(datum, x, local, right.length() * 0.2)
                            .unwrap()
                            .0;
                        assert!((projected - local).abs() < 2e-7, "{name}");
                    }
                }
            }
        }
        assert!(worst[0] < 2e-10 && position_error < 2e-10, "{name}");
        if name == "changing-bank-M1" {
            assert!(worst[1] > 1e-7);
        }
        if name == "curvature-twist-helix" {
            let axis = Vec3::new(0.04, 0., 1. / 8.).normalized().unwrap();
            for point in &track.samples {
                assert!((point.position_m.dot(axis) - point.distance_m * axis.x).abs() < 1e-8);
            }
        }
        for pair in track.sections.windows(2) {
            assert_eq!(pair[0].end_sample, pair[1].start_sample);
            assert_eq!(pair[0].end_distance_m, pair[1].start_distance_m);
        }
        println!(
            "M4EA_CASE {name} samples={} sections={} smooth={} length={:.12} p_d_dd_k_n_dn={worst:?} section_jumps={join_worst:?} min_D={minimum_d:.9e} sampler_position_error={position_error:.9e}",
            track.samples.len(),
            track.sections.len(),
            track.smooth_ranges.len(),
            track.length_m()
        );
        assert_eq!(track, before);
        assert_eq!((initial, sections), source_before);
    }
}

#[test]
fn native_sources_retain_positions_but_g1_and_c2_are_insufficient() {
    let source = sample::spline_comparison_project();
    let track = generate(&source.initial, &source.sections, Default::default()).unwrap();
    let mut nong2 = 0;
    let mut max_node = 0_f64;
    let mut max_between = 0_f64;
    for (section, range) in source.sections.iter().zip(&track.sections) {
        let SectionKind::Spline { curve } = &section.kind else {
            unreachable!()
        };
        let prepared = curve.prepare().unwrap();
        let table = ArcLengthTable::build(&prepared, ArcLengthOptions::default()).unwrap();
        let anchor = &track.samples[range.start_sample];
        for i in range.start_sample..range.end_sample {
            for s in [
                track.samples[i].distance_m,
                (track.samples[i].distance_m + track.samples[i + 1].distance_m) * 0.5,
            ] {
                let u = table
                    .parameter_at_distance(
                        &prepared,
                        (s - range.start_distance_m).min(table.length_m()),
                    )
                    .unwrap();
                let e = prepared.evaluate(u, Side::Right).unwrap();
                let expected = anchor.position_m + anchor.frame.local_to_world(e.position_m);
                let error = (track.sample_at_distance(s).unwrap().position_m - expected).length();
                if s == track.samples[i].distance_m {
                    max_node = max_node.max(error);
                } else {
                    max_between = max_between.max(error);
                }
            }
        }
        for join in prepared.continuity().unwrap() {
            assert!(join.g1(1e-8));
            if !join.g2(1e-8) {
                nong2 += 1;
            }
            // Independent differences of existing SECOND derivatives estimate
            // third native derivatives; they do not create a source jet API.
            let h = 1e-6;
            let u = join.parameter;
            let a = prepared.evaluate(u, Side::Left).unwrap();
            let b = prepared.evaluate(u, Side::Right).unwrap();
            let third_left = (a.second - prepared.evaluate(u - h, Side::Right).unwrap().second) / h;
            let third_right =
                (prepared.evaluate(u + h, Side::Right).unwrap().second - b.second) / h;
            let third_gap = (third_left - third_right).length();
            if curve.method() == "b_spline" {
                assert!(join.c2(1e-8) && third_gap > 1.);
            }
            println!(
                "M4EA_NATIVE method={} u={u:.9} C2={} G2={} curvature_gap={:.9e} estimated_native_third_gap={third_gap:.9e}",
                curve.method(),
                join.c2(1e-8),
                join.g2(1e-8),
                join.curvature_vector_gap_per_m
            );
        }
    }
    assert!(nong2 > 0);
    assert!(max_node < 2e-9 && max_between > max_node);
    println!(
        "M4EA_NATIVE_POSITION max_node={max_node:.9e} max_between={max_between:.9e} nong2={nong2}"
    );
}

#[test]
fn continuous_rate_policy_does_not_certify_second_or_third_datum_derivatives() {
    for function in [Function::Linear {}, Function::Quadratic {}] {
        let mut a = Section::straight(1, "incoming source roll", 5.);
        a.roll_rate.transitions[0] = Transition {
            id: 1,
            length: 5.,
            start: 0.,
            end: 0.02,
            amplitude: 0.,
            function: function.clone(),
        };
        let left = a.roll_rate.transitions[0].evaluate(5.).unwrap();
        let mut b = Section::straight(2, "continuous incoming rate", 5.);
        b.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
        b.roll_rate.transitions[0] = Transition {
            id: 1,
            length: 5.,
            start: 0.02,
            end: 0.02 + left.derivative * 5.,
            amplitude: 0.,
            function: Function::Linear {},
        };
        if matches!(function, Function::Linear {}) {
            b.roll_rate = Channel::constant(Unit::RadiansPerMetre, 5., 0.02);
        }
        let right = b.roll_rate.transitions[0].evaluate(0.).unwrap();
        let track = generate(&InitialConditions::default(), &[a, b], Default::default()).unwrap();
        assert!(track.connections[0].twist_rate_gap_rad_per_m.abs() < 1e-12);
        let frame = track.samples[track.sections[0].end_sample].frame;
        let r = frame.local_to_world(Vec3::new(0., -0.08, 0.6));
        let jet = |v: qx_geometry::transition::Evaluation| {
            let k = Vec3::X * v.value;
            let dk = Vec3::X * v.derivative;
            let ddk = Vec3::X * v.second_derivative;
            (
                k.cross(r),
                dk.cross(r) + k.cross(k.cross(r)),
                ddk.cross(r)
                    + dk.cross(k.cross(r)) * 2.
                    + k.cross(dk.cross(r))
                    + k.cross(k.cross(k.cross(r))),
            )
        };
        let (l1, l2, l3) = jet(left);
        let (r1, r2, r3) = jet(right);
        // Position-only one-sided differences of independently integrated roll
        // polynomials. This verifies the boundary algebra, not an unavailable
        // dense source evaluator for arbitrary sections.
        for (direction, expected_second, expected_third) in [(1., r2, r3), (-1., l2, l3)] {
            let h = 0.02;
            let position = |i: usize| {
                let t = direction * h * i as f64;
                let quadratic = matches!(function, Function::Quadratic {});
                let theta = 0.02 * t
                    + if quadratic {
                        0.004 * t * t
                    } else if direction < 0. {
                        0.002 * t * t
                    } else {
                        0.
                    }
                    + if quadratic && direction < 0. {
                        0.0008 * t.powi(3) / 3.
                    } else {
                        0.
                    };
                Vec3::X * t
                    + Vec3::new(
                        r.x,
                        r.y * theta.cos() - r.z * theta.sin(),
                        r.y * theta.sin() + r.z * theta.cos(),
                    )
            };
            let p: [Vec3; 5] = std::array::from_fn(position);
            let second =
                (p[0] * 35. - p[1] * 104. + p[2] * 114. - p[3] * 56. + p[4] * 11.) / (12. * h * h);
            let third = (p[0] * -5. + p[1] * 18. - p[2] * 24. + p[3] * 14. - p[4] * 3.)
                / (2. * direction * h.powi(3));
            assert!((second - expected_second).length() < 2e-7);
            assert!((third - expected_third).length() < 2e-6);
        }
        assert!((l1 - r1).length() < 1e-14);
        if matches!(function, Function::Linear {}) {
            assert!((l2 - r2).length() > 0.002);
        } else {
            assert!((l2 - r2).length() < 1e-14 && (l3 - r3).length() > 0.0009);
        }
        println!(
            "M4EA_SOURCE_JOIN {function:?} P1_gap={:.9e} P2_gap={:.9e} P3_gap={:.9e}",
            (l1 - r1).length(),
            (l2 - r2).length(),
            (l3 - r3).length()
        );
    }
    // Existing authored jump remains explicit and unchanged, with strict-policy rejection.
    let mut a = Section::straight(1, "abrupt roll", 10.);
    a.roll_rate.transitions = vec![
        Transition::constant(1, 5., 0.),
        Transition::constant(2, 5., 0.02),
    ];
    let mut b = Section::straight(2, "next", 10.);
    let before = a.clone();
    let track = generate(
        &InitialConditions::default(),
        &[a.clone(), b.clone()],
        Default::default(),
    )
    .unwrap();
    assert!(
        track
            .diagnostics
            .iter()
            .any(|d| d.code == "channel_discontinuity")
    );
    b.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
    assert!(
        generate(
            &InitialConditions::default(),
            &[a.clone(), b],
            Default::default()
        )
        .is_err()
    );
    assert_eq!(a, before);
}

#[test]
fn physical_conversion_offsets_seams_and_source_identity_remain_explicit() {
    let mut p = sample::heartline_reference_project();
    let source = generate(&p.initial, &p.sections, Default::default()).unwrap();
    p.heartline.authored_reference = AuthoredReference::RiderReference;
    let reference = p.reference_geometry(&source).unwrap();
    let physical = supports::physical_attachment_track(&source, &reference).unwrap();
    assert!(physical.smooth_ranges.is_empty());
    assert!((physical.length_m() - source.length_m()).abs() > 12.);
    assert_eq!(physical.sections[0].id, source.sections[0].id);
    let rail = RailReference::from_construction(&p.construction, "rail-right").unwrap();
    assert_eq!(rail.offset_m, Vec3::new(0., 0., 0.55));
    let geometry = PhysicalRailGeometry::new(&physical, LayoutMode::Open).unwrap();
    let path = WheelCenterPath {
        rail,
        center_offset_m: Vec3::new(0., -0.08, 0.),
    };
    let point = geometry.sample(&path, physical.length_m() * 0.4).unwrap();
    let exact = physical.sample_at_distance(point.station_m).unwrap();
    assert!(
        (point.wheel_center_m
            - exact.position_m
            - exact
                .frame
                .local_to_world(path.rail.offset_m + path.center_offset_m))
        .length()
            < 1e-11
    );
    assert!(topology::diagnose(&source).unwrap().valid);
    let first = piece(&source, 0);
    let last = piece(&source, source.samples.len() - 2);
    let seam = jumps(
        last.jet(surface(path.rail.offset_m, Role::Side), last.length())
            .unwrap(),
        first
            .jet(surface(path.rail.offset_m, Role::Side), 0.)
            .unwrap(),
    );
    println!(
        "M4EA_PHYSICAL source_length={:.12} physical_length={:.12} seam_p_d_dd_k_n_dn={seam:?}",
        source.length_m(),
        physical.length_m()
    );
    // Closure is only a pose tolerance, not a periodic-jet certificate.
    let mut tolerated = source.clone();
    tolerated.samples.last_mut().unwrap().position_m.x += 0.0005;
    assert!(topology::diagnose(&tolerated).unwrap().valid);
    assert!(matches!(
        Rail::new(tolerated, true),
        Err(sampled::Failure::Corner)
    ));
    // Conversion clones this source-station rate; it is not a physical-station
    // orientation jet. The inverse circle has radius 22 m instead of 20 m.
    assert_eq!(
        physical.sections[0].end_angular_rate_rad_per_m,
        source.sections[0].end_angular_rate_rad_per_m
    );
    assert!((physical.sections[0].end_angular_rate_rad_per_m.length() - 1. / 22.).abs() > 0.004);
    let mut renamed = p.sections.clone();
    renamed[0].id = SectionId(9001);
    let regenerated = generate(&p.initial, &renamed, Default::default()).unwrap();
    assert_eq!(regenerated.samples, source.samples);
    assert_ne!(regenerated.sections[0].id, source.sections[0].id);
    // Equal shape/length cannot establish source identity after replacement.
    assert_ne!(renamed, p.sections);
    assert_eq!(
        generate(&p.initial, &p.sections, Default::default()).unwrap(),
        source
    );
}
