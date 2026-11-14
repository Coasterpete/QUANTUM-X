//! M4C isolated bilateral guidance: preserved M4B controls and independent checks.
#[allow(dead_code)]
#[path = "support/m4b_contract.rs"]
mod contract;
#[path = "support/m4c_dynamics.rs"]
mod dynamics;
#[allow(dead_code)]
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[allow(dead_code)]
#[path = "support/m4a_matrix.rs"]
mod matrix;
#[allow(dead_code)]
#[path = "support/m4a_mechanisms.rs"]
mod mechanisms;
#[path = "support/m4b_dynamics.rs"]
mod original_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[path = "support/m4c_surface.rs"]
mod surface;
#[path = "support/m4c_tracking.rs"]
mod tracking;

use contract::{Kind, Model};
use matrix::{finite_jacobian, max_abs, multiply};
use mechanisms::{Mechanism, Path};
use qx_geometry::math::Vec3;
use surface::{Failure, Rail, Role, Surface};
use tracking::Tracked;

fn rail(path: Path) -> Rail {
    Rail::new(
        rail_fixtures::sampled(60., 0.025, |s| {
            let p = path.at(s);
            (p.position_m, p.orientation)
        }),
        false,
    )
    .unwrap()
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
        face: "authored-guide",
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
const PATHS: [Path; 6] = [
    Path::Straight,
    Path::Turn,
    Path::Bank,
    Path::Helix,
    Path::Loop,
    Path::Inverted,
];

#[test]
fn cached_jets_preserve_interpolation_and_independent_position_derivatives() {
    for path in PATHS {
        let r = rail(path);
        for role in [Role::Running, Role::Side] {
            let surface = support(role);
            for s in [0., 9.0137, 20.0137, 59.9123, 60.] {
                let a = r.jet(surface, s).unwrap();
                let b = r.reference_jet(surface, s).unwrap();
                for (a, b) in [
                    (a.p, b.p),
                    (a.d, b.d),
                    (a.dd, b.dd),
                    (a.n, b.n),
                    (a.dn, b.dn),
                    (a.k, b.k),
                ] {
                    assert!((a - b).length() < 1e-12, "{path:?} {s}");
                }
                if s > 0. && s < 60. {
                    for h in [1e-5, 1e-6] {
                        let plus = r.reference_jet(surface, s + h).unwrap();
                        let minus = r.reference_jet(surface, s - h).unwrap();
                        assert!(((plus.p - minus.p) / (2. * h) - a.d).length() < 2e-8);
                        assert!(((plus.n - minus.n) / (2. * h) - a.dn).length() < 2e-8);
                    }
                }
            }
        }
    }
}

// Independent bounded minimization of POSITION distances, with no F, D or Newton
// code. Each interpolation span is minimized separately, including both ends.
fn reference_station(r: &Rail, surface: Surface, x: Vec3, lo: f64, hi: f64) -> f64 {
    let distance = |s| {
        let d = x - r.reference_jet(surface, s).unwrap().p;
        d.dot(d)
    };
    let mut best = (lo, distance(lo));
    for pair in r.track.samples.windows(2) {
        let (mut a, mut b) = (lo.max(pair[0].distance_m), hi.min(pair[1].distance_m));
        if b <= a {
            continue;
        }
        for end in [a, b] {
            if distance(end) < best.1 {
                best = (end, distance(end));
            }
        }
        for _ in 0..64 {
            let l = a + (b - a) * 0.3819660112501051;
            let h = b - (b - a) * 0.3819660112501051;
            if distance(l) < distance(h) {
                b = h;
            } else {
                a = l;
            }
        }
        let s = (a + b) * 0.5;
        if distance(s) < best.1 {
            best = (s, distance(s));
        }
    }
    best.0
}

#[test]
fn piecewise_projection_matches_independent_distance_minimization() {
    for path in PATHS {
        let r = rail(path);
        for role in [Role::Running, Role::Side] {
            let surface = support(role);
            for s in [9.0137, 20.0137, 39.0123] {
                let j = r.jet(surface, s).unwrap();
                let x = j.p + j.n * 0.003;
                let (station, _) = r.project(surface, x, s - 0.03, 0.15).unwrap();
                let reference = reference_station(&r, surface, x, s - 0.18, s + 0.12);
                assert!(
                    (station - reference).abs() < 2e-7,
                    "{path:?} {station} {reference}"
                );
                assert!(
                    (x - r.jet(surface, station).unwrap().p)
                        .dot(r.jet(surface, station).unwrap().d)
                        .abs()
                        < 1e-11
                );
            }
        }
    }
}

#[test]
fn changing_bank_corner_is_measured_not_smoothed_or_misreported_as_a_root() {
    let r = rail(Path::Bank);
    let s = support(Role::Running);
    let x = r.jet(s, 20.).unwrap().p;
    let knot = r.knot(s, 800, x).unwrap();
    assert!(knot.derivative_jump > 1e-6);
    assert_eq!(r.project(s, x, 20., 0.4).unwrap_err(), Failure::Corner);
    let witness = r.corner.get().unwrap();
    assert_eq!(witness.station, 20.);
    println!("M4C_KNOT {witness:?}");
    for reference in [true, false] {
        let mut r = rail(Path::Bank);
        r.use_reference = reference;
        let m = model(Path::Bank, 20.0137, false);
        let (mut state, _) = original_dynamics::initial(&m, &r, 6.).unwrap();
        let mut completed = 0;
        let mut failure = None;
        for _ in 0..24 {
            let before = state.clone();
            match dynamics::step(&m, &r, &mut state, Vec3::new(0., -9.81, 0.)) {
                Ok(_) => completed += 1,
                Err(reason) => {
                    assert_eq!(state, before);
                    failure = Some(reason);
                    break;
                }
            }
        }
        println!(
            "M4C_BANK reference={reference} completed={completed} failure={failure:?} corner={:?}",
            r.corner.get()
        );
        assert_eq!(failure, Some(Failure::Corner));
        assert_eq!(completed, 23);
        if !reference {
            let witness = r.corner.get().unwrap();
            assert!(witness.f_left > 0. && witness.f_right < 0.);
            assert!(witness.derivative_jump > 1e-6);
            // The independent distance minimum is the corner, not a smooth root.
            let independent = reference_station(
                &r,
                witness.surface,
                witness.point,
                witness.station - 0.025,
                witness.station + 0.025,
            );
            assert!((independent - witness.station).abs() < 1e-7);
        }
    }
}

#[test]
fn endpoints_ambiguity_and_unwrapped_seams_keep_explicit_branches() {
    let r = rail(Path::Straight);
    let s = support(Role::Running);
    assert_eq!(
        r.project(s, Vec3::new(-0.1, -0.08, 0.6), 0.1, 0.2)
            .unwrap_err(),
        Failure::Endpoint
    );
    assert_eq!(
        r.project(s, Vec3::new(22., 0., 0.6), 20., 0.2).unwrap_err(),
        Failure::Inaccessible
    );
    for (hint, half) in [(f64::NAN, 0.2), (20., 3.), (20., 0.)] {
        assert_eq!(
            r.project(s, Vec3::ZERO, hint, half).unwrap_err(),
            Failure::Invalid
        );
    }
    let s = Surface {
        offset: Vec3::ZERO,
        ..s
    };
    let radius = 0.2;
    let lap = 2. * std::f64::consts::PI * radius;
    let sampled =
        |length| rail_fixtures::sampled(length, 0.005, |s| rail_fixtures::helix(s, radius, 0., 0.));
    let repeated = Rail::new(sampled(2. * lap), false).unwrap();
    let x = repeated.jet(s, 0.137).unwrap().p;
    assert_eq!(
        repeated.project(s, x, 0.137 + lap * 0.5, 0.8).unwrap_err(),
        Failure::Ambiguous
    );
    for seed in [0.137, 0.137 + lap] {
        assert!((repeated.project(s, x, seed, 0.08).unwrap().0 - seed).abs() < 1e-9);
    }
    // Nearby parallel laps have unequal spatial minima. A nearer lap is still
    // not authorization to abandon a branch in a broad continuation window.
    let near = Rail::new(
        rail_fixtures::sampled(2. * lap, 0.005, |s| {
            rail_fixtures::helix(s, radius, 0.001, 0.)
        }),
        false,
    )
    .unwrap();
    let x = near.jet(s, 0.137).unwrap().p;
    assert_eq!(
        near.project(s, x, 0.137 + lap * 0.5, 0.8).unwrap_err(),
        Failure::Ambiguous
    );
    let closed = Rail::new(sampled(lap), true).unwrap();
    for direction in [-1., 1.] {
        let mut hint = lap - 0.0137;
        for _ in 0..100 {
            let target = hint + direction * 0.013;
            let x = closed.jet(s, target).unwrap().p;
            let found = closed.project(s, x, hint, 0.08).unwrap().0;
            assert!((found - target).abs() < 1e-10);
            hint = found;
        }
    }
}

#[test]
fn unchanged_contract_jacobian_acceleration_and_independent_reactions() {
    for path in PATHS {
        let r = rail(path);
        let m = model(path, 20.0137, false);
        let (state, _) = original_dynamics::initial(&m, &r, 3.).unwrap();
        let e = m.evaluate(&r, &state.poses, Kind::B1).unwrap();
        assert!(max_abs(&multiply(&e.j, &state.velocity)) < 1e-10);
        let finite = finite_jacobian(state.velocity.len(), 1e-6, |d| {
            m.evaluate(&r, &m.shifted(&state.poses, d), Kind::B1)
                .unwrap()
                .c
        });
        assert!(error(&e.j.concat(), &finite.concat()) < 3e-7);
        let displacement: Vec<_> = (0..state.velocity.len())
            .map(|i| (i as f64 * 1.7).sin() * 0.002)
            .collect();
        let displaced = m.shifted(&state.poses, &displacement);
        let displaced_e = m.evaluate(&r, &displaced, Kind::B1).unwrap();
        let displaced_finite = finite_jacobian(state.velocity.len(), 1e-6, |d| {
            m.evaluate(&r, &m.shifted(&displaced, d), Kind::B1)
                .unwrap()
                .c
        });
        assert!(error(&displaced_e.j.concat(), &displaced_finite.concat()) < 3e-7);
        let gamma = m
            .curvature(&r, &state.poses, &state.velocity, Kind::B1, 1e-5)
            .unwrap();
        let refined = m
            .curvature(&r, &state.poses, &state.velocity, Kind::B1, 5e-6)
            .unwrap();
        assert!(error(&gamma, &refined) < 1e-6);
        let mut h = 2e-4;
        for _ in 0..12 {
            let plus = m
                .evaluate(
                    &r,
                    &m.shifted(
                        &state.poses,
                        &state.velocity.iter().map(|v| v * h).collect::<Vec<_>>(),
                    ),
                    Kind::B1,
                )
                .unwrap();
            let minus = m
                .evaluate(
                    &r,
                    &m.shifted(
                        &state.poses,
                        &state.velocity.iter().map(|v| -v * h).collect::<Vec<_>>(),
                    ),
                    Kind::B1,
                )
                .unwrap();
            if plus.branches == e.branches && minus.branches == e.branches {
                break;
            }
            h *= 0.5;
        }
        let plus = m
            .evaluate(
                &r,
                &m.shifted(
                    &state.poses,
                    &state.velocity.iter().map(|v| v * h).collect::<Vec<_>>(),
                ),
                Kind::B1,
            )
            .unwrap();
        let minus = m
            .evaluate(
                &r,
                &m.shifted(
                    &state.poses,
                    &state.velocity.iter().map(|v| -v * h).collect::<Vec<_>>(),
                ),
                Kind::B1,
            )
            .unwrap();
        assert_eq!(e.branches, plus.branches);
        assert_eq!(e.branches, minus.branches);
        let second: Vec<_> = plus
            .c
            .iter()
            .zip(&minus.c)
            .zip(&e.c)
            .map(|((a, b), c)| (a + b - 2. * c) / (h * h))
            .collect();
        assert!(error(&gamma, &second) < 2e-5);
        let gravity = Vec3::new(0., -9.81, 0.);
        let (a, loads) = original_dynamics::instantaneous(&m, &r, &state, gravity).unwrap();
        assert!(contract::work(&e.j, &state.velocity, &loads).abs() < 1e-7);
        if matches!(path, Path::Straight | Path::Inverted) {
            let running: f64 = m
                .supports
                .iter()
                .zip(&loads)
                .filter(|(s, _)| s.surface.role == Role::Running)
                .map(|(_, l)| *l)
                .sum();
            let expected = if matches!(path, Path::Straight) {
                6867.
            } else {
                -6867.
            };
            assert!((running - expected).abs() < 1e-7);
        }
        if matches!(path, Path::Turn) {
            // Independent analytic circular-motion acceleration for every COM.
            let omega = state.velocity[m.axles[0].0 * 6 + 4];
            for (p, acc) in state.poses.iter().zip(a.as_chunks::<6>().0) {
                let expected =
                    Vec3::new(p.position_m.x, 0., p.position_m.z - 8.) * (-omega * omega);
                assert!((Vec3::new(acc[0], acc[1], acc[2]) - expected).length() < 2e-5);
            }
        }
        println!(
            "M4C_SYSTEM {{\"name\":\"{path:?}\",\"j\":{:?},\"v\":{:?},\"gamma\":{:?},\"loads\":{:?},\"acceleration\":{:?}}}",
            e.j, state.velocity, gamma, loads, a
        );
    }
}

#[test]
fn sparse_impulses_match_preserved_dense_and_projection_controls() {
    for path in PATHS {
        let mut r = rail(path);
        let m = model(path, 20.0137, false);
        let (mut fast, _) = original_dynamics::initial(&m, &r, 3.).unwrap();
        let mut dense = fast.clone();
        let mut reference = fast.clone();
        for _ in 0..8 {
            r.use_reference = false;
            let report = dynamics::step(&m, &r, &mut fast, Vec3::new(0., -9.81, 0.)).unwrap();
            let dense_report =
                original_dynamics::step(&m, &r, &mut dense, Vec3::new(0., -9.81, 0.)).unwrap();
            r.use_reference = true;
            let reference_report =
                original_dynamics::step(&m, &r, &mut reference, Vec3::new(0., -9.81, 0.)).unwrap();
            assert!(error(&fast.velocity, &dense.velocity) < 1e-10);
            assert!(error(&report.loads, &dense_report.loads) < 1e-6);
            assert!(error(&fast.velocity, &reference.velocity) < 1e-7);
            assert!(error(&report.loads, &reference_report.loads) < 1e-3);
            for (a, b) in fast.poses.iter().zip(&reference.poses) {
                assert!((a.position_m - b.position_m).length() < 1e-9);
            }
        }
    }
}

#[test]
fn tracked_dynamics_reverse_energy_residuals_and_transactional_hint_loss() {
    for path in [Path::Straight, Path::Turn, Path::Inverted] {
        for speed in [-3., 3.] {
            run_tracked(path, speed, 120, false);
        }
    }
    // Moving beyond the declared continuation window fails without a branch hop.
    let r = rail(Path::Straight);
    let mut t = Tracked::new(model(Path::Straight, 20.0137, false), &r, 12.).unwrap();
    let before = t.state.clone();
    let hints = t.hints();
    assert_eq!(t.step(&r, Vec3::ZERO).unwrap_err(), Failure::Inaccessible);
    assert_eq!(t.state, before);
    assert_eq!(t.hints(), hints);
    assert_eq!(t.ticks, 0);
    // A true corner also rolls back hints and accepted tick count, including
    // repeated attempts after the failure. No automatic smoothing or retry hop.
    let r = rail(Path::Bank);
    let mut t = Tracked::new(model(Path::Bank, 20.0137, false), &r, 6.).unwrap();
    for _ in 0..23 {
        t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap();
    }
    let before = t.state.clone();
    let hints = t.hints();
    for _ in 0..2 {
        assert_eq!(
            t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap_err(),
            Failure::Corner
        );
        assert_eq!(t.state, before);
        assert_eq!(t.hints(), hints);
        assert_eq!(t.ticks, 23);
    }
}
fn run_tracked(path: Path, speed: f64, ticks: usize, shared: bool) {
    let r = rail(path);
    let lead = if ticks > 1000 { 6.0137 } else { 20.0137 };
    let mut t = Tracked::new(model(path, lead, shared), &r, speed).unwrap();
    let mut repeat = Tracked::new(model(path, lead, shared), &r, speed).unwrap();
    let gravity = if ticks > 1000 {
        Vec3::ZERO
    } else {
        Vec3::new(0., -9.81, 0.)
    };
    let initial = original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, gravity);
    let initial_kinetic = contract::kinetic(&t.state.velocity, &t.model.inverse_mass());
    let (mut p, mut v, mut energy, mut balance) = (0_f64, 0_f64, 0_f64, 0_f64);
    let start = t.hints()[0];
    for _ in 0..ticks {
        let report = t.step(&r, gravity).unwrap();
        assert_eq!(report, repeat.step(&r, gravity).unwrap());
        assert_eq!(t.state, repeat.state);
        assert_eq!(t.hints(), repeat.hints());
        assert_eq!(t.ticks, repeat.ticks);
        p = p.max(report.max_position);
        v = v.max(report.max_velocity);
        energy = energy.max((report.energy - initial).abs());
        balance = balance.max(report.linear_balance);
    }
    let travel = t.hints()[0] - start;
    assert!(travel * speed > 0.);
    assert!(p < 2e-5);
    assert!(v < 0.003);
    // At most 1e-5 of the initial kinetic energy, with a 5 mJ roundoff floor.
    // This is a research conservation envelope, not a contact/safety claim.
    assert!(energy < (1e-5 * initial_kinetic).max(0.005));
    assert!(balance < 1e-6);
    if ticks > 1000 {
        assert!(travel > 16. * std::f64::consts::PI);
    }
    println!(
        "M4C_RUN {path:?} speed={speed} ticks={ticks} shared={shared} travel_m={travel:.9e} position={p:.9e} velocity={v:.9e} energy_J={energy:.9e} momentum_N={balance:.9e}"
    );
}

#[test]
#[ignore = "long native Release dynamic element traversal and shared graph replay"]
fn long_turn_and_shared_graph_replay() {
    run_tracked(Path::Turn, 6., 2100, false);
    run_tracked(Path::Straight, 3., 480, true);
}

#[test]
#[ignore = "native Release paired phase profile; timings are not CI limits"]
fn paired_phase_profile() {
    use std::{hint::black_box, time::Instant};
    for path in [Path::Straight, Path::Bank, Path::Helix] {
        let mut r = rail(path);
        let mut m = model(path, 20.0137, false);
        let (state, _) = original_dynamics::initial(&m, &r, 3.).unwrap();
        for (reference, sparse, tracked) in [
            (true, false, false),
            (false, false, false),
            (false, false, true),
            (false, true, true),
        ] {
            if tracked {
                for support in &mut m.supports {
                    support.station = r
                        .project(
                            support.surface,
                            guidance::point(state.poses[support.owner], support.local),
                            support.station,
                            support.half_window,
                        )
                        .unwrap()
                        .0;
                    support.half_window = 0.08;
                }
            }
            r.use_reference = reference;
            let mut times = vec![];
            let mut phases = [0.; 4];
            for sample in 0..36 {
                let mut s = state.clone();
                r.reset_counts();
                let clock = Instant::now();
                let (_, p) = black_box(
                    dynamics::profiled(&m, &r, &mut s, Vec3::new(0., -9.81, 0.), sparse).unwrap(),
                );
                if sample == 0 {
                    continue;
                }
                times.push(clock.elapsed().as_secs_f64() * 1e6);
                for (sum, d) in phases.iter_mut().zip([
                    p.evaluate,
                    p.curvature,
                    p.impulses,
                    p.advance_and_validate,
                ]) {
                    *sum += d.as_secs_f64() * 1e6 / 35.;
                }
            }
            times.sort_by(f64::total_cmp);
            println!(
                "M4C_PROFILE {path:?} reference={reference} sparse={sparse} tracked={tracked} median_us={:.3} min_us={:.3} max_us={:.3} phase_mean_us={phases:?} counts={:?}",
                times[17],
                times[0],
                times[34],
                r.counts.get()
            );
        }
        // Full station-tracked wrapper, including transactional state copy,
        // post-tick projection and hint commit. Setup/reset are outside timers.
        r.use_reference = false;
        let mut t = Tracked::new(model(path, 20.0137, false), &r, 3.).unwrap();
        let state = t.state.clone();
        let hints = t.hints();
        let mut times = vec![];
        for sample in 0..36 {
            t.state = state.clone();
            t.ticks = 0;
            for (support, hint) in t.model.supports.iter_mut().zip(&hints) {
                support.station = *hint;
            }
            let clock = Instant::now();
            black_box(t.step(&r, Vec3::new(0., -9.81, 0.)).unwrap());
            if sample > 0 {
                times.push(clock.elapsed().as_secs_f64() * 1e6);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "M4C_TRACKED_COST {path:?} median_us={:.3} min_us={:.3} max_us={:.3}",
            times[17], times[0], times[34]
        );
    }
}
