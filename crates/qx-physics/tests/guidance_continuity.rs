//! M4D continuity/continuation research. No production APIs select this adapter.
#[allow(dead_code)]
#[path = "support/articulation.rs"]
mod articulation;
#[allow(dead_code)]
#[path = "support/m4b_contract.rs"]
mod contract;
#[allow(dead_code)]
#[path = "support/m4c_dynamics.rs"]
mod dynamics;
#[allow(dead_code)]
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[allow(dead_code)]
#[path = "support/m4c_tracking.rs"]
mod legacy_tracking;
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
#[path = "support/m4d_reference.rs"]
mod reference;
#[allow(dead_code)]
#[path = "support/m4d_dynamics.rs"]
mod refined_dynamics;
#[allow(dead_code)]
#[path = "support/m4d_surface.rs"]
mod surface;
#[path = "support/m4d_tracking.rs"]
mod tracking;

#[path = "support/m4d_characterization.rs"]
mod characterization;
use characterization::run;
use contract::{Kind, Model};
use matrix::{finite_jacobian, max_abs, multiply};
use mechanisms::{Mechanism, Path};
use qx_geometry::{
    math::{Frame, Vec3},
    solver::GeneratedTrack,
};
use surface::{Failure, Rail, Role, Surface};
use tracking::Tracked;
const PATHS: [Path; 6] = [
    Path::Straight,
    Path::Turn,
    Path::Bank,
    Path::Helix,
    Path::Loop,
    Path::Inverted,
];
fn sampled(path: Path) -> GeneratedTrack {
    rail_fixtures::sampled(60., 0.025, |s| {
        let p = path.at(s);
        (p.position_m, p.orientation)
    })
}
fn model(path: Path, lead: f64, shared: bool) -> Model {
    Model::new(Mechanism::new(
        path,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        shared,
    ))
}
fn support(role: Role) -> Surface {
    Surface {
        rail: "right",
        face: "explicit-research-datum",
        offset: Vec3::new(0., -0.08, 0.6),
        role,
    }
}
fn error(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0., f64::max)
}
fn explicit(path: Path, s: f64, offset: Vec3) -> Vec3 {
    path.locus(s, offset.y, offset.z)
        + Frame::new(path.at(s).orientation).unwrap().tangent() * offset.x
}

#[test]
fn actual_sampled_one_sided_continuity_audit() {
    for path in PATHS {
        let track = sampled(path);
        let mut worst = [0_f64; 12];
        for index in 1..track.samples.len() - 1 {
            let (ca, cb) = surface::sides(
                &track,
                index,
                Surface {
                    offset: Vec3::ZERO,
                    ..support(Role::Running)
                },
            )
            .unwrap();
            let dk = |piece: usize, k: Vec3, sign: f64| {
                let a = &track.samples[piece];
                let b = &track.samples[piece + 1];
                let angle = b
                    .frame
                    .orientation()
                    .compose(a.frame.orientation().conjugate())
                    .rotation_vector()
                    .unwrap()
                    .length();
                let dot = (angle * 0.5).cos();
                if dot > 0.9995 {
                    k * (sign * 2. * (1. - dot) / (b.distance_m - a.distance_m))
                } else {
                    Vec3::ZERO
                }
            };
            for role in [Role::Running, Role::Side] {
                let (a, b) = surface::sides(&track, index, support(role)).unwrap();
                let angle = a
                    .frame
                    .orientation()
                    .conjugate()
                    .compose(b.frame.orientation())
                    .rotation_vector()
                    .unwrap()
                    .length();
                for (max, value) in worst.iter_mut().zip([
                    (a.p - b.p).length(),
                    (a.d / a.d.length() - b.d / b.d.length()).length(),
                    angle,
                    (a.k - b.k).length(),
                    (a.d - b.d).length(),
                    (a.dd - b.dd).length(),
                    (a.n - b.n).length(),
                    (a.dn - b.dn).length(),
                    (a.frame.tangent() - a.d / a.d.length()).length(),
                    (ca.d - cb.d).length(),
                    (ca.dd - cb.dd).length(),
                    (dk(index - 1, a.k, -1.) - dk(index, b.k, 1.)).length(),
                ]) {
                    *max = max.max(value);
                }
            }
        }
        assert!(worst[0] < 1e-11 && worst[2] < 1e-12 && worst[9] < 1e-12);
        if matches!(path, Path::Bank) {
            assert!(worst[3] > 1e-5 && worst[4] > 1e-5);
        }
        println!(
            "M4D_SAMPLED {path:?} p_t_frame_k_d_dd_n_dn_frame_vs_d_center_d_center_dd_dk={worst:?}"
        );
    }
    let track = sampled(Path::Bank);
    let (a, b) = surface::sides(&track, 817, support(Role::Running)).unwrap();
    assert!((track.samples[817].distance_m - 20.425).abs() < 1e-12);
    assert!((a.d - b.d).length() > 1e-5);
    // Also inspect actual solver output, not just analytically sampled fixtures.
    for (name, track) in [
        ("generated-straight", articulation::straight()),
        ("generated-arc", articulation::arc(12., 0.6, 80., false)),
        ("generated-bank", articulation::arc(12., 0.6, 80., true)),
    ] {
        let before = track.clone();
        let mut jumps = [0_f64; 5];
        for i in 1..track.samples.len() - 1 {
            let (a, b) = surface::sides(&track, i, support(Role::Side)).unwrap();
            for (max, value) in jumps.iter_mut().zip([
                (a.p - b.p).length(),
                (a.d - b.d).length(),
                (a.dd - b.dd).length(),
                (a.k - b.k).length(),
                (a.dn - b.dn).length(),
            ]) {
                *max = max.max(value);
            }
        }
        assert_eq!(track, before);
        println!("M4D_GENERATED {name} p_d_dd_k_dn={jumps:?}");
    }
}

#[test]
fn authored_abrupt_rates_and_invalid_seams_are_not_repaired() {
    use qx_geometry::math::Quaternion;
    let track = rail_fixtures::sampled(40., 0.025, |s| {
        let bank = if s <= 20. { 0. } else { 0.1 * (s - 20.) };
        (Vec3::X * s, Quaternion::axis_angle(Vec3::X, bank).unwrap())
    });
    let before = track.clone();
    let (a, b) = surface::sides(&track, 800, support(Role::Running)).unwrap();
    assert!((a.p - b.p).length() < 1e-14);
    assert!((a.k - b.k).length() > 0.09);
    assert!((a.d - b.d).length() > 0.05);
    assert!((a.n - b.n).length() > 0.001);
    assert_eq!(track, before);
    let length = 16. * std::f64::consts::PI;
    for frame_gap in [false, true] {
        let mut track = rail_fixtures::sampled(length, 0.025, |s| {
            let p = Path::Turn.at(s);
            (p.position_m, p.orientation)
        });
        let end = track.samples.last_mut().unwrap();
        if frame_gap {
            end.frame = Frame::new(
                end.frame
                    .orientation()
                    .compose(Quaternion::axis_angle(Vec3::X, 0.0005).unwrap()),
            )
            .unwrap();
        } else {
            end.position_m.y += 0.0005;
        }
        let before = track.clone();
        assert!(matches!(
            Rail::sampled(track.clone(), true),
            Err(Failure::Corner)
        ));
        assert_eq!(track, before);
    }
}

#[test]
fn source_reference_derivatives_boundaries_offsets_and_closed_seams() {
    for path in PATHS {
        for role in [Role::Running, Role::Side] {
            for offset in [
                Vec3::ZERO,
                Vec3::new(0., -0.08, 0.6),
                Vec3::new(0.17, 0.11, -0.6),
            ] {
                let surface = Surface {
                    offset,
                    ..support(role)
                };
                for s in [0.025, 9.0137, 20., 20.425, 25.132741228718345, 50.] {
                    let d = reference::differential(path, surface, s).unwrap();
                    let j = d.jet;
                    assert!((j.p - explicit(path, s, offset)).length() < 2e-14);
                    for h in [1e-3, 1e-4] {
                        let a = reference::differential(path, surface, s - h).unwrap();
                        let b = reference::differential(path, surface, s + h).unwrap();
                        let fd = (explicit(path, s + h, offset) - explicit(path, s - h, offset))
                            / (2. * h);
                        assert!((fd - j.d).length() < 2e-8);
                        assert!(((b.jet.d - a.jet.d) / (2. * h) - j.dd).length() < 2e-8);
                        assert!(((b.jet.dd - a.jet.dd) / (2. * h) - d.ddd).length() < 2e-8);
                        assert!(((b.jet.n - a.jet.n) / (2. * h) - j.dn).length() < 2e-8);
                        assert!(((b.jet.dn - a.jet.dn) / (2. * h) - d.ddn).length() < 2e-8);
                        assert!(((b.jet.k - a.jet.k) / (2. * h) - d.dk).length() < 2e-8);
                        // One-sided limits converge to the SAME analytic jet.
                        assert!((a.jet.d - j.d).length() < h * 2.);
                        assert!((b.jet.d - j.d).length() < h * 2.);
                    }
                    assert!(j.d.dot(j.n).abs() < 1e-12 && (j.n.length() - 1.).abs() < 1e-12);
                    println!(
                        "M4D_GEOMETRY {{\"name\":\"{path:?}\",\"s\":{s:?},\"role\":\"{role:?}\",\"offset\":{:?},\"p\":{:?},\"d\":{:?},\"dd\":{:?},\"ddd\":{:?},\"n\":{:?},\"dn\":{:?},\"ddn\":{:?},\"k\":{:?},\"dk\":{:?}}}",
                        [offset.x, offset.y, offset.z],
                        [j.p.x, j.p.y, j.p.z],
                        [j.d.x, j.d.y, j.d.z],
                        [j.dd.x, j.dd.y, j.dd.z],
                        [d.ddd.x, d.ddd.y, d.ddd.z],
                        [j.n.x, j.n.y, j.n.z],
                        [j.dn.x, j.dn.y, j.dn.z],
                        [d.ddn.x, d.ddn.y, d.ddn.z],
                        [j.k.x, j.k.y, j.k.z],
                        [d.dk.x, d.dk.y, d.dk.z]
                    );
                }
            }
        }
    }
    for path in [Path::Turn, Path::Loop] {
        let length = 16. * std::f64::consts::PI;
        let rail = Rail::analytic(path, length, true).unwrap();
        for role in [Role::Running, Role::Side] {
            let surface = support(role);
            let a = reference::differential(path, surface, 0.).unwrap();
            let b = reference::differential(path, surface, length).unwrap();
            for (a, b) in [
                (a.jet.p, b.jet.p),
                (a.jet.d, b.jet.d),
                (a.jet.dd, b.jet.dd),
                (a.ddd, b.ddd),
                (a.jet.n, b.jet.n),
                (a.jet.dn, b.jet.dn),
                (a.ddn, b.ddn),
                (a.jet.k, b.jet.k),
                (a.dk, b.dk),
            ] {
                assert!((a - b).length() < 2e-14);
            }
            for direction in [-1., 1.] {
                let mut hint = length - 0.01;
                for _ in 0..1100 {
                    let target = hint + direction * 0.05;
                    let x = rail.jet(surface, target).unwrap().p;
                    let s = rail.project(surface, x, hint, 0.1).unwrap().0;
                    assert!((s - target).abs() < 1e-10);
                    hint = s;
                }
            }
        }
    }
    assert!(matches!(
        Rail::analytic(Path::Bank, 60., true),
        Err(Failure::Corner)
    ));
}

#[test]
fn common_mechanical_contract_and_independent_matrix_evidence() {
    for path in PATHS {
        for (lead, shared) in [(20.425, false), (25.132741228718345, false), (20.425, true)] {
            let r = Rail::analytic(path, 120., false).unwrap();
            let m = model(path, lead, shared);
            let (state, _) = original_dynamics::initial(&m, &r, 6.).unwrap();
            let e = m.evaluate(&r, &state.poses, Kind::B1).unwrap();
            assert!(max_abs(&e.c) < 2e-11);
            assert!(max_abs(&multiply(&e.j, &state.velocity)) < 1e-9);
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
                assert!(error(&e.j.concat(), &finite.concat()) < 3e-7);
                let gamma = m
                    .curvature(&r, &p, &state.velocity, Kind::B1, 1e-5)
                    .unwrap();
                let refined = m
                    .curvature(&r, &p, &state.velocity, Kind::B1, 5e-6)
                    .unwrap();
                assert!(error(&gamma, &refined) < 1e-6);
                let h = 5e-5;
                let shift = |sign: f64| {
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
                let (a, b) = (shift(1.), shift(-1.));
                let second: Vec<_> = a
                    .iter()
                    .zip(b)
                    .zip(e.c)
                    .map(|((a, b), c)| (a + b - 2. * c) / (h * h))
                    .collect();
                assert!(
                    error(&gamma, &second) < 2e-5,
                    "{path:?} {lead} displaced={displaced} error={}",
                    error(&gamma, &second)
                );
            }
            let gamma = m
                .curvature(&r, &state.poses, &state.velocity, Kind::B1, 1e-5)
                .unwrap();
            let (acceleration, loads) =
                original_dynamics::instantaneous(&m, &r, &state, Vec3::new(0., -9.81, 0.)).unwrap();
            assert!(contract::work(&e.j, &state.velocity, &loads).abs() < 1e-7);
            if matches!(path, Path::Straight | Path::Inverted) {
                let running: f64 = m
                    .supports
                    .iter()
                    .zip(&loads)
                    .filter(|(s, _)| s.surface.role == Role::Running)
                    .map(|(_, l)| l)
                    .sum();
                let sign = if matches!(path, Path::Inverted) {
                    -1.
                } else {
                    1.
                };
                assert!((running - sign * 981. * m.poses.len() as f64).abs() < 1e-7);
            }
            for (i, s) in m.supports.iter().enumerate() {
                let p = state.poses[s.owner];
                let x = guidance::point(p, s.local);
                let (_, jet) = r.project(s.surface, x, s.station, s.half_window).unwrap();
                let k = s.owner * 6;
                let g = Vec3::new(e.j[i][k], e.j[i][k + 1], e.j[i][k + 2]);
                // Force is g*lambda, not unit datum-normal*lambda. Verify its
                // direction, magnitude and associated moment without the solver.
                assert!((g.dot(jet.n) - 1.).abs() < 1e-11);
                let torque = (x - p.position_m).cross(g) * loads[i];
                let row_torque = Vec3::new(e.j[i][k + 3], e.j[i][k + 4], e.j[i][k + 5]) * loads[i];
                assert!((torque - row_torque).length() < 1e-9);
                assert!(((g * loads[i]).length() - g.length() * loads[i].abs()).abs() < 1e-9);
            }
            println!(
                "M4D_SYSTEM {{\"name\":\"{path:?}-{lead}-{shared}\",\"j\":{:?},\"v\":{:?},\"gamma\":{:?},\"loads\":{:?},\"acceleration\":{:?}}}",
                e.j, state.velocity, gamma, loads, acceleration
            );
        }
    }
}

#[test]
fn former_corner_crossing_and_original_failure_are_both_retained() {
    let discontinuous = Rail::sampled(sampled(Path::Bank), false).unwrap();
    let mut t = Tracked::new(model(Path::Bank, 20.0137, false), &discontinuous, 6.).unwrap();
    for _ in 0..23 {
        t.step(&discontinuous, Vec3::new(0., -9.81, 0.)).unwrap();
    }
    let (before, hints) = (t.state.clone(), t.hints());
    for _ in 0..2 {
        assert_eq!(
            t.step(&discontinuous, Vec3::new(0., -9.81, 0.))
                .unwrap_err(),
            Failure::Corner
        );
        assert_eq!(t.state, before);
        assert_eq!(t.hints(), hints);
        assert_eq!(t.ticks, 23);
    }
    let corner = discontinuous
        .control
        .as_ref()
        .unwrap()
        .corner
        .get()
        .unwrap();
    assert!((corner.station - 20.425).abs() < 1e-10);
    assert!(corner.f_left > 0. && corner.f_right < 0.);
    println!("M4D_ORIGINAL_CORNER {corner:?}");
    run(
        Path::Bank,
        6.,
        120,
        false,
        false,
        Vec3::new(0., -9.81, 0.),
        20.0137,
    );
}

#[test]
fn speed_envelopes_and_transactional_rejection() {
    for speed in [3., 6., 12., 24.] {
        for path in [
            Path::Straight,
            Path::Turn,
            Path::Bank,
            Path::Helix,
            Path::Loop,
            Path::Inverted,
        ] {
            run(path, speed, 24, false, false, Vec3::ZERO, 20.0137);
        }
    }
    let r = Rail::analytic(Path::Straight, 120., false).unwrap();
    let mut t = Tracked::new(model(Path::Straight, 20.0137, false), &r, 24.).unwrap();
    t.step(&r, Vec3::ZERO).unwrap();
    let (before, hints, envelopes) = (t.state.clone(), t.hints(), t.envelopes.clone());
    let windows: Vec<_> = t.model.supports.iter().map(|s| s.half_window).collect();
    // Budget rejection and a failure AFTER full dynamics (near an open end).
    for _ in 0..2 {
        assert!(t.step(&r, Vec3::new(f64::NAN, 0., 0.)).is_err());
        assert_eq!(t.state, before);
        assert_eq!(t.hints(), hints);
        assert_eq!(t.envelopes, envelopes);
    }
    let mut fast = Tracked::new(model(Path::Straight, 20.0137, false), &r, 48.).unwrap();
    let old = fast.state.clone();
    let old_hints = fast.hints();
    assert_eq!(
        fast.step(&r, Vec3::ZERO).unwrap_err(),
        Failure::Inaccessible
    );
    assert_eq!(fast.state, old);
    assert_eq!(fast.hints(), old_hints);
    assert_eq!(fast.ticks, 0);
    assert_eq!(
        windows,
        t.model
            .supports
            .iter()
            .map(|s| s.half_window)
            .collect::<Vec<_>>()
    );
    let mut late = Tracked::new(model(Path::Straight, 20.0137, false), &r, 0.).unwrap();
    let (before, hints) = (late.state.clone(), late.hints());
    // At zero initial rate the declared post-step innovation is 2 mm. A real
    // 500 m/s² external acceleration moves 5.4 mm within the 12 mm search.
    // Prove the core finishes, then require late rollback of the whole wrapper.
    for s in &mut late.model.supports {
        s.half_window = 0.012;
    }
    let mut candidate = before.clone();
    refined_dynamics::step(&late.model, &r, &mut candidate, Vec3::new(500., 0., 0.), 4).unwrap();
    assert!((candidate.poses[0].position_m - before.poses[0].position_m).length() > 0.002);
    for _ in 0..2 {
        assert_eq!(
            late.step(&r, Vec3::new(500., 0., 0.)).unwrap_err(),
            Failure::Inaccessible
        );
        assert_eq!(late.state, before);
        assert_eq!(late.hints(), hints);
        assert_eq!(late.ticks, 0);
        assert!(late.envelopes.is_empty());
        assert!(late.model.supports.iter().all(|s| s.half_window == 0.012));
    }
    let small = Rail::analytic(Path::Turn, 2. * std::f64::consts::PI * 8. * 2., false).unwrap();
    let surface = Surface {
        offset: Vec3::ZERO,
        ..support(Role::Running)
    };
    let x = small.jet(surface, 10.).unwrap().p;
    assert_eq!(
        small.project(surface, x, 10., 0.0000001).unwrap_err(),
        Failure::Inaccessible
    );
    // Preserve conservative ambiguity rejection even for unequal nearby laps.
    let radius = 0.02;
    let lap = 2. * std::f64::consts::PI * radius;
    let ambiguous = Rail::sampled(
        rail_fixtures::sampled(2. * lap, 0.001, |s| {
            rail_fixtures::helix(s, radius, 0.001, 0.)
        }),
        false,
    )
    .unwrap();
    let x = ambiguous.jet(surface, 0.03).unwrap().p;
    assert_eq!(
        ambiguous
            .project(surface, x, 0.03 + lap * 0.5, 0.1)
            .unwrap_err(),
        Failure::Ambiguous
    );
}

#[test]
fn refinement_preserves_four_step_control_and_exposes_speed_squared_loads() {
    for path in PATHS {
        let r = Rail::analytic(path, 120., false).unwrap();
        let m = model(path, 20.0137, false);
        let (state, _) = original_dynamics::initial(&m, &r, 3.).unwrap();
        let mut a = state.clone();
        let mut b = state.clone();
        assert_eq!(
            dynamics::step(&m, &r, &mut a, Vec3::ZERO).unwrap(),
            refined_dynamics::step(&m, &r, &mut b, Vec3::ZERO, 4).unwrap()
        );
        assert_eq!(a, b);
        // Keep the unrefined high-speed accuracy limitation executable. The
        // speed-aware tracker can find the correct station while four internal
        // steps fail M4C's tighter mechanical-quality envelope.
        if matches!(path, Path::Turn | Path::Bank | Path::Helix | Path::Loop) {
            for speed in [12., 24.] {
                let mut t = Tracked::new(model(path, 20.0137, false), &r, speed).unwrap();
                let (mut p, mut v) = (0_f64, 0_f64);
                for _ in 0..24 {
                    let report = t.step(&r, Vec3::ZERO).unwrap();
                    p = p.max(report.max_position);
                    v = v.max(report.max_velocity);
                }
                assert!(p > 2e-5 || v > 0.003);
                println!(
                    "M4D_UNREFINED {path:?} speed={speed} substeps=4 position={p:.9e} velocity={v:.9e} quality=false"
                );
            }
        }
        let (_, base) = original_dynamics::instantaneous(&m, &r, &state, Vec3::ZERO).unwrap();
        for speed in [6., 12., 24.] {
            let (state, _) = original_dynamics::initial(&m, &r, speed).unwrap();
            let (_, loads) = original_dynamics::instantaneous(&m, &r, &state, Vec3::ZERO).unwrap();
            let expected: Vec<_> = base.iter().map(|l| l * (speed / 3.).powi(2)).collect();
            assert!(error(&loads, &expected) < 0.003, "{path:?} speed={speed}");
        }
    }
}

#[test]
#[ignore = "long native Release dynamic characterization, deterministic replay"]
fn long_continuous_elements_and_shared_carrier() {
    for (path, speed, ticks, shared, circuit, gravity, lead) in [
        (
            Path::Bank,
            6.,
            480,
            false,
            false,
            Vec3::new(0., -9.81, 0.),
            20.0137,
        ),
        (
            Path::Helix,
            6.,
            480,
            false,
            false,
            Vec3::new(0., -9.81, 0.),
            20.0137,
        ),
        (Path::Loop, 12., 1200, false, true, Vec3::ZERO, 45.0137),
        (Path::Turn, 12., 1200, false, true, Vec3::ZERO, 45.0137),
        (Path::Bank, 6., 480, true, false, Vec3::ZERO, 20.0137),
        (Path::Loop, 24., 600, false, true, Vec3::ZERO, 45.0137),
    ] {
        run(path, speed, ticks, shared, circuit, gravity, lead);
    }
}

#[test]
#[ignore = "serial native Release complete-step timing, not a CI deadline"]
fn complete_step_costs_and_helix_variability() {
    use std::{hint::black_box, time::Instant};
    let summarize = |label: &str, mut values: Vec<f64>| {
        values.sort_by(f64::total_cmp);
        println!(
            "M4D_COST {label} median_us={:.3} min_us={:.3} max_us={:.3}",
            values[values.len() / 2],
            values[0],
            values[values.len() - 1]
        );
    };
    for path in [Path::Straight, Path::Bank, Path::Helix] {
        let r = Rail::analytic(path, 120., false).unwrap();
        for (speed, substeps) in [(3., 4), (6., 4), (12., 16), (24., 64)] {
            let mut t = Tracked::new(model(path, 20.0137, false), &r, speed).unwrap();
            t.substeps = substeps;
            let before = t.state.clone();
            let hints = t.hints();
            let mut times = vec![];
            for sample in 0..36 {
                t.state = before.clone();
                t.ticks = 0;
                t.envelopes.clear();
                for (s, h) in t.model.supports.iter_mut().zip(&hints) {
                    s.station = *h;
                }
                r.jets.set(0);
                let clock = Instant::now();
                black_box(t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap());
                let elapsed = clock.elapsed().as_secs_f64() * 1e6;
                if sample > 0 {
                    times.push(elapsed);
                }
            }
            summarize(
                &format!(
                    "{path:?} analytic speed={speed} substeps={substeps} jets={}",
                    r.jets.get()
                ),
                times,
            );
        }
    }
    // Complete late failure: the core finishes, then the innovation gate rolls
    // back. Include snapshots, candidate dynamics and restoration in the timer.
    let r = Rail::analytic(Path::Straight, 120., false).unwrap();
    let mut t = Tracked::new(model(Path::Straight, 20.0137, false), &r, 0.).unwrap();
    let before = t.state.clone();
    let hints = t.hints();
    let mut times = vec![];
    for sample in 0..36 {
        let clock = Instant::now();
        let result = black_box(t.step(&r, Vec3::new(500., 0., 0.)));
        let elapsed = clock.elapsed().as_secs_f64() * 1e6;
        assert_eq!(result.unwrap_err(), Failure::Inaccessible);
        assert_eq!(t.state, before);
        assert_eq!(t.hints(), hints);
        assert_eq!(t.ticks, 0);
        if sample > 0 {
            times.push(elapsed);
        }
    }
    summarize("late-rollback after successful straight core", times);
    // Old helix core/wrapper paired in BOTH orders, repeated in three rounds.
    // Identical reset states and deterministic counters separate query work
    // from scheduling/cache/frequency variability. No compile job runs here.
    for round in 0..3 {
        let r = Rail::sampled(sampled(Path::Helix), false).unwrap();
        let mut t =
            legacy_tracking::Tracked::new(model(Path::Helix, 20.0137, false), &r, 3.).unwrap();
        let before = t.state.clone();
        let hints = t.hints();
        let (mut core_times, mut wrapper_times, mut overhead) = (vec![], vec![], vec![]);
        let mut counts = [0; 2];
        for sample in 0..36 {
            for wrapper in if sample % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                t.state = before.clone();
                t.ticks = 0;
                for (s, h) in t.model.supports.iter_mut().zip(&hints) {
                    s.station = *h;
                }
                r.control.as_ref().unwrap().reset_counts();
                let clock = Instant::now();
                if wrapper {
                    black_box(t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap());
                } else {
                    black_box(
                        dynamics::step(&t.model, &r, &mut t.state, Vec3::new(0., -9.81, 0.))
                            .unwrap(),
                    );
                }
                let time = clock.elapsed().as_secs_f64() * 1e6;
                let index = usize::from(wrapper);
                let work = r.control.as_ref().unwrap().counts.get().jets;
                if sample > 0 {
                    if counts[index] != 0 {
                        assert_eq!(counts[index], work);
                    }
                    counts[index] = work;
                    if wrapper {
                        wrapper_times.push(time);
                    } else {
                        core_times.push(time);
                    }
                }
            }
            if sample > 0 {
                overhead.push(wrapper_times.last().unwrap() - core_times.last().unwrap());
            }
        }
        summarize(
            &format!("helix-control round={round} core jets={}", counts[0]),
            core_times,
        );
        summarize(
            &format!("helix-control round={round} wrapper jets={}", counts[1]),
            wrapper_times,
        );
        summarize(
            &format!("helix-control round={round} paired-extra"),
            overhead,
        );
    }
}
