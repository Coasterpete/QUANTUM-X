//! M4B isolated bilateral guidance research. No production path imports this.
#[path = "support/m4b_contract.rs"]
mod contract;
#[path = "support/m4b_dynamics.rs"]
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
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[path = "support/m4b_surface.rs"]
mod surface;

use contract::{Kind, Model, correction};
use guidance::point;
use matrix::{Matrix, finite_jacobian, max_abs, multiply, nullspace};
use mechanisms::{Mechanism, Path};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    rail_geometry::{PhysicalRailGeometry, RailReference, WheelCenterPath},
    topology::LayoutMode,
};
use surface::{Failure, Rail, Role, Surface};

#[test]
fn acceleration_contract_velocity_initialization_and_reactions() {
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let r = rail(path);
        let m = model(path, 20.0137, true, false, false);
        for speed in [0., 3., 6.] {
            let (state, _) = dynamics::initial(&m, &r, speed).unwrap();
            let e = m.evaluate(&r, &state.poses, Kind::B1).unwrap();
            assert!(max_abs(&multiply(&e.j, &state.velocity)) < 1e-10);
            let gamma = m
                .curvature(&r, &state.poses, &state.velocity, Kind::B1, 1e-5)
                .unwrap();
            let refined = m
                .curvature(&r, &state.poses, &state.velocity, Kind::B1, 5e-6)
                .unwrap();
            assert!(
                max_abs(
                    &gamma
                        .iter()
                        .zip(refined)
                        .map(|(a, b)| a - b)
                        .collect::<Vec<_>>()
                ) < 1e-6
            );
            // Independent SECOND differences of positions, not differences of J.
            let mut h = 2e-4;
            for _ in 0..12 {
                let plus = m.shifted(
                    &state.poses,
                    &state.velocity.iter().map(|v| v * h).collect::<Vec<_>>(),
                );
                let minus = m.shifted(
                    &state.poses,
                    &state.velocity.iter().map(|v| -v * h).collect::<Vec<_>>(),
                );
                if m.evaluate(&r, &plus, Kind::B1).unwrap().branches == e.branches
                    && m.evaluate(&r, &minus, Kind::B1).unwrap().branches == e.branches
                {
                    break;
                }
                h *= 0.5;
            }
            let plus = m.shifted(
                &state.poses,
                &state.velocity.iter().map(|v| v * h).collect::<Vec<_>>(),
            );
            let minus = m.shifted(
                &state.poses,
                &state.velocity.iter().map(|v| -v * h).collect::<Vec<_>>(),
            );
            let a = m.evaluate(&r, &plus, Kind::B1).unwrap();
            let b = m.evaluate(&r, &minus, Kind::B1).unwrap();
            assert_eq!(a.branches, e.branches);
            assert_eq!(b.branches, e.branches);
            let second: Vec<_> =
                a.c.iter()
                    .zip(&b.c)
                    .zip(&e.c)
                    .map(|((a, b), c)| (a + b - 2. * c) / (h * h))
                    .collect();
            assert!(
                max_abs(
                    &second
                        .iter()
                        .zip(&gamma)
                        .map(|(a, b)| a - b)
                        .collect::<Vec<_>>()
                ) < 2e-5,
                "{path:?} {speed}"
            );
            let gravity = Vec3::new(0., -9.81, 0.);
            let (_, loads) = dynamics::instantaneous(&m, &r, &state, gravity).unwrap();
            assert!(contract::work(&e.j, &state.velocity, &loads).abs() < 1e-7);
            // Every INTERNAL bearing linear row sums to zero across the graph.
            for row in &e.j[e.rail_rows..] {
                let total = row
                    .as_chunks::<6>()
                    .0
                    .iter()
                    .fold(Vec3::ZERO, |a, b| a + Vec3::new(b[0], b[1], b[2]));
                assert!(total.length() < 1e-12);
            }
            let running: Vec<_> = m
                .supports
                .iter()
                .zip(&loads)
                .filter(|(s, _)| s.surface.role == Role::Running)
                .map(|(_, l)| *l)
                .collect();
            println!(
                "M4B_REACTION {path:?} speed={speed} max_load={:.9e} running_min={:.9e} rail_max={:.9e} bearing_max={:.9e} power={:.9e}",
                max_abs(&loads),
                running.iter().copied().fold(f64::INFINITY, f64::min),
                max_abs(&loads[..e.rail_rows]),
                max_abs(&loads[e.rail_rows..]),
                contract::work(&e.j, &state.velocity, &loads)
            );
            if matches!(path, Path::Straight) {
                let total: f64 = running.iter().sum();
                assert!((total - 6867.).abs() < 1e-7);
            }
            if matches!(path, Path::Inverted) {
                assert!(running.iter().all(|l| *l < 0.));
            }
        }
    }
}

#[test]
fn isolated_impulses_residual_energy_determinism_and_rollback() {
    let gravity = Vec3::new(0., -9.81, 0.);
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let r = rail(path);
        let m = model(path, 20.0137, true, false, false);
        for speed in [0., 3., 6.] {
            let (mut state, _) = dynamics::initial(&m, &r, speed).unwrap();
            let mut repeat = state.clone();
            let start = dynamics::energy(&m, &state.poses, &state.velocity, gravity);
            let mut pos = 0_f64;
            let mut vel = 0_f64;
            let mut energy = 0_f64;
            let mut balance = 0_f64;
            let mut first = 0_f64;
            let mut completed = 0;
            let mut failure = None;
            for i in 0..24 {
                let before = state.clone();
                let report = match dynamics::step(&m, &r, &mut state, gravity) {
                    Ok(report) => report,
                    Err(reason) => {
                        assert_eq!(reason, Failure::Corner);
                        assert_eq!(state, before);
                        assert_eq!(
                            dynamics::step(&m, &r, &mut repeat, gravity).unwrap_err(),
                            reason
                        );
                        assert_eq!(state, repeat);
                        failure = Some(reason);
                        break;
                    }
                };
                let again = dynamics::step(&m, &r, &mut repeat, gravity).unwrap();
                completed += 1;
                assert_eq!(state, repeat);
                assert_eq!(report, again);
                pos = pos.max(report.max_position);
                vel = vel.max(report.max_velocity);
                energy = energy.max((report.energy - start).abs());
                balance = balance.max(report.linear_balance);
                if i == 0 {
                    first = max_abs(&report.loads);
                }
            }
            println!(
                "M4B_TIME {path:?} speed={speed} steps={completed} failure={failure:?} first_max_load={first:.9e} max_position={pos:.9e} max_velocity={vel:.9e} energy_drift_J={energy:.9e} momentum_balance_N={balance:.9e}"
            );
            if matches!(path, Path::Bank) && speed == 6. {
                assert_eq!(completed, 23);
                assert_eq!(failure, Some(Failure::Corner));
            } else {
                assert_eq!(completed, 24);
                assert_eq!(failure, None);
            }
            assert!(pos < 2e-5);
            assert!(vel < 0.003);
            assert!(energy < 3.);
            let before = state.clone();
            let mut bad = state.clone();
            bad.velocity[0] = f64::NAN;
            let original = bad.clone();
            assert_eq!(
                dynamics::step(&m, &r, &mut bad, gravity).unwrap_err(),
                Failure::Invalid
            );
            assert_eq!(bad.poses, original.poses);
            assert!(bad.velocity[0].is_nan());
            assert_eq!(state, before);
        }
    }
    // Failure after beginning a substep must roll back ALL poses and velocities.
    let r = rail(Path::Straight);
    let m = model(Path::Straight, 20.0137, false, false, false);
    let (mut state, _) = dynamics::initial(&m, &r, 500.).unwrap();
    let before = state.clone();
    assert!(dynamics::step(&m, &r, &mut state, gravity).is_err());
    assert_eq!(state, before);
}

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
fn model(path: Path, lead: f64, offset: bool, shared: bool, suspended: bool) -> Model {
    Model::new(Mechanism::new(
        path,
        lead,
        suspended,
        if offset {
            Vec3::new(0.05, 0.12, 0.03)
        } else {
            Vec3::ZERO
        },
        shared,
    ))
}
fn difference(a: &Matrix, b: &Matrix) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
        .fold(0., f64::max)
}
fn support(role: Role) -> Surface {
    Surface {
        rail: "right",
        face: "explicit-test-plane",
        offset: Vec3::new(0., 0., 0.6),
        role,
    }
}
fn rank(name: &str, j: &Matrix, expected: usize) {
    for t in [1e-5, 1e-7, 1e-9] {
        assert_eq!(nullspace(j, t).0, expected, "{name} {t}");
    }
    println!("M4B_MATRIX {{\"name\":\"{name}\",\"rank\":{expected},\"a\":{j:?}}}");
}

#[test]
fn exact_interpolation_jets_and_independent_analytic_surfaces() {
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let r = rail(path);
        let physical = PhysicalRailGeometry::new(&r.track, LayoutMode::Open).unwrap();
        for s in [4.0137, 19.0137, 30.0137] {
            for role in [Role::Running, Role::Side] {
                let surface = support(role);
                let j = r.jet(surface, s).unwrap();
                let offset = surface.offset;
                let independent = path.locus(s, offset.y, offset.z);
                assert!((j.p - independent).length() < 2e-6, "{path:?}");
                let declared = WheelCenterPath {
                    rail: RailReference {
                        id: "right".into(),
                        offset_m: offset,
                    },
                    center_offset_m: Vec3::ZERO,
                };
                let old = physical.sample(&declared, s).unwrap();
                assert!((old.wheel_center_m - j.p).length() < 1e-12);
                assert!((old.derivative - j.d).length() < 1e-12);
                for h in [1e-5, 1e-6] {
                    let a = r.jet(surface, s + h).unwrap();
                    let b = r.jet(surface, s - h).unwrap();
                    assert!(((a.p - b.p) / (2. * h) - j.d).length() < 2e-8);
                    assert!(((a.d - b.d) / (2. * h) - j.dd).length() < 2e-8);
                    assert!(((a.n - b.n) / (2. * h) - j.dn).length() < 2e-8);
                }
                assert!(j.n.dot(j.d).abs() < 1e-12);
                if matches!(path, Path::Turn) {
                    let theta = s / 8.;
                    let exact = if role == Role::Running {
                        Vec3::Y
                    } else {
                        Vec3::new(-theta.sin(), 0., theta.cos())
                    };
                    assert!((j.n - exact).length() < 1e-8);
                }
                if matches!(path, Path::Inverted) {
                    assert!(
                        (j.n + if role == Role::Running {
                            Vec3::Y
                        } else {
                            Vec3::Z
                        })
                        .length()
                            < 1e-12
                    );
                }
            }
        }
    }
    // The bank derivative changes the normal: treating R*Y as constant is wrong.
    let r = rail(Path::Bank);
    let j = r.jet(support(Role::Running), 20.0137).unwrap();
    assert!((j.n - j.frame.normal()).length() > 0.01);
}

#[test]
fn common_contract_jacobians_at_closed_and_displaced_poses() {
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let r = rail(path);
        let m = model(path, 20.0137, true, false, false);
        for kind in [Kind::B0, Kind::B1] {
            let (p, _) = m.initialize(&r, kind).unwrap();
            for displacement in [0., 0.002] {
                let d: Vec<_> = (0..6 * p.len())
                    .map(|i| displacement * (i as f64 * 1.37).sin())
                    .collect();
                let p = m.shifted(&p, &d);
                let e = m.evaluate(&r, &p, kind).unwrap();
                for h in [1e-5, 1e-6] {
                    let fd = finite_jacobian(d.len(), h, |d| {
                        m.evaluate(&r, &m.shifted(&p, d), kind).unwrap().c
                    });
                    let error = difference(&e.j, &fd);
                    assert!(
                        error < 3e-7,
                        "{path:?} {kind:?} displacement={displacement} h={h} error={error}"
                    );
                    println!(
                        "M4B_JAC {path:?} {kind:?} displaced={displacement} h={h} error={error:.9e}"
                    );
                }
            }
        }
    }
}

fn wheel_errors(m: &Model, r: &Rail, p: &[qx_physics::Pose]) -> (f64, f64) {
    let (mut center, mut rolling) = (0_f64, 0_f64);
    for s in m
        .supports
        .iter()
        .filter(|s| s.surface.role == Role::Running)
    {
        let mut target = s.surface;
        target.offset.y += 0.18;
        let x = point(p[s.owner], s.local + Vec3::Y * 0.18);
        let (_, j) = r.project(target, x, s.station, s.half_window).unwrap();
        center = center.max((x - j.p).length());
        let forward = Frame::new(p[s.owner].orientation).unwrap().tangent();
        rolling = rolling.max(
            forward
                .cross(j.d / j.d.length())
                .length()
                .atan2(forward.dot(j.d / j.d.length())),
        );
    }
    (center, rolling)
}

#[test]
fn b0_b1_finite_mechanical_closure_rank_and_alignment() {
    for (name, path, lead, offset, shared, suspended) in [
        ("straight", Path::Straight, 20.0137, false, false, false),
        ("tight-turn", Path::Turn, 20.0137, false, false, false),
        ("changing-bank", Path::Bank, 20.0137, false, false, false),
        ("helix", Path::Helix, 20.0137, false, false, false),
        (
            "loop-top",
            Path::Loop,
            8. * std::f64::consts::PI + 0.0137,
            false,
            false,
            false,
        ),
        (
            "loop-vertical",
            Path::Loop,
            4. * std::f64::consts::PI + 0.0137,
            false,
            false,
            false,
        ),
        ("inverted", Path::Inverted, 20.0137, false, false, false),
        ("offset", Path::Helix, 20.0137, true, false, false),
        ("shared", Path::Bank, 20.0137, false, true, false),
        ("suspended", Path::Bank, 20.0137, false, false, true),
    ] {
        let r = rail(path);
        let m = model(path, lead, offset, shared, suspended);
        for kind in [Kind::B0, Kind::B1] {
            let (p, it) = m.initialize(&r, kind).unwrap();
            let e = m.evaluate(&r, &p, kind).unwrap();
            assert!(max_abs(&e.c) < 2e-11);
            rank(&format!("{name}-{kind:?}"), &e.j, e.j.len());
            assert_eq!(6 * p.len() - e.j.len(), if suspended { 2 } else { 1 });
            let b1 = m.evaluate(&r, &p, Kind::B1).unwrap();
            let gap = max_abs(&b1.c[..b1.rail_rows]);
            let (center, rolling) = wheel_errors(&m, &r, &p);
            let articulation = m
                .bearings
                .iter()
                .map(|b| {
                    p[b.a]
                        .orientation
                        .conjugate()
                        .compose(p[b.b].orientation)
                        .rotation_vector()
                        .unwrap()
                        .length()
                })
                .fold(0., f64::max);
            println!(
                "M4B_COMPARE {name} {kind:?} bodies={} rows={} iterations={it} residual={:.9e} support_gap={gap:.9e} wheel_center={center:.9e} rolling={rolling:.9e} articulation={articulation:.9e}",
                p.len(),
                e.j.len(),
                max_abs(&e.c)
            );
        }
    }
}

#[test]
fn finite_continuation_does_not_mistake_local_rank_for_traversal() {
    for path in [Path::Turn, Path::Bank, Path::Helix, Path::Loop] {
        let r = rail(path);
        let mut worst = 0_f64;
        for i in 0..24 {
            let lead = 5.0137 + i as f64 * 1.9;
            let m = model(path, lead, false, false, false);
            let (p, _) = m.initialize(&r, Kind::B1).unwrap();
            worst = worst.max(max_abs(&m.evaluate(&r, &p, Kind::B1).unwrap().c));
        }
        println!("M4B_FINITE {path:?} independent_poses=24 max_residual={worst:.9e}");
    }
}

#[test]
fn redundant_contradictory_near_singular_and_bearing_limit_failures() {
    let r = rail(Path::Straight);
    let mut m = model(Path::Straight, 20.0137, false, false, false);
    let e = m.evaluate(&r, &m.poses, Kind::B1).unwrap();
    let mut j = e.j.clone();
    j.push(j[0].clone());
    assert_eq!(nullspace(&j, 1e-9).0, e.j.len());
    assert!(matches!(
        correction(&j, &m.inverse_mass(), &vec![0.; j.len()]),
        Err(Failure::Singular)
    ));
    rank("redundant-running", &j, e.j.len());
    let mut opposite = j.clone();
    for v in opposite.last_mut().unwrap() {
        *v = -*v;
    }
    rank("opposed-bilateral", &opposite, e.j.len());
    // Two coincident points assigned planes .01 m apart: globally inconsistent.
    let mut bad = m.supports[0];
    bad.surface.offset.y += 0.01;
    m.supports.push(bad);
    assert!(m.initialize(&r, Kind::B1).is_err());
    m.supports.pop();
    for s in &mut m.supports {
        s.local.x = 0.;
    }
    let e = m.evaluate(&r, &m.poses, Kind::B1).unwrap();
    assert!(nullspace(&e.j, 1e-7).0 < e.j.len());
    rank("collapsed-footprint", &e.j, nullspace(&e.j, 1e-7).0);
    for s in &mut m.supports {
        s.local.x = if s.station > 20.0137 { 1e-10 } else { -1e-10 };
    }
    let e = m.evaluate(&r, &m.poses, Kind::B1).unwrap();
    assert!(matches!(
        correction(&e.j, &m.inverse_mass(), &vec![0.; e.j.len()]),
        Err(Failure::Singular)
    ));
    let r = rail(Path::Turn);
    let mut m = model(Path::Turn, 20.0137, false, false, false);
    m.bearings[0].max_swing = 0.001;
    assert!(matches!(
        m.initialize(&r, Kind::B1),
        Err(Failure::BearingLimit)
    ));
}

#[test]
fn bounded_targets_endpoints_corners_ambiguity_and_explicit_circuit() {
    let r = rail(Path::Straight);
    let s = support(Role::Running);
    assert_eq!(
        r.project(s, Vec3::new(-0.1, 0., 0.6), 0.1, 0.2)
            .unwrap_err(),
        Failure::Endpoint
    );
    assert_eq!(
        r.project(s, Vec3::new(22., 0., 0.6), 20., 0.2).unwrap_err(),
        Failure::Inaccessible
    );
    assert_eq!(
        r.project(s, Vec3::ZERO, f64::NAN, 0.2).unwrap_err(),
        Failure::Invalid
    );
    assert_eq!(
        r.project(s, Vec3::ZERO, 20., 3.).unwrap_err(),
        Failure::Invalid
    );
    let bank = rail(Path::Bank);
    let x = bank.jet(s, 20.).unwrap().p;
    assert_eq!(bank.project(s, x, 20., 0.4).unwrap_err(), Failure::Corner);
    let length = 16. * std::f64::consts::PI;
    let circle = || {
        rail_fixtures::sampled(length, 0.025, |s| {
            let p = Path::Loop.at(s);
            (p.position_m, p.orientation)
        })
    };
    let closed = Rail::new(circle(), true).unwrap();
    let mut tolerated = circle();
    tolerated.samples.last_mut().unwrap().position_m.y += 0.0005;
    assert!(matches!(Rail::new(tolerated, true), Err(Failure::Corner)));
    let target = closed.jet(s, length + 0.0137).unwrap();
    let (station, _) = closed.project(s, target.p, length + 0.01, 0.2).unwrap();
    assert!((station - length - 0.0137).abs() < 1e-9);
    let open = Rail::new(circle(), false).unwrap();
    assert_eq!(open.jet(s, length + 0.0137).unwrap_err(), Failure::Endpoint);
    assert!(Rail::new(r.track, true).is_err());
    // Two explicit laps in a small fixture: equal local minima on distinct stations.
    let repeated = Rail::new(
        rail_fixtures::sampled(4. * std::f64::consts::PI * 0.2, 0.005, |s| {
            rail_fixtures::helix(s, 0.2, 0., 0.)
        }),
        false,
    )
    .unwrap();
    let mut surface = s;
    surface.offset = Vec3::ZERO;
    let x = repeated.jet(surface, 0.11).unwrap().p;
    assert_eq!(
        repeated.project(surface, x, 1.0, 1.).unwrap_err(),
        Failure::Ambiguous
    );
}

#[test]
fn individual_carrier_mobility_and_frozen_car_have_no_invented_freedom() {
    let r = rail(Path::Bank);
    let m = model(Path::Bank, 20.0137, false, false, false);
    for kind in [Kind::B0, Kind::B1] {
        let (p, _) = m.initialize(&r, kind).unwrap();
        let e = m.evaluate(&r, &p, kind).unwrap();
        let fixed: Matrix = e.j.iter().map(|r| r[..36].to_vec()).collect();
        rank(&format!("frozen-car-{kind:?}"), &fixed, 36);
    }
    let r = rail(Path::Straight);
    let mut m = model(Path::Straight, 20.0137, false, false, false);
    for (name, x, z, expected) in [
        ("carrier", 0.12, 0.6, 5),
        ("tiny-footprint", 1e-10, 0.6, 3),
        ("zero-footprint", 0., 0.6, 3),
        ("zero-gauge", 0.12, 0., 4),
    ] {
        for s in &mut m.supports[..5] {
            s.local.x = if s.local.x < 0. {
                -x
            } else if s.local.x > 0. {
                x
            } else {
                0.
            };
            s.local.z = if s.local.z < 0. { -z } else { z };
        }
        // Restore authored signs for each independently configured experiment.
        m.supports[0].local.x = -x;
        m.supports[1].local.x = x;
        m.supports[3].local.x = -x;
        m.supports[4].local.x = x;
        m.supports[0].local.z = -z;
        m.supports[1].local.z = -z;
        let e = m.evaluate(&r, &m.poses, Kind::B1).unwrap();
        let local: Matrix = e.j[..5].iter().map(|r| r[6..12].to_vec()).collect();
        rank(name, &local, expected);
    }
}

#[test]
fn explicit_bearing_frames_and_independent_analytic_gap_witness() {
    let r = rail(Path::Straight);
    let mut m = model(Path::Straight, 20.0137, true, false, false);
    for b in &mut m.bearings {
        b.frame_a = Quaternion::axis_angle(Vec3::Z, 0.35).unwrap();
        b.frame_b = b.frame_a;
    }
    let d: Vec<_> = (0..42).map(|i| 0.002 * (i as f64).sin()).collect();
    let p = m.shifted(&m.poses, &d);
    let e = m.evaluate(&r, &p, Kind::B1).unwrap();
    let fd = finite_jacobian(42, 1e-6, |d| {
        m.evaluate(&r, &m.shifted(&p, d), Kind::B1).unwrap().c
    });
    assert!(difference(&e.j, &fd) < 1e-8);
    // Independent trigonometric surfaces and golden-section target search,
    // neither Hermite jets nor our station Newton/bisection equation.
    for path in [
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let r = rail(path);
        let m = model(path, 20.0137, false, false, false);
        let (p, _) = m.initialize(&r, Kind::B1).unwrap();
        let mut worst = 0_f64;
        for support in &m.supports {
            let x = point(p[support.owner], support.local);
            let offset = support.surface.offset;
            let at = |s| path.locus(s, offset.y, offset.z);
            let distance = |s| {
                let d = x - at(s);
                d.dot(d)
            };
            let (mut a, mut b) = (support.station - 0.7, support.station + 0.7);
            for _ in 0..90 {
                let l = a + (b - a) * 0.3819660112501051;
                let r = a + (b - a) * 0.6180339887498949;
                if distance(l) < distance(r) {
                    b = r;
                } else {
                    a = l;
                }
            }
            let s = (a + b) * 0.5;
            let tangent = (at(s + 1e-5) - at(s - 1e-5)) / 2e-5;
            // Basis vectors are independent trigonometric offsets, not Frame.
            let normal = path.locus(s, 1., 0.) - path.locus(s, 0., 0.);
            let lateral = path.locus(s, 0., 1.) - path.locus(s, 0., 0.);
            let n = match support.surface.role {
                Role::Running => lateral.cross(tangent),
                Role::Side => tangent.cross(normal),
            }
            .normalized()
            .unwrap();
            worst = worst.max((x - at(s)).dot(n).abs());
        }
        println!("M4B_ANALYTIC {path:?} max_gap={worst:.9e}");
        // Independent smooth analytic geometry differs from sampled Hermite/frame geometry.
        assert!(worst < 2e-6);
    }
}

#[test]
fn seating_impulses_are_distinct_from_speed_squared_support() {
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
    ] {
        let r = rail(path);
        let m = model(path, 20.0137, false, false, false);
        let mut reaction = vec![];
        for speed in [0., 3., 6.] {
            let (mut state, _) = dynamics::initial(&m, &r, speed).unwrap();
            let e = m.evaluate(&r, &state.poses, Kind::B1).unwrap();
            let consistent = max_abs(&multiply(&e.j, &state.velocity));
            let mut naive = state.velocity.clone();
            for (i, body) in naive.as_chunks_mut::<6>().0.iter_mut().enumerate() {
                let tangent = Frame::new(state.poses[i].orientation).unwrap().tangent();
                body.copy_from_slice(&[
                    speed * tangent.x,
                    speed * tangent.y,
                    speed * tangent.z,
                    0.,
                    0.,
                    0.,
                ]);
            }
            let mismatch = multiply(&e.j, &naive);
            let (_, seating) = correction(
                &e.j,
                &m.inverse_mass(),
                &mismatch.iter().map(|x| -x).collect::<Vec<_>>(),
            )
            .unwrap();
            let (_, force) = dynamics::instantaneous(&m, &r, &state, Vec3::ZERO).unwrap();
            reaction.push(force);
            let first = dynamics::step(&m, &r, &mut state, Vec3::ZERO).unwrap();
            println!(
                "M4B_SEATING {path:?} speed={speed} consistent_Jv={consistent:.9e} naive_Jv={:.9e} naive_max_impulse_Ns={:.9e} first_force_N_or_Nm={:.9e}",
                max_abs(&mismatch),
                max_abs(&seating),
                max_abs(&first.loads)
            );
            if matches!(path, Path::Straight) {
                // Roundoff-sized anchor errors (~1e-14 m) are amplified by
                // Baumgarte/dt². Gate the resulting sub-micronewton force.
                assert!(max_abs(&first.loads) < 1e-6);
            }
        }
        assert!(max_abs(&reaction[0]) < 1e-9);
        assert!(
            max_abs(
                &reaction[2]
                    .iter()
                    .zip(&reaction[1])
                    .map(|(a, b)| a - 4. * b)
                    .collect::<Vec<_>>()
            ) < 1e-5
        );
    }
}

#[test]
#[ignore = "native Release setup / solve / rigid-body tick characterization"]
fn research_phase_timing() {
    use std::{hint::black_box, time::Instant};
    fn measure(mut f: impl FnMut()) -> (f64, f64, f64) {
        f();
        let mut times = vec![];
        for _ in 0..7 {
            let t = Instant::now();
            for _ in 0..5 {
                f();
            }
            times.push(t.elapsed().as_secs_f64() * 1e6 / 5.);
        }
        times.sort_by(f64::total_cmp);
        (times[3], times[0], times[6])
    }
    for path in [Path::Straight, Path::Bank, Path::Helix] {
        let setup = measure(|| {
            black_box((rail(path), model(path, 20.0137, true, false, false)));
        });
        let r = rail(path);
        let m = model(path, 20.0137, true, false, false);
        for kind in [Kind::B0, Kind::B1] {
            let solve = measure(|| {
                black_box(m.initialize(&r, kind).unwrap());
            });
            println!("M4B_COST {path:?} {kind:?} setup_us={setup:?} initialize_us={solve:?}");
        }
        let (state, _) = dynamics::initial(&m, &r, 3.).unwrap();
        let tick = measure(|| {
            let mut state = state.clone();
            black_box(dynamics::step(&m, &r, &mut state, Vec3::new(0., -9.81, 0.)).unwrap());
        });
        println!("M4B_COST {path:?} B1 tick_us={tick:?}");
    }
}

#[test]
fn shared_carrier_is_one_body_in_the_isolated_impulse_graph() {
    let r = rail(Path::Bank);
    let m = model(Path::Bank, 20.0137, true, true, false);
    assert_eq!(m.poses.len(), 11);
    assert_eq!(m.axles.len(), 6);
    assert_eq!(m.supports.len(), 30);
    let (mut state, _) = dynamics::initial(&m, &r, 3.).unwrap();
    let mut repeat = state.clone();
    let mut worst = 0_f64;
    for _ in 0..8 {
        let report = dynamics::step(&m, &r, &mut state, Vec3::new(0., -9.81, 0.)).unwrap();
        assert_eq!(
            report,
            dynamics::step(&m, &r, &mut repeat, Vec3::new(0., -9.81, 0.)).unwrap()
        );
        assert_eq!(state, repeat);
        worst = worst.max(report.max_position);
    }
    assert!(worst < 2e-5);
    println!("M4B_SHARED bodies=11 axles=6 steps=8 max_residual={worst:.9e}");
}
