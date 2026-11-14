//! Opt-in M4F experiment. Every mechanical equation is imported unchanged.
#![cfg(feature = "experimental-source-guidance")]
#[allow(dead_code)]
#[path = "support/m4b_contract.rs"]
mod contract;
#[path = "support/m4g_extended.rs"]
mod extended;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/m4e_c_extension.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[allow(dead_code)]
#[path = "support/m4a_matrix.rs"]
mod matrix;
#[allow(dead_code)]
#[path = "support/m4a_mechanisms.rs"]
mod mechanisms;
#[allow(dead_code)]
#[path = "support/m4b_dynamics.rs"]
mod original_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[allow(dead_code)]
#[path = "support/m4d_dynamics.rs"]
mod refined_dynamics;
#[path = "support/m4f_surface.rs"]
mod surface;
#[allow(dead_code)]
#[path = "support/m4d_tracking.rs"]
mod tracking;
#[path = "support/m4f_visualization.rs"]
mod visualization;
use contract::{Kind, Model};
use matrix::{finite_jacobian, max_abs, multiply};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    heartline::AuthoredReference,
    math::Vec3,
    solver::{
        GenerationOptions,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
use surface::{Rail, Role, Surface};
const ID: SourceIdentity = SourceIdentity {
    document: 0x4fb,
    generation: 1,
};
fn rail(initial: &InitialConditions, sections: &[Section]) -> Rail {
    Rail::new(
        ContinuousGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            initial,
            sections,
            GenerationOptions::default(),
        )
        .unwrap(),
        ID,
    )
    .unwrap()
}
fn cases() -> Vec<(&'static str, InitialConditions, Vec<Section>)> {
    let mut cases = fixtures::fixtures();
    let p = qx_document::sample::foundation_project();
    cases.push(("foundation", p.initial, p.sections[..2].to_vec()));
    cases
}
fn model(r: &Rail, lead: f64, shared: bool) -> Model {
    // Reuse only M4B's declared mechanical topology and local dimensions.
    // Seed EVERY body from this source, never from the synthetic path geometry.
    let mut mechanism = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        shared,
    );
    for p in &mut mechanism.poses {
        let local = Vec3::new(0., p.position_m.y, p.position_m.z);
        let e = r.source.evaluate(ID, p.position_m.x, Side::Right).unwrap();
        p.position_m = e.position_m + e.frame.local_to_world(local);
        p.orientation = e.frame.orientation();
    }
    Model::new(mechanism)
}
fn error(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0., f64::max)
}
fn array(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

#[test]
fn source_datum_normals_and_checkpoint_geometry() {
    for (name, initial, sections) in cases() {
        let r = rail(&initial, &sections);
        let indices = [
            0,
            r.source.segments().len() / 2,
            r.source.segments().len() - 1,
        ];
        for index in indices {
            let c = &r.source.segments()[index];
            let s = (c.start_station_m + c.end_station_m) * 0.5;
            for role in [Role::Running, Role::Side] {
                let surface = Surface {
                    rail: "right",
                    face: "explicit-development-datum",
                    offset: Vec3::new(0.1, -0.08, 0.6),
                    role,
                };
                let d = r.differential(surface, s).unwrap();
                let j = d.jet;
                assert!(j.n.dot(j.d).abs() < 1e-12 && (j.n.length() - 1.).abs() < 1e-12);
                for h in [1e-3, 1e-4] {
                    let a = r.differential(surface, s - h).unwrap();
                    let b = r.differential(surface, s + h).unwrap();
                    assert!(((b.jet.p - a.jet.p) / (2. * h) - j.d).length() < 2e-8);
                    assert!(((b.jet.d - a.jet.d) / (2. * h) - j.dd).length() < 2e-8);
                    assert!(((b.jet.dd - a.jet.dd) / (2. * h) - d.ddd).length() < 2e-8);
                    assert!(((b.jet.n - a.jet.n) / (2. * h) - j.dn).length() < 2e-8);
                    assert!(((b.jet.dn - a.jet.dn) / (2. * h) - d.ddn).length() < 2e-8);
                }
                let q: Vec<_> = c
                    .quaternion_coefficients
                    .iter()
                    .map(|q| [q.w, q.x, q.y, q.z])
                    .collect();
                println!(
                    "M4F_GEOMETRY {{\"name\":\"{name}\",\"role\":\"{role:?}\",\"s\":{s},\"offset\":{:?},\"domain\":{:?},\"start\":{:?},\"q\":{:?},\"p\":{:?},\"d\":{:?},\"dd\":{:?},\"ddd\":{:?},\"n\":{:?},\"dn\":{:?},\"ddn\":{:?}}}",
                    array(surface.offset),
                    [c.start_station_m, c.end_station_m, c.nominal_step_m],
                    array(c.start_position_m),
                    q,
                    array(j.p),
                    array(j.d),
                    array(j.dd),
                    array(d.ddd),
                    array(j.n),
                    array(j.dn),
                    array(d.ddn)
                );
            }
        }
    }
}

#[test]
fn source_mechanical_qualification() {
    for (name, initial, sections) in cases().into_iter().filter(|c| c.0 != "rolling") {
        let r = rail(&initial, &sections);
        for shared in [false, true] {
            let lead = if r.length() > 25. { 20.425 } else { 8.025 };
            let m = model(&r, lead, shared);
            let (state, _) = original_dynamics::initial(&m, &r, 6.)
                .unwrap_or_else(|e| panic!("{name} shared={shared}: {e:?}"));
            let e = m.evaluate(&r, &state.poses, Kind::B1).unwrap();
            assert!(max_abs(&e.c) < 2e-11 && max_abs(&multiply(&e.j, &state.velocity)) < 1e-9);
            for displaced in [false, true] {
                let p = if displaced {
                    m.shifted(
                        &state.poses,
                        &(0..state.velocity.len())
                            .map(|i| (i as f64 * 1.7).sin() * 0.002)
                            .collect::<Vec<_>>(),
                    )
                } else {
                    state.poses.clone()
                };
                let e = m.evaluate(&r, &p, Kind::B1).unwrap();
                let finite = finite_jacobian(state.velocity.len(), 1e-6, |d| {
                    m.evaluate(&r, &m.shifted(&p, d), Kind::B1).unwrap().c
                });
                let jac = error(&e.j.concat(), &finite.concat());
                assert!(jac < 3e-7, "{name} J={jac}");
                let gamma = m
                    .curvature(&r, &p, &state.velocity, Kind::B1, 1e-5)
                    .unwrap();
                let refined = m
                    .curvature(&r, &p, &state.velocity, Kind::B1, 5e-6)
                    .unwrap();
                let dot = error(&gamma, &refined);
                assert!(dot < 1e-6, "{name} Jdot={dot}");
                let h = 5e-5;
                let at = |sign: f64| {
                    m.evaluate(
                        &r,
                        &m.shifted(
                            &p,
                            &state
                                .velocity
                                .iter()
                                .map(|v| v * h * sign)
                                .collect::<Vec<_>>(),
                        ),
                        Kind::B1,
                    )
                    .unwrap()
                    .c
                };
                let second: Vec<_> = at(1.)
                    .iter()
                    .zip(at(-1.))
                    .zip(e.c)
                    .map(|((a, b), c)| (a + b - 2. * c) / (h * h))
                    .collect();
                let acceleration = error(&gamma, &second);
                assert!(acceleration < 2e-5, "{name} second={acceleration}");
                println!("M4F_DIFFERENTIAL {name} {shared} {displaced} {jac} {dot} {acceleration}");
            }
            let gamma = m
                .curvature(&r, &state.poses, &state.velocity, Kind::B1, 1e-5)
                .unwrap();
            let (acceleration, loads) =
                original_dynamics::instantaneous(&m, &r, &state, Vec3::new(0., -9.81, 0.)).unwrap();
            assert!(contract::work(&e.j, &state.velocity, &loads).abs() < 1e-7);
            println!(
                "M4F_SYSTEM {{\"name\":\"{name}-{shared}\",\"j\":{:?},\"v\":{:?},\"gamma\":{:?},\"loads\":{:?},\"acceleration\":{:?}}}",
                e.j, state.velocity, gamma, loads, acceleration
            );
        }
    }
}

#[test]
fn short_source_dynamics_replay_energy_and_rollback() {
    use std::time::Instant;
    for (name, initial, sections) in cases().into_iter().filter(|c| c.0 != "rolling") {
        let r = rail(&initial, &sections);
        let lead = if r.length() > 25. { 20.425 } else { 8.025 };
        for shared in [false, true] {
            let mut t = tracking::Tracked::new(model(&r, lead, shared), &r, 6.)
                .unwrap_or_else(|e| panic!("{name} {e:?}"));
            let mut repeat = tracking::Tracked::new(model(&r, lead, shared), &r, 6.).unwrap();
            let gravity = Vec3::new(0., -9.81, 0.);
            let start =
                original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, gravity);
            let mut max_energy = 0_f64;
            let mut max_p = 0_f64;
            let mut max_v = 0_f64;
            let clock = Instant::now();
            for tick in 0..24 {
                let report = t
                    .step(&r, gravity)
                    .unwrap_or_else(|e| panic!("{name} shared={shared} tick={tick}: {e:?}"));
                assert_eq!(report, repeat.step(&r, gravity).unwrap());
                assert_eq!(t.state, repeat.state);
                assert_eq!(t.hints(), repeat.hints());
                assert_eq!(t.envelopes, repeat.envelopes);
                assert!(report.max_position < 2e-5 && report.max_velocity < 0.003);
                max_p = max_p.max(report.max_position);
                max_v = max_v.max(report.max_velocity);
                max_energy = max_energy.max((report.energy - start).abs());
            }
            println!(
                "M4F_DYNAMICS {name} {shared} ticks={} p={max_p} v={max_v} energy_drift_J={max_energy} paired_elapsed_ms={}",
                t.ticks,
                clock.elapsed().as_secs_f64() * 1000.
            );
            let state = t.state.clone();
            let hints = t.hints();
            let envelopes = t.envelopes.clone();
            let windows: Vec<_> = t.model.supports.iter().map(|s| s.half_window).collect();
            t.substeps = 3;
            assert_eq!(t.step(&r, gravity).unwrap_err(), surface::Failure::Invalid);
            assert_eq!(t.state, state);
            assert_eq!(t.hints(), hints);
            assert_eq!(t.envelopes, envelopes);
            assert_eq!(t.ticks, 24);
            assert_eq!(
                t.model
                    .supports
                    .iter()
                    .map(|s| s.half_window)
                    .collect::<Vec<_>>(),
                windows
            );
        }
    }
}

#[test]
fn rolling_source_is_valid_but_this_mechanism_has_no_qualified_seating() {
    let (_, initial, sections) = fixtures::fixtures().remove(3);
    let r = rail(&initial, &sections);
    // Source and numerical checkpoints are accepted. High twist over a 2.4 m
    // car is deliberately outside the admitted short-run cases above.
    let m = model(&r, 8.025, false);
    let before = m.poses.clone();
    let result = original_dynamics::initial(&m, &r, 6.);
    println!(
        "M4F_ROLLING_MECHANISM {:?}",
        result.as_ref().map(|_| ()).map_err(|e| *e)
    );
    assert_eq!(result.unwrap_err(), surface::Failure::NoConvergence);
    assert_eq!(m.poses, before);
}

#[test]
fn compatible_source_join_is_traversed_without_a_numerical_boundary_guard() {
    let p = qx_document::sample::foundation_project();
    let r = rail(&p.initial, &p.sections[..2]);
    let join = r
        .source
        .segments()
        .iter()
        .find(|c| c.chart == 1)
        .unwrap()
        .start_station_m;
    for shared in [false, true] {
        let mut t = tracking::Tracked::new(model(&r, 11.625, shared), &r, 6.).unwrap();
        let start = t.hints()[0];
        assert!(start < join);
        for _ in 0..24 {
            t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap();
        }
        assert!(t.hints()[0] > join);
        println!("M4F_JOIN {shared} {join} {start} {}", t.hints()[0]);
    }
}
