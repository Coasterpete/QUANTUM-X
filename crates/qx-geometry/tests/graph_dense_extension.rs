#![cfg(feature = "experimental-graph-source")]
#[allow(dead_code)]
#[path = "support/m4e_c_extension.rs"]
mod retained;
use qx_geometry::{
    authoring::{InitialConditions, Section},
    heartline::AuthoredReference,
    math::{Frame, Quaternion, Vec3},
    solver::{
        GenerationOptions,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::{Channel, Side, Unit},
};
const ID: SourceIdentity = SourceIdentity {
    document: 0x4f,
    generation: 1,
};
fn prepare(initial: &InitialConditions, sections: &[Section], step: f64) -> ContinuousGraph {
    ContinuousGraph::prepare(
        ID,
        AuthoredReference::TrackCenterline,
        initial,
        sections,
        GenerationOptions {
            max_step_m: step,
            ..Default::default()
        },
    )
    .unwrap()
}
fn values(v: impl IntoIterator<Item = Vec3>) -> Vec<f64> {
    v.into_iter().flat_map(|v| [v.x, v.y, v.z]).collect()
}
fn frame_jets(frame: Frame, k: [Vec3; 3]) -> [[Vec3; 3]; 3] {
    retained::frame_derivatives(frame, k)
}
#[test]
fn dense_contract_and_inspectable_polynomials() {
    for (name, initial, sections) in retained::fixtures() {
        measure(name, &initial, &sections);
    }
}
pub fn measure(name: &str, initial: &InitialConditions, sections: &[Section]) {
    for step in [0.2, 0.1, 0.05] {
        let g = prepare(initial, sections, step);
        g.validate_interval(ID, 0., g.length_m()).unwrap();
        let cells = g.segments();
        let mut indices = vec![0, 1, cells.len() / 2, cells.len() - 2, cells.len() - 1];
        for i in 1..cells.len() {
            if cells[i].chart != cells[i - 1].chart {
                indices.extend([i - 1, i]);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        for index in indices {
            let c = &cells[index];
            let q: Vec<_> = c
                .quaternion_coefficients
                .iter()
                .map(|q| [q.w, q.x, q.y, q.z])
                .collect();
            println!(
                "M4F_CELL {name} {step} {index} {:?}",
                (
                    vec![c.start_station_m, c.end_station_m, c.nominal_step_m],
                    values([c.start_position_m]),
                    q
                )
            );
            for f in [0., 0.173, 0.5, 0.893, 1.] {
                let s = if f == 0. {
                    c.start_station_m
                } else if f == 1. {
                    c.end_station_m
                } else {
                    c.start_station_m + (c.end_station_m - c.start_station_m) * f
                };
                let side = if f == 1. { Side::Left } else { Side::Right };
                let e = g.evaluate(ID, s, side).unwrap();
                let r = frame_jets(e.frame, e.angular_derivatives);
                println!(
                    "M4F_QUERY {name} {step} {index} {f} {s} {:?}",
                    (
                        values([
                            e.position_m,
                            e.frame.tangent(),
                            e.frame.normal(),
                            e.frame.lateral()
                        ]),
                        values(e.position_derivatives),
                        r.map(values).concat(),
                        values(e.angular_derivatives)
                    )
                );
                assert!((e.position_derivatives[0] - e.frame.tangent()).length() < 1e-14);
                assert!(e.local_rk4_steps == 0);
                assert!(e.position_m.is_finite());
            }
        }
        let mut maxima = [0_f64; 6];
        for c in cells.iter().take(cells.len() - 1) {
            let a = g.evaluate(ID, c.end_station_m, Side::Left).unwrap();
            let b = g.evaluate(ID, c.end_station_m, Side::Right).unwrap();
            let ra = frame_jets(a.frame, a.angular_derivatives);
            let rb = frame_jets(b.frame, b.angular_derivatives);
            for j in 0..3 {
                maxima[j] =
                    maxima[j].max((a.position_derivatives[j] - b.position_derivatives[j]).length());
                for axis in 0..3 {
                    maxima[j + 3] = maxima[j + 3].max((ra[axis][j] - rb[axis][j]).length());
                }
            }
            assert!((a.position_m - b.position_m).length() < 2e-12);
        }
        println!("M4F_LIMITS {name} {step} {maxima:?}");
        // Roundoff-level polynomial endpoint matching, not the former
        // h^4/h^3/h^2 restart defect. Independent Decimal checks follow.
        assert!(
            maxima[0] < 2e-12 && maxima[1] < 2e-10 && maxima[2] < 2e-8,
            "{maxima:?}"
        );
        assert!(
            maxima[3] < 2e-10 && maxima[4] < 2e-8 && maxima[5] < 2e-6,
            "{maxima:?}"
        );
        println!(
            "M4F_DRIFT {name} {step} length={:.12e}",
            g.length_m() - g.source().length_m()
        );
    }
}
#[test]
fn rolling_restart_is_resolved_without_rejection() {
    let (_, initial, sections) = retained::fixtures().remove(3);
    let old = retained::prepare(&initial, &sections, 0.2);
    assert!(old.boundaries().is_empty());
    let g = prepare(&initial, &sections, 0.2);
    let checkpoint = g.segments()[0].end_station_m;
    g.validate_interval(ID, checkpoint - 0.01, checkpoint + 0.01)
        .unwrap();
    for half in [6e-5, 3e-5, 1.5e-5] {
        let offset = Vec3::new(0.1, -0.08, 0.6);
        let e = g.evaluate(ID, checkpoint, Side::Right).unwrap();
        let at = |s| {
            g.evaluate(ID, s, Side::Right)
                .unwrap()
                .constant_offset(offset)
                .unwrap()[0]
        };
        let observed = (at(checkpoint - half) - at(checkpoint) * 2. + at(checkpoint + half))
            / (half * half)
            * 36.;
        let expected = e.constant_offset(offset).unwrap()[2] * 36.;
        println!(
            "M4F_ROLL_ACCEL {half} {:.12e}",
            (observed - expected).length()
        );
        // f64 second differences are reported too; Decimal resolves the limit.
    }
}
#[test]
fn source_convergence_and_compatible_join() {
    let (_, initial, sections) = retained::fixtures().remove(2);
    let k = Frame::new(initial.orientation)
        .unwrap()
        .local_to_world(Vec3::new(0.04, 0., 0.125));
    let w = k.length();
    let axis = k / w;
    let t = Frame::new(initial.orientation).unwrap().tangent();
    let mut errors = Vec::new();
    for step in [1., 0.5, 0.25] {
        let g = prepare(&initial, &sections, step);
        let s = g.length_m();
        let e = g.evaluate(ID, s, Side::Left).unwrap();
        let parallel = axis * t.dot(axis);
        let perp = t - parallel;
        let exact = initial.position_m
            + parallel * s
            + perp * ((w * s).sin() / w)
            + axis.cross(t) * ((1. - (w * s).cos()) / w);
        let rot = Quaternion::axis_angle(axis, w * s)
            .unwrap()
            .compose(initial.orientation);
        let error = (e.position_m - exact).length();
        println!(
            "M4F_CONVERGENCE {step} {error:.12e} {:.12e}",
            e.frame
                .orientation()
                .compose(rot.conjugate())
                .rotation_vector()
                .unwrap()
                .length()
        );
        errors.push(error);
    }
    assert!(
        errors[0] / errors[1] > 12. && errors[1] / errors[2] > 12.,
        "{errors:?}"
    );
    let mut a = Section::straight(1, "left", 2.);
    a.roll_rate = Channel::constant(Unit::RadiansPerMetre, 2., 0.4);
    let mut b = a.clone();
    b.id.0 = 2;
    let g = prepare(&initial, &[a.clone(), b.clone()], 0.1);
    g.validate_interval(ID, 0., g.length_m()).unwrap();
    let join = g
        .segments()
        .iter()
        .find(|c| c.chart == 1)
        .unwrap()
        .start_station_m;
    let l = g.evaluate(ID, join, Side::Left).unwrap();
    let r = g.evaluate(ID, join, Side::Right).unwrap();
    assert_eq!(l.chart, 0);
    assert_eq!(r.chart, 1);
    for j in 0..3 {
        assert!((l.angular_derivatives[j] - r.angular_derivatives[j]).length() < 1e-7);
    }
    b.roll_rate = Channel::constant(Unit::RadiansPerMetre, 2., 0.2);
    let bad = prepare(&initial, &[a, b], 0.1);
    assert!(bad.validate_interval(ID, 0., bad.length_m()).is_err());
    let mut tiny = Section::straight(1, "left", 2.);
    tiny.roll_rate = Channel::constant(Unit::RadiansPerMetre, 2., 0.4);
    let mut next = tiny.clone();
    next.id.0 = 2;
    next.roll_rate = Channel::constant(Unit::RadiansPerMetre, 2., 0.4 + 5e-10);
    let tiny_jump = prepare(&initial, &[tiny, next], 0.1);
    assert!(tiny_jump.source().boundaries()[0].compatible);
    assert!(
        tiny_jump
            .validate_interval(ID, 0., tiny_jump.length_m())
            .is_err()
    );
    for s in [-1., f64::NAN, f64::INFINITY] {
        assert!(g.evaluate(ID, s, Side::Right).is_err());
    }
    assert!(
        g.evaluate(
            SourceIdentity {
                generation: 2,
                ..ID
            },
            1.,
            Side::Right
        )
        .is_err()
    );
    assert!(
        ContinuousGraph::prepare(
            ID,
            AuthoredReference::RiderReference,
            &initial,
            &sections,
            GenerationOptions::default()
        )
        .is_err()
    );
}

#[test]
fn varying_source_refinement_measures_all_pose_orders() {
    for (name, initial, sections) in retained::fixtures().into_iter().skip(4) {
        let reference = prepare(&initial, &sections, 0.025);
        let mut records = Vec::new();
        for step in [0.8, 0.4, 0.2] {
            let g = prepare(&initial, &sections, step);
            let mut error = [0_f64; 7];
            for s in [0.137, 5.137, 12.137, 24.137, 29.137] {
                let a = g.evaluate(ID, s, Side::Right).unwrap();
                let b = reference.evaluate(ID, s, Side::Right).unwrap();
                let aa = frame_jets(a.frame, a.angular_derivatives);
                let bb = frame_jets(b.frame, b.angular_derivatives);
                error[0] = error[0].max((a.position_m - b.position_m).length());
                for j in 0..3 {
                    error[1 + j] = error[1 + j]
                        .max((a.position_derivatives[j] - b.position_derivatives[j]).length());
                    for axis in 0..3 {
                        error[4 + j] = error[4 + j].max((aa[axis][j] - bb[axis][j]).length());
                    }
                }
            }
            println!("M4F_VARYING_CONVERGENCE {name} {step} {error:?}");
            records.push(error);
        }
        for i in 0..2 {
            assert!(records[i][0] / records[i + 1][0] > 10., "{records:?}");
            for j in 1..7 {
                assert!(records[i + 1][j] < records[i][j], "{name}: {records:?}");
            }
        }
    }
}
