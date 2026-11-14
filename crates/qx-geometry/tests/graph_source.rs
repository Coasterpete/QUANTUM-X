#![cfg(feature = "experimental-graph-source")]
use qx_geometry::{
    authoring::{BoundaryPolicy, Domain, InitialConditions, Section, SectionKind},
    heartline::AuthoredReference,
    math::{Frame, Quaternion, Vec3},
    solver::{
        GenerationOptions, generate,
        prepared::{PreparedGraph, SourceIdentity},
    },
    transition::{Channel, Function, Side, Transition, Unit},
};
const ID: SourceIdentity = SourceIdentity {
    document: 0x04eb,
    generation: 1,
};
fn options(step: f64) -> GenerationOptions {
    GenerationOptions {
        max_step_m: step,
        max_rotation_per_step_rad: 0.5,
        ..Default::default()
    }
}
fn prepare(initial: &InitialConditions, sections: &[Section], step: f64) -> PreparedGraph {
    PreparedGraph::prepare(
        ID,
        AuthoredReference::TrackCenterline,
        initial,
        sections,
        options(step),
    )
    .unwrap()
}
fn profile(length: f64, start: f64, end: f64, function: Function) -> Channel {
    Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition {
            id: 1,
            length,
            start,
            end,
            amplitude: 0.,
            function,
        }],
    }
}
fn arc(radius: f64, length: f64, plane: f64) -> Section {
    let mut s = Section::straight(1, "Retained graph arc fixture", length);
    s.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: plane,
    };
    s
}
fn side(s: f64, length: f64) -> Side {
    if s == length { Side::Left } else { Side::Right }
}
fn close(a: Vec3, b: Vec3, tol: f64) {
    assert!(
        (a - b).length() < tol,
        "{a:?} != {b:?}, error {}",
        (a - b).length()
    );
}

#[test]
fn straight_arc_helix_and_rider_local_match_independent_exponentials() {
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, 0.6).unwrap(),
        ..Default::default()
    };
    let mut helix = arc(8., 60., 0.);
    helix.roll_rate = Channel::constant(Unit::RadiansPerMetre, 60., 0.04);
    let mut local = Section::straight(1, "Existing rider-local controls", 30.);
    local.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 30., -0.008),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 30., 0.006),
    };
    let cases = [
        (Section::straight(1, "M1 straight", 600.), Vec3::ZERO),
        (
            arc(12., 80., std::f64::consts::FRAC_PI_2),
            Vec3::new(0., -1. / 12., 0.),
        ),
        (
            arc(12., 12. * std::f64::consts::TAU, 0.),
            Vec3::new(0., 0., 1. / 12.),
        ),
        (helix, Vec3::new(0.04, 0., 0.125)),
        (local, Vec3::new(0., -0.006, -0.008)),
    ];
    for (section, local_k) in cases {
        let source = prepare(&initial, std::slice::from_ref(&section), 0.1);
        let f = Frame::new(initial.orientation).unwrap();
        let k = f.local_to_world(local_k);
        let w = k.length();
        let t0 = f.tangent();
        let mut max_p = 0_f64;
        let mut max_r = 0_f64;
        for j in 0..=71 {
            let s = source.length_m() * j as f64 / 71.;
            let e = source.evaluate(ID, s, side(s, source.length_m())).unwrap();
            assert!(e.local_rk4_steps <= 2);
            let (position, rotation) = if w == 0. {
                (initial.position_m + t0 * s, initial.orientation)
            } else {
                let u = k / w;
                let parallel = u * t0.dot(u);
                let perp = t0 - parallel;
                (
                    initial.position_m
                        + parallel * s
                        + perp * ((w * s).sin() / w)
                        + u.cross(t0) * ((1. - (w * s).cos()) / w),
                    Quaternion::axis_angle(u, w * s)
                        .unwrap()
                        .compose(initial.orientation),
                )
            };
            let frame = Frame::new(rotation).unwrap();
            let t = frame.tangent();
            max_p = max_p.max((e.position_m - position).length());
            max_r = max_r.max(
                e.frame
                    .orientation()
                    .compose(rotation.conjugate())
                    .rotation_vector()
                    .unwrap()
                    .length(),
            );
            close(e.position_m, position, 2e-9);
            close(e.position_derivatives[0], t, 3e-10);
            close(e.position_derivatives[1], k.cross(t), 3e-11);
            close(e.position_derivatives[2], k.cross(k.cross(t)), 3e-12);
            close(e.angular_derivatives[0], k, 3e-12);
            close(e.angular_derivatives[1], Vec3::ZERO, 1e-14);
            close(e.angular_derivatives[2], Vec3::ZERO, 1e-14);
        }
        println!(
            "M4EB_ANALYTIC {:?} position={max_p:.12e} orientation={max_r:.12e}",
            section.kind
        );
    }
}

#[test]
fn fourth_order_refinement_and_existing_generated_pose_agreement() {
    let initial = InitialConditions::default();
    let section = arc(10., 10., 0.);
    let exact = Vec3::new(10. * 1_f64.sin(), 10. * (1. - 1_f64.cos()), 0.);
    let errors = [2., 1., 0.5].map(|h| {
        let source = prepare(&initial, std::slice::from_ref(&section), h);
        let e = source.evaluate(ID, 10., Side::Left).unwrap();
        println!(
            "M4EB_REFINEMENT step={h} actual={:.12e} difference={:.12e}",
            (e.position_m - exact).length(),
            e.refinement_position_difference_m
        );
        (e.position_m - exact).length()
    });
    assert!(
        errors[0] / errors[1] > 12. && errors[1] / errors[2] > 12.,
        "{errors:?}"
    );
    let query = 8.7316;
    let exact_query = Vec3::new(
        10. * (query / 10_f64).sin(),
        10. * (1. - (query / 10_f64).cos()),
        0.,
    );
    let arbitrary_errors = [2., 1., 0.5].map(|h| {
        let source = prepare(&initial, std::slice::from_ref(&section), h);
        let e = source.evaluate(ID, query, Side::Right).unwrap();
        let error = (e.position_m - exact_query).length();
        println!("M4EB_ARBITRARY_REFINEMENT step={h} station={query} error={error:.12e}");
        error
    });
    assert!(
        arbitrary_errors[0] / arbitrary_errors[1] > 10.
            && arbitrary_errors[1] / arbitrary_errors[2] > 10.,
        "{arbitrary_errors:?}"
    );
    let source = prepare(&initial, std::slice::from_ref(&section), 0.1);
    let generated = generate(&initial, &[section], options(0.05)).unwrap();
    for p in &generated.samples {
        let s = p.distance_m.clamp(0., 10.);
        let e = source.evaluate(ID, s, side(s, 10.)).unwrap();
        close(e.position_m, p.position_m, 1e-11);
        close(e.frame.normal(), p.frame.normal(), 1e-12);
    }
}

#[test]
fn variable_roll_jets_and_constant_offset_match_closed_form() {
    let initial = InitialConditions::default();
    let mut section = Section::straight(1, "Quadratic roll", 12.);
    section.roll_rate = profile(12., 0.02, 0.056, Function::Quadratic {});
    let source = prepare(&initial, &[section], 0.1);
    let a = Vec3::new(0.1, -0.08, 0.6);
    for s in [0., 0.0137, 5., 7.431, 12.] {
        let e = source.evaluate(ID, s, side(s, 12.)).unwrap();
        let theta = 0.02 * s + 0.00025 * s * s * s / 3.;
        let frame = Frame::new(Quaternion::axis_angle(Vec3::X, theta).unwrap()).unwrap();
        close(e.frame.normal(), frame.normal(), 1e-11);
        close(
            e.angular_derivatives[0],
            Vec3::X * (0.02 + 0.00025 * s * s),
            1e-13,
        );
        close(e.angular_derivatives[1], Vec3::X * (0.0005 * s), 1e-13);
        close(e.angular_derivatives[2], Vec3::X * 0.0005, 1e-13);
        close(
            e.constant_offset(a).unwrap()[0],
            Vec3::X * s + frame.local_to_world(a),
            1e-11,
        );
    }
}

#[test]
fn time_and_distance_jets_share_physical_station() {
    let initial = InitialConditions::default();
    let mut d = Section::straight(1, "Distance", 40.);
    d.roll_rate = profile(40., 0.01, 0.03, Function::QuinticSmoothstep {});
    let mut t = d.clone();
    t.domain = Domain::Time;
    t.extent = 2.;
    t.roll_rate = profile(2., 0.2, 0.6, Function::QuinticSmoothstep {});
    t.roll_rate.unit = Unit::RadiansPerSecond;
    let a = prepare(&initial, &[d], 0.1);
    let b = prepare(&initial, &[t], 0.1);
    assert_eq!(a.length_m(), b.length_m());
    for s in [0., 1.234, 20., 39.99, 40.] {
        let ea = a.evaluate(ID, s, side(s, 40.)).unwrap();
        let eb = b.evaluate(ID, s, side(s, 40.)).unwrap();
        assert!((ea.section_input - eb.section_input * 20.).abs() < 1e-12);
        close(ea.frame.normal(), eb.frame.normal(), 2e-13);
        for (u, v) in ea
            .angular_derivatives
            .into_iter()
            .zip(eb.angular_derivatives)
        {
            close(u, v, 1e-13);
        }
    }
}

#[test]
fn section_and_internal_boundaries_reject_each_missing_angular_order() {
    let initial = InitialConditions::default();
    for order in 0..3 {
        let mut a = Section::straight(1, "Left jet", 5.);
        let mut b = Section::straight(2, "Right jet", 5.);
        match order {
            0 => {
                b.roll_rate = Channel::constant(Unit::RadiansPerMetre, 5., 0.02);
            }
            1 => {
                a.roll_rate = profile(5., 0., 0.02, Function::Linear {});
                b.roll_rate = Channel::constant(Unit::RadiansPerMetre, 5., 0.02);
            }
            _ => {
                a.roll_rate = profile(5., 0., 0.02, Function::Quadratic {});
                b.roll_rate = profile(5., 0.02, 0.06, Function::Linear {});
            }
        }
        if order > 0 {
            b.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
        }
        let source = prepare(&initial, &[a.clone(), b.clone()], 0.1);
        assert_eq!(source.boundaries().len(), 1);
        let join = &source.boundaries()[0];
        assert!(!join.compatible);
        assert!(join.angular_gaps[order] > 1e-4);
        assert!(source.validate_interval(ID, 4., 6.).is_err());
        assert!(source.validate_interval(ID, 0., 4.).is_ok());
        let l = source.evaluate(ID, 5., Side::Left).unwrap();
        let r = source.evaluate(ID, 5., Side::Right).unwrap();
        assert_eq!(l.section, a.id);
        assert_eq!(r.section, b.id);
        assert_eq!(l.section_input, 5.);
        assert_eq!(r.section_input, 0.);
        close(l.position_m, r.position_m, 1e-14);
        close(l.frame.normal(), r.frame.normal(), 1e-14);
        for j in 0..3 {
            assert!(
                ((l.angular_derivatives[j] - r.angular_derivatives[j]).length()
                    - join.angular_gaps[j])
                    .abs()
                    < 1e-12
            );
        }
        println!("M4EB_JOIN order={order} gaps={:?}", join.angular_gaps);
        // The same source change inside a channel retains explicit sided charts.
        let mut combined = Section::straight(1, "Internal transition", 10.);
        let mut end = b.roll_rate.transitions[0].clone();
        end.id = 2;
        combined.roll_rate = Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![a.roll_rate.transitions[0].clone(), end],
        };
        let internal = prepare(&initial, &[combined], 0.1);
        assert!(!internal.boundaries()[0].compatible);
        assert!(internal.validate_interval(ID, 4., 6.).is_err());
        let left = internal.evaluate(ID, 5., Side::Left).unwrap();
        let right = internal.evaluate(ID, 5., Side::Right).unwrap();
        assert_ne!(left.chart, right.chart);
    }
    let a = Section::straight(1, "Connected", 12.);
    let b = Section::straight(2, "Connected", 8.);
    let accepted = prepare(&initial, &[a, b], 0.1);
    assert!(accepted.boundaries()[0].compatible);
    accepted.validate_interval(ID, 0., 20.).unwrap();
}

#[test]
fn identities_domains_unsupported_sources_and_resource_limits_are_explicit() {
    let initial = InitialConditions::default();
    let section = Section::straight(1, "Source", 10.);
    let source = prepare(&initial, std::slice::from_ref(&section), 0.1);
    assert!(source.matches_source(&initial, std::slice::from_ref(&section), options(0.1)));
    let mut renamed = section.clone();
    renamed.id.0 = 99;
    assert!(!source.matches_source(&initial, &[renamed], options(0.1)));
    assert!(!source.matches_source(&initial, std::slice::from_ref(&section), options(0.2)));
    let stale = SourceIdentity {
        generation: 2,
        ..ID
    };
    assert!(source.evaluate(stale, 2., Side::Right).is_err());
    for s in [-1e-12, 10. + 1e-12, f64::NAN, f64::INFINITY] {
        assert!(source.evaluate(ID, s, Side::Right).is_err());
    }
    assert!(source.evaluate(ID, 0., Side::Left).is_err());
    assert!(source.evaluate(ID, 10., Side::Right).is_err());
    assert!(
        PreparedGraph::prepare(
            ID,
            AuthoredReference::RiderReference,
            &initial,
            std::slice::from_ref(&section),
            options(0.1)
        )
        .is_err()
    );
    let mut world = section.clone();
    world.kind = SectionKind::Geometric {
        pitch_rate: section.roll_rate.clone(),
        yaw_rate: section.roll_rate.clone(),
    };
    assert!(
        PreparedGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            &initial,
            &[world],
            options(0.1)
        )
        .is_err()
    );
    let mut budget = options(0.1);
    budget.max_samples = 150; // coarse fits, fine does not
    assert!(
        PreparedGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            &initial,
            &[section],
            budget
        )
        .is_err()
    );
    let long = prepare(&initial, &[Section::straight(1, "Long", 600.)], 0.1);
    assert!(long.checkpoint_count() > 10000);
    for s in [0.0137, 300.0137, 599.9137] {
        assert!(long.evaluate(ID, s, Side::Right).unwrap().local_rk4_steps <= 2);
    }
}

#[test]
fn independent_pose_stencils_cover_force_and_variable_rider_controls() {
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, 0.4).unwrap(),
        ..Default::default()
    };
    let mut local = Section::straight(1, "Rider profiles", 30.);
    local.kind = SectionKind::RiderLocal {
        pitch_rate: profile(30., -0.008, 0.02, Function::QuinticSmoothstep {}),
        yaw_rate: profile(30., 0.006, -0.01, Function::Cosine {}),
    };
    local.roll_rate = profile(30., 0.02, 0.06, Function::Quadratic {});
    let mut force = local.clone();
    force.kind = SectionKind::Force {
        normal_g: Channel {
            unit: Unit::G,
            ..profile(30., 1., 1.7, Function::QuinticSmoothstep {})
        },
        lateral_g: Channel {
            unit: Unit::G,
            ..profile(30., 0., 0.35, Function::QuinticSmoothstep {})
        },
    };
    for (name, section) in [("rider", local), ("force", force)] {
        let source = prepare(&initial, std::slice::from_ref(&section), 0.1);
        for center in [0.2, 7.431, 15.0137, 29.8] {
            let jet = source.evaluate(ID, center, Side::Right).unwrap();
            let h = 0.02;
            let rows: Vec<Vec<f64>> = (-8..=8)
                .map(|i| {
                    let e = source
                        .evaluate(ID, center + i as f64 * h, Side::Right)
                        .unwrap();
                    [
                        e.position_m,
                        e.frame.tangent(),
                        e.frame.normal(),
                        e.frame.lateral(),
                    ]
                    .into_iter()
                    .flat_map(|v| [v.x, v.y, v.z])
                    .collect()
                })
                .collect();
            let expected: Vec<f64> = jet
                .position_derivatives
                .into_iter()
                .chain(jet.angular_derivatives)
                .flat_map(|v| [v.x, v.y, v.z])
                .collect();
            println!("M4EB_STENCIL {name} {center} {h} {rows:?}");
            println!("M4EB_JET {expected:?}");
            let offset = Vec3::new(0.1, -0.08, 0.6);
            let offset_jet: Vec<f64> = [offset]
                .into_iter()
                .chain(jet.constant_offset(offset).unwrap().into_iter().skip(1))
                .flat_map(|v| [v.x, v.y, v.z])
                .collect();
            println!("M4EB_OFFSET {offset_jet:?}");
            let delta = 1e-3;
            let a = source.evaluate(ID, center - delta, Side::Right).unwrap();
            let b = source.evaluate(ID, center + delta, Side::Right).unwrap();
            close(
                (b.position_m - a.position_m) / (2. * delta),
                jet.position_derivatives[0],
                2e-8,
            );
            close(
                (b.position_derivatives[0] - a.position_derivatives[0]) / (2. * delta),
                jet.position_derivatives[1],
                2e-9,
            );
            close(
                (b.position_derivatives[1] - a.position_derivatives[1]) / (2. * delta),
                jet.position_derivatives[2],
                2e-9,
            );
            close(
                (b.angular_derivatives[0] - a.angular_derivatives[0]) / (2. * delta),
                jet.angular_derivatives[1],
                2e-9,
            );
            close(
                (b.angular_derivatives[1] - a.angular_derivatives[1]) / (2. * delta),
                jet.angular_derivatives[2],
                2e-9,
            );
        }
    }
}

#[test]
fn force_circle_native_rejection_and_offset_singularity() {
    let initial = InitialConditions::default();
    let mut force = Section::straight(1, "Existing R40 force-circle fixture", 30.);
    force.kind = SectionKind::Force {
        normal_g: Channel::constant(Unit::G, 30., 1.),
        lateral_g: Channel::constant(
            Unit::G,
            30.,
            initial.reference_speed_mps.powi(2) / (initial.gravity_mps2 * 40.),
        ),
    };
    let source = prepare(&initial, &[force], 0.1);
    for s in [0., 1.234, 15.0137, 30.] {
        let e = source.evaluate(ID, s, side(s, 30.)).unwrap();
        let angle = s / 40.;
        close(
            e.position_m,
            Vec3::new(40. * angle.sin(), 0., 40. * (1. - angle.cos())),
            2e-10,
        );
        close(e.angular_derivatives[0], Vec3::new(0., -0.025, 0.), 1e-12);
        close(e.angular_derivatives[1], Vec3::ZERO, 1e-13);
        close(e.angular_derivatives[2], Vec3::ZERO, 1e-13);
    }
    use qx_geometry::curve::{ControlPoint, Curve};
    let spline = Section::spline(
        1,
        "Native excluded",
        Curve::Bezier {
            points: vec![
                ControlPoint {
                    id: 1,
                    position_m: Vec3::ZERO,
                },
                ControlPoint {
                    id: 2,
                    position_m: Vec3::X * 10.,
                },
            ],
        },
    );
    assert!(
        PreparedGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            &initial,
            &[spline],
            options(0.1)
        )
        .is_err()
    );
    let circle = prepare(&initial, &[arc(10., 10., 0.)], 0.1);
    assert!(
        circle
            .evaluate(ID, 0., Side::Right)
            .unwrap()
            .constant_offset(Vec3::Y * 10.)
            .is_err()
    );
}

#[test]
fn scalar_nurbs_profiles_reuse_existing_second_derivatives_and_sided_knots() {
    use qx_geometry::profile::{Continuity, NurbsProfile};
    let initial = InitialConditions::default();
    for continuity in [Continuity::C0, Continuity::C1] {
        let mut p = NurbsProfile::cubic(0., 0.03);
        for _ in 0..3 {
            p.insert_knot(0.5).unwrap();
        }
        p.interior = continuity;
        p.values[if continuity == Continuity::C0 { 4 } else { 5 }] += 0.002;
        let mut s = Section::straight(1, "Existing scalar repeated-knot fixture", 10.);
        s.roll_rate = profile(
            10.,
            p.values[0],
            *p.values.last().unwrap(),
            Function::Nurbs { profile: p.clone() },
        );
        let source = prepare(&initial, &[s], 0.1);
        assert!(source.boundaries().iter().any(|b| !b.compatible));
        assert!(source.validate_interval(ID, 4., 6.).is_err());
        for side in [Side::Left, Side::Right] {
            let jet = source.evaluate(ID, 5., side).unwrap();
            let scalar = p.evaluate(0.5, 10., side).unwrap();
            for (actual, value) in jet.angular_derivatives.into_iter().zip([
                scalar.value,
                scalar.derivative,
                scalar.second_derivative,
            ]) {
                close(actual, Vec3::X * value, 1e-13);
            }
        }
    }
}

#[test]
#[ignore = "serial Release observation; no real-time guarantee"]
fn prepared_cost_and_bounded_query_work() {
    use std::{hint::black_box, time::Instant};
    let initial = InitialConditions::default();
    for length in [30., 600.] {
        let mut s = Section::straight(1, "Bounded query cost", length);
        s.kind = SectionKind::RiderLocal {
            pitch_rate: profile(length, -0.008, 0.02, Function::QuinticSmoothstep {}),
            yaw_rate: profile(length, 0.006, -0.01, Function::Cosine {}),
        };
        s.roll_rate = profile(length, 0.02, 0.06, Function::Quadratic {});
        let started = Instant::now();
        let source = prepare(&initial, &[s], 0.1);
        let preparation = started.elapsed();
        let mut times = Vec::new();
        let mut maximum_steps = 0;
        for _ in 0..11 {
            let started = Instant::now();
            for i in 0..1000 {
                let station = length * ((i as f64 + 0.137) / 1000.);
                let e = source
                    .evaluate(ID, black_box(station), Side::Right)
                    .unwrap();
                maximum_steps = maximum_steps.max(e.local_rk4_steps);
                black_box(e);
            }
            times.push(started.elapsed().as_secs_f64() * 1e6 / 1000.);
        }
        times.sort_by(f64::total_cmp);
        assert!(maximum_steps <= 2);
        println!(
            "M4EB_COST length={length} prepare_ms={:.6} checkpoints={} max_rk4_steps={maximum_steps} query_us_min_median_max={:?}",
            preparation.as_secs_f64() * 1000.,
            source.checkpoint_count(),
            [times[0], times[5], times[10]]
        );
    }
}
