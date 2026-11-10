#![cfg(feature = "experimental-graph-source")]
#[path = "support/m4e_c_extension.rs"]
mod extension;
use extension::*;
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    curve::{ControlPoint, Curve},
    heartline::AuthoredReference,
    math::Vec3,
    solver::prepared::{PreparedGraph, SourceIdentity},
    transition::{Channel, Function, Side, Transition, Unit},
};

#[test]
fn poses_and_source_jets_are_exported_at_actual_query_boundaries() {
    for (name, initial, sections) in fixtures() {
        measure(name, &initial, &sections, &[0.2, 0.1, 0.05]);
    }
}

#[test]
fn coarse_local_rk4_derivatives_have_a_resolved_defect_and_refine() {
    let (_, initial, sections) = fixtures().remove(3);
    let mut defects = Vec::new();
    for step in [0.3, 0.15, 0.075] {
        let source = prepare(&initial, &sections, step);
        assert!(source.boundaries().is_empty());
        let h = step * 0.5;
        let s = h * 0.75;
        let delta = h * 0.02;
        let e = source.evaluate(ID, s, Side::Right).unwrap();
        let f = |i: f64| {
            source
                .evaluate(ID, s + i * delta, Side::Right)
                .unwrap()
                .frame
                .normal()
        };
        // Sixth-order centered first difference of the actual numerical query.
        let observed = (f(-3.) * -1. + f(-2.) * 9. - f(-1.) * 45. + f(1.) * 45. - f(2.) * 9.
            + f(3.))
            / (60. * delta);
        let expected = e.angular_derivatives[0].cross(e.frame.normal());
        let defect = (observed - expected).length();
        println!("M4EC_RESOLVED step={step} first_frame_defect={defect:.12e}");
        defects.push(defect);
    }
    assert!(defects[0] > 1e-10);
    assert!(
        defects[0] / defects[1] > 12. && defects[1] / defects[2] > 10.,
        "{defects:?}"
    );
}

#[test]
#[ignore = "serial Release upstream query cost; no adapter/physics cost claim"]
fn upstream_query_cost_includes_exact_identity_and_interval_checks() {
    use std::{hint::black_box, time::Instant};
    for (name, initial, sections) in fixtures() {
        let source = prepare(&initial, &sections, 0.1);
        let query = |s| {
            assert!(source.matches_source(black_box(&initial), black_box(&sections), options(0.1)));
            source
                .validate_interval(black_box(ID), s - 0.01, s + 0.01)
                .unwrap();
            let e = source.evaluate(black_box(ID), s, Side::Right).unwrap();
            black_box(
                e.constant_offset(black_box(Vec3::new(0.1, -0.08, 0.6)))
                    .unwrap(),
            );
            black_box(e);
        };
        for i in 0..1000 {
            query(0.1 + (source.length_m() - 0.2) * (i as f64 + 0.137) / 1000.);
        }
        let mut times = Vec::new();
        for _ in 0..11 {
            let start = Instant::now();
            for i in 0..1000 {
                query(0.1 + (source.length_m() - 0.2) * (i as f64 + 0.137) / 1000.);
            }
            times.push(start.elapsed().as_secs_f64() * 1e6 / 1000.);
        }
        times.sort_by(f64::total_cmp);
        println!(
            "M4EC_UPSTREAM_COST {name} us_min_median_max={:?}",
            [times[0], times[5], times[10]]
        );
    }
}

#[test]
fn rejected_charts_domains_and_stale_sources_remain_rejected() {
    let initial = InitialConditions::default();
    for order in 0..3 {
        let mut a = Section::straight(1, "Left source", 5.);
        let mut b = Section::straight(2, "Right source", 5.);
        let (start, end, next, function) = match order {
            0 => (0., 0., 0.02, Function::Linear {}),
            1 => (0., 0.02, 0.02, Function::Linear {}),
            _ => (0., 0.02, 0.06, Function::Quadratic {}),
        };
        a.roll_rate = Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![Transition {
                id: 1,
                length: 5.,
                start,
                end,
                amplitude: 0.,
                function,
            }],
        };
        b.roll_rate = Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![Transition {
                id: 1,
                length: 5.,
                start: if order == 0 { next } else { end },
                end: next,
                amplitude: 0.,
                function: Function::Linear {},
            }],
        };
        let source = prepare(&initial, &[a, b], 0.1);
        assert!(!source.boundaries()[0].compatible);
        for (a, b) in [(4., 6.), (6., 4.), (5., 6.), (4., 5.)] {
            assert!(source.validate_interval(ID, a, b).is_err());
        }
        assert_eq!(source.evaluate(ID, 5., Side::Left).unwrap().chart, 0);
        assert_eq!(source.evaluate(ID, 5., Side::Right).unwrap().chart, 1);
    }
    let line = Section::straight(1, "Open", 10.);
    let source = prepare(&initial, std::slice::from_ref(&line), 0.1);
    for id in [
        SourceIdentity { document: 9, ..ID },
        SourceIdentity {
            generation: 2,
            ..ID
        },
    ] {
        assert!(
            source
                .evaluate(id, 2., Side::Right)
                .unwrap_err()
                .0
                .contains("stale")
        );
        assert!(source.validate_interval(id, 1., 2.).is_err());
    }
    let mut replacement = line.clone();
    replacement.id.0 = 9;
    assert!(!source.matches_source(&initial, &[replacement], options(0.1)));
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
            std::slice::from_ref(&line),
            options(0.1)
        )
        .is_err()
    );
    let mut world = line.clone();
    world.kind = SectionKind::Geometric {
        pitch_rate: line.roll_rate.clone(),
        yaw_rate: line.roll_rate.clone(),
    };
    let native = Section::spline(
        2,
        "Native unsupported",
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
    for section in [world, native] {
        assert!(
            PreparedGraph::prepare(
                ID,
                AuthoredReference::TrackCenterline,
                &initial,
                &[section],
                options(0.1)
            )
            .is_err()
        );
    }
    let mut circle = line;
    circle.kind = SectionKind::Arc {
        radius_m: 10.,
        plane_angle_rad: 0.,
    };
    let circle = prepare(&initial, &[circle], 0.1);
    assert!(
        circle
            .evaluate(ID, 0., Side::Right)
            .unwrap()
            .constant_offset(Vec3::Y * 10.)
            .is_err()
    );
    // A regular datum tangent alone does not define a running normal. This
    // source/offset gives P' = .32 RZ, so RZ cross P' cannot be normalized.
    // This is only a degeneracy measurement, not a usable normal adapter.
    let (_, banked, helix) = fixtures().remove(2);
    let source = prepare(&banked, &helix, 0.1);
    let e = source.evaluate(ID, 0., Side::Right).unwrap();
    let datum = e.constant_offset(Vec3::Y * 8.).unwrap();
    assert!(datum[1].length() > 0.3);
    assert!(e.frame.lateral().cross(datum[1]).normalized().is_err());
    // No projector is constructed after the failed differential qualification.
    // Explicit ambiguous-projection failures remain in the unchanged M4C/M4D
    // controls, not a claim about a nonexistent source projector.
}
