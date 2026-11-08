//! M4A feasibility study. Everything here is test-only and explicitly synthetic.
#[path = "support/articulation.rs"]
mod baseline;
#[path = "support/bogie_fixtures.rs"]
mod carrier;
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[path = "support/m4a_matrix.rs"]
mod matrix;
#[path = "support/m4a_mechanisms.rs"]
mod mechanisms;
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;

use guidance::*;
use matrix::*;
use mechanisms::{Mechanism, Path};
use qx_document::train::{DevelopmentArrangement, Train, mechanism::JointModel};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    rail_geometry::{
        PhysicalRailGeometry, RailReference, SearchStatus, StationWindow, WheelCenterPath,
    },
    topology::LayoutMode,
};
use qx_physics::{
    TrackAttitude, TrackGuide, World, assembly_diagnostics::velocity_residuals, consist::initialize,
};

fn rank(name: &str, a: &Matrix, expected: usize) {
    for t in [1e-5, 1e-7, 1e-9] {
        assert_eq!(nullspace(a, t).0, expected, "{name} at {t}");
    }
    emit(name, a, expected);
    println!(
        "M4A_RANK {name}: rows={} columns={} rank={expected} mobility={} dependent_rows={}",
        a.len(),
        a[0].len(),
        a[0].len() - expected,
        a.len() - expected
    );
}
fn matrix_error(a: &Matrix, b: &Matrix) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
        .fold(0., f64::max)
}
fn check_existing_velocity_rows(
    world: &World,
    track: &qx_geometry::solver::GeneratedTrack,
    g: &[TrackGuide],
    a: &[TrackAttitude],
    j: &Matrix,
) {
    let mut w = world.clone();
    for k in 0..6 * w.bodies.len() + 1 {
        let mut v = vec![0.; 6 * w.bodies.len()];
        if k < v.len() {
            v[k] = 1.;
        } else {
            for (i, x) in v.iter_mut().enumerate() {
                *x = (i as f64 * 1.37).sin();
            }
        }
        for (b, v) in w.bodies.iter_mut().zip(v.as_chunks::<6>().0) {
            b.linear_velocity_mps = Vec3::new(v[0], v[1], v[2]);
            b.angular_velocity_radps = Vec3::new(v[3], v[4], v[5]);
        }
        let rows = multiply(j, &v);
        let mut i = 0;
        let mut joint = 0_f64;
        let mut angular = 0_f64;
        let mut guide = 0_f64;
        for _ in &w.joints {
            joint = joint.max(Vec3::new(rows[i], rows[i + 1], rows[i + 2]).length());
            i += 3;
        }
        for edge in &w.angular_joints {
            let count = match edge.model {
                JointModel::RollLock { .. } => 1,
                JointModel::Fixed { .. } => 3,
                _ => 2,
            };
            angular = angular.max(dot(&rows[i..i + count], &rows[i..i + count]).sqrt());
            i += count;
        }
        for _ in g {
            guide = guide.max(rows[i].hypot(rows[i + 1]));
            i += 2;
        }
        let rail = max_abs(&rows[i..]);
        let real = velocity_residuals(&w, track, g, a).unwrap();
        for (x, y) in [
            (joint, real.max_joint_anchor_speed_mps),
            (angular, real.max_angular_constraint_speed_radps),
            (guide, real.max_guide_transverse_speed_mps),
            (rail, real.max_rail_twist_speed_radps),
        ] {
            assert!((x - y).abs() < 2e-11, "{x} vs {y}");
        }
    }
}

#[test]
fn current_straight_mobility_and_independent_derivatives() {
    let track = baseline::straight();
    for (name, train, expected) in [
        ("A-conventional", baseline::conventional(1), 17),
        (
            "A-suspended",
            Train::development_consist("suspended".into(), 1, DevelopmentArrangement::BelowTrack)
                .unwrap(),
            16,
        ),
        ("A-added-carrier", carrier::carrier(false), 22),
    ] {
        let (world, g, a, _) =
            initialize(&train, &track, 25., Vec3::new(0., -9.81, 0.), 32, 4).unwrap();
        let j = current(&world, &track, &g, &a);
        rank(name, &j, expected);
        check_existing_velocity_rows(&world, &track, &g, &a, &j);
        for h in [1e-4, 1e-5, 1e-6] {
            let fd = finite_jacobian(j[0].len(), h, |d| straight_residual(&world, &g, &a, d));
            assert!(matrix_error(&j, &fd) < 2e-8, "{name} h={h}");
        }
        // Fix ALL car/other poses: count only actual child-coordinate columns.
        let body = if name == "A-added-carrier" { 3 } else { 1 };
        let child: Matrix = j
            .iter()
            .map(|r| r[6 * body..6 * body + 6].to_vec())
            .collect();
        let expected = if name == "A-added-carrier" { 5 } else { 6 };
        rank(&format!("{name}-parent-fixed"), &child, expected);
    }
}

#[test]
fn current_guides_curved_rows_and_single_pivot_do_not_invent_freedom() {
    for (name, track) in [
        ("curve", baseline::arc(8., 0., 70., false)),
        ("bank", baseline::arc(12., 0.6, 70., true)),
    ] {
        let (w, g, a, _) =
            initialize(&baseline::conventional(1), &track, 25., Vec3::ZERO, 32, 4).unwrap();
        let j = current(&w, &track, &g, &a);
        check_existing_velocity_rows(&w, &track, &g, &a, &j);
        let gear: Matrix = j.iter().skip(7).map(|r| r[6..12].to_vec()).collect();
        // Only rail/guide rows, excluding suspension. The gear retains one
        // path-travel coordinate, which includes curvature-induced rotation.
        rank(&format!("A-{name}-gear-guidance"), &gear, 5);
        let child: Matrix = j.iter().map(|r| r[6..12].to_vec()).collect();
        rank(&format!("A-{name}-parent-fixed"), &child, 6);
        println!(
            "M4A_CURRENT {name}: full ranks at 1e-3/1e-5/1e-7 = {:?}; residual-dependent near singular values are NOT physical mobility",
            [1e-3, 1e-5, 1e-7].map(|t| nullspace(&j, t).0)
        );
    }
    let one = vec![
        point_row(1, 0, Vec3::ZERO, Vec3::Y),
        point_row(1, 0, Vec3::ZERO, Vec3::Z),
        row(1, 0, Vec3::ZERO, Vec3::X),
    ];
    rank("A-single-pivot-guide-plus-roll", &one, 3);
    let (_, null) = nullspace(&one, 1e-9);
    // Longitudinal translation, pitch and yaw remain; latter two are not
    // evidence that a finite wheel set remains on rails.
    assert_eq!(null.len(), 3);
    let p = perturb(
        qx_physics::Pose {
            position_m: Vec3::ZERO,
            orientation: Quaternion::IDENTITY,
        },
        &[0., 0., 0., 0., 0.1, 0.],
    );
    assert!(point(p, Vec3::new(0.3, 0.18, 0.6)).z - 0.6 < -0.02);
}

/// Eliminate massless rail parameters from the six carriage equations. This
/// yields 2 transverse + 3 orientation rows, including curvature/bank LINEAR
/// terms. No target orientation is assigned during dynamics.
fn condensed(m: &Mechanism, augmented: &Matrix) -> Matrix {
    let n = 6 * m.poses.len();
    let mut rows = vec![];
    for (i, &(_, s)) in m.axles.iter().enumerate() {
        let f = Frame::new(m.path.at(s).orientation).unwrap();
        let xyz = &augmented[6 * i..6 * i + 3];
        for normal in [f.normal(), f.lateral()] {
            rows.push(
                (0..n)
                    .map(|c| normal.x * xyz[0][c] + normal.y * xyz[1][c] + normal.z * xyz[2][c])
                    .collect(),
            );
        }
        let tangent = f.tangent();
        let trow: Vec<_> = (0..n)
            .map(|c| tangent.x * xyz[0][c] + tangent.y * xyz[1][c] + tangent.z * xyz[2][c])
            .collect();
        for k in 0..3 {
            let source = &augmented[6 * i + 3 + k];
            let rate = -source[n + i];
            rows.push((0..n).map(|c| source[c] - rate * trow[c]).collect());
        }
    }
    rows.extend(
        augmented[6 * m.axles.len()..]
            .iter()
            .map(|r| r[..n].to_vec()),
    );
    rows
}

#[test]
fn independent_carriage_bearing_witnesses_close_across_the_track_matrix() {
    for (name, path, lead, suspended, offset, shared) in [
        ("B-straight", Path::Straight, 20., false, Vec3::ZERO, false),
        ("B-tight-turn", Path::Turn, 20., false, Vec3::ZERO, false),
        (
            "B-bank-transition",
            Path::Bank,
            20.,
            false,
            Vec3::ZERO,
            false,
        ),
        ("B-helix", Path::Helix, 20., false, Vec3::ZERO, false),
        (
            "B-loop-top",
            Path::Loop,
            8. * std::f64::consts::PI,
            false,
            Vec3::ZERO,
            false,
        ),
        (
            "B-loop-vertical",
            Path::Loop,
            4. * std::f64::consts::PI,
            false,
            Vec3::ZERO,
            false,
        ),
        ("B-inverted", Path::Inverted, 20., false, Vec3::ZERO, false),
        ("B-suspended", Path::Bank, 20., true, Vec3::ZERO, false),
        (
            "B-offset",
            Path::Helix,
            20.,
            false,
            Vec3::new(0.05, 0.12, 0.03),
            false,
        ),
        ("B-shared", Path::Bank, 20., false, Vec3::ZERO, true),
    ] {
        let m = Mechanism::new(path, lead, suspended, offset, shared);
        let zero = vec![0.; m.coordinates()];
        let closure = max_abs(&m.residual(&zero));
        assert!(closure < 1e-12, "{name}: {closure}");
        let j = finite_jacobian(m.coordinates(), 1e-5, |d| m.residual(d));
        let finer = finite_jacobian(m.coordinates(), 1e-6, |d| m.residual(d));
        assert!(matrix_error(&j, &finer) < 2e-8, "{name}");
        let mobility = if suspended { 2 } else { 1 };
        rank(name, &j, m.coordinates() - mobility);
        let reduced = condensed(&m, &j);
        rank(
            &format!("{name}-condensed"),
            &reduced,
            6 * m.poses.len() - mobility,
        );
        if name == "B-tight-turn" {
            let car = m.cars[0];
            let fixed: Matrix = reduced
                .iter()
                .map(|row| {
                    row.iter()
                        .enumerate()
                        .filter(|(i, _)| *i < 6 * car || *i >= 6 * car + 6)
                        .map(|(_, x)| *x)
                        .collect()
                })
                .collect();
            rank("B-tight-turn-car-fixed", &fixed, 6 * (m.poses.len() - 1));
        }
        // A separate continuation derivative verifies a finite family, not
        // merely a first-order flex at an isolated singular pose.
        let h = 1e-4;
        let plus = Mechanism::new(path, lead + h, suspended, offset, shared);
        let minus = Mechanism::new(path, lead - h, suspended, offset, shared);
        let mut v = vec![0.; m.coordinates()];
        for (i, ((a, b), p)) in plus
            .poses
            .iter()
            .zip(&minus.poses)
            .zip(&m.poses)
            .enumerate()
        {
            let linear = (a.position_m - b.position_m) / (2. * h);
            let angular = a
                .orientation
                .compose(b.orientation.conjugate())
                .rotation_vector()
                .unwrap()
                / (2. * h);
            assert!(p.position_m.is_finite());
            v[6 * i..6 * i + 6].copy_from_slice(&[
                linear.x, linear.y, linear.z, angular.x, angular.y, angular.z,
            ]);
        }
        for i in 0..m.axles.len() {
            v[6 * m.poses.len() + i] = (plus.axles[i].1 - minus.axles[i].1) / (2. * h);
        }
        let velocity = max_abs(&multiply(&j, &v));
        assert!(velocity < 1e-7, "{name}: {velocity}");
        let relative = m.poses[m.cars[0]]
            .orientation
            .conjugate()
            .compose(m.poses[m.bogies[0]].orientation)
            .rotation_vector()
            .unwrap()
            .length();
        println!(
            "M4A_WITNESS {name}: bodies={} axles={} closure={closure:.6e} Jv={velocity:.6e} car_bogie_relative_rad={relative:.6e}",
            m.poses.len(),
            m.axles.len()
        );
    }
}

#[test]
fn independent_wheel_centers_improve_but_direction_errors_remain_explicit() {
    for path in [
        Path::Straight,
        Path::Turn,
        Path::Bank,
        Path::Helix,
        Path::Loop,
        Path::Inverted,
    ] {
        let lead = if matches!(path, Path::Loop) {
            8. * std::f64::consts::PI
        } else {
            20.
        };
        let m = Mechanism::new(path, lead, false, Vec3::ZERO, false);
        if matches!(path, Path::Helix) {
            let (position, orientation) = rail_fixtures::helix(lead, 8., 0.3, 0.4 + 0.04 * lead);
            assert!((position - path.at(lead).position_m).length() < 1e-12);
            assert!(
                (orientation.conjugate().compose(path.at(lead).orientation))
                    .rotation_vector()
                    .unwrap()
                    .length()
                    < 1e-12
            );
            assert!(
                (rail_fixtures::helix_locus(
                    lead,
                    8.,
                    0.3,
                    0.4 + 0.04 * lead,
                    Vec3::new(0., 0.18, 0.6)
                ) - path.locus(lead, 0.18, 0.6))
                .length()
                    < 1e-12
            );
        }
        let track = rail_fixtures::sampled(50., 0.005, |s| {
            let p = path.at(s);
            (p.position_m, p.orientation)
        });
        let rails = PhysicalRailGeometry::new(&track, LayoutMode::Open).unwrap();
        let mut center_error = 0_f64;
        let mut sample_error = 0_f64;
        let mut angle = 0_f64;
        for &(body, s) in &m.axles {
            for z in [-0.6, 0.6] {
                let p = point(m.poses[body], Vec3::new(0., 0.18, z));
                center_error = center_error.max((p - path.locus(s, 0.18, z)).length());
                let locus = WheelCenterPath {
                    rail: RailReference {
                        id: format!("synthetic-{z}"),
                        offset_m: Vec3::Z * z,
                    },
                    center_offset_m: Vec3::Y * 0.18,
                };
                let sample = rails.sample(&locus, s).unwrap();
                sample_error = sample_error.max((p - sample.wheel_center_m).length());
                let query = rails
                    .nearest(
                        &locus,
                        p,
                        StationWindow {
                            min_m: s - 0.2,
                            max_m: s + 0.2,
                        },
                        Default::default(),
                    )
                    .unwrap();
                assert_eq!(query.status, SearchStatus::Resolved);
                assert!((p - query.candidates[0].sample.wheel_center_m).length() < 5e-6);
                let h = 1e-5;
                let t = (path.locus(s + h, 0.18, z) - path.locus(s - h, 0.18, z))
                    .normalized()
                    .unwrap();
                let rolling = Frame::new(m.poses[body].orientation).unwrap().tangent();
                angle = angle.max(rolling.cross(t).length().atan2(rolling.dot(t)));
            }
        }
        assert!(
            center_error < 1e-12 && sample_error < 1e-7,
            "{path:?}: analytic={center_error} sampler={sample_error}"
        );
        let train = baseline::conventional(1);
        let poses = qx_physics::consist::inspect(&train, &track, lead).unwrap();
        let wheels = carrier::wheels(&train, &poses, false);
        let old = qx_physics::wheel_geometry::diagnose(
            &train,
            &poses,
            &rails,
            &wheels,
            Default::default(),
        )
        .unwrap();
        println!(
            "M4A_WHEELS {path:?}: A_rms={:.9e} A_max={:.9e} B_analytic_max={center_error:.9e} B_sample_max={sample_error:.9e} B_direction_max_rad={angle:.9e}",
            old.rms_error_m.unwrap(),
            old.max_error_m.unwrap()
        );
        if matches!(path, Path::Bank | Path::Helix) {
            assert!(angle > 0.001);
        }
    }
}

#[test]
fn scalar_wheel_normal_rows_have_five_directions_and_sensible_reactions() {
    // Three running-wheel normal rows and two side-guide rows. Ideal BILATERAL
    // gap equalities on authored planar test surfaces, NOT complete contacts.
    let supports = [
        (Vec3::new(-0.3, 0., -0.6), Vec3::Y),
        (Vec3::new(0.3, 0., -0.6), Vec3::Y),
        (Vec3::new(0., 0., 0.6), Vec3::Y),
        (Vec3::new(-0.3, 0., 0.), Vec3::Z),
        (Vec3::new(0.3, 0., 0.), Vec3::Z),
    ];
    let j: Matrix = supports
        .iter()
        .map(|&(p, n)| point_row(1, 0, p, n))
        .collect();
    rank("B-five-normal-rows", &j, 5);
    let fd = finite_jacobian(6, 1e-5, |d| {
        supports
            .iter()
            .map(|&(p, n)| {
                n.dot(
                    point(
                        perturb(
                            qx_physics::Pose {
                                position_m: Vec3::ZERO,
                                orientation: Quaternion::IDENTITY,
                            },
                            d,
                        ),
                        p,
                    ) - p,
                )
            })
            .collect()
    });
    assert!(matrix_error(&j, &fd) < 1e-10);
    let minv = [0.01, 0.01, 0.01, 0.05, 0.05, 0.05];
    let k: Matrix = j
        .iter()
        .map(|a| {
            j.iter()
                .map(|b| a.iter().zip(b).zip(minv).map(|((a, b), m)| a * b * m).sum())
                .collect()
        })
        .collect();
    // 100 kg, isotropic inertia 20 kg m², gravity -Y. No seating correction.
    let free = [0., -9.81, 0., 0., 0., 0.];
    let rhs: Vec<_> = multiply(&j, &free).into_iter().map(|x| -x).collect();
    let lambda = solve(&k, &rhs);
    let mut reaction = vec![0.; 6];
    for (r, l) in j.iter().zip(&lambda) {
        for (x, a) in reaction.iter_mut().zip(r) {
            *x += a * l;
        }
    }
    let acceleration: Vec<_> = free
        .iter()
        .zip(&reaction)
        .zip(minv)
        .map(|((f, r), m)| f + r * m)
        .collect();
    assert!(max_abs(&multiply(&j, &acceleration)) < 1e-12);
    assert!((reaction[1] - 981.).abs() < 1e-10);
    let (_, basis) = nullspace(&j, 1e-9);
    assert!(dot(&reaction, &basis[0]).abs() < 1e-12);
    // Bilateral reactions may pull. Inversion requires up-stop contact rather
    // than accepting negative running-wheel normal forces as physical contact.
    assert!(lambda[..3].iter().all(|x| *x > 0.));
    let inverted = solve(&k, &rhs.iter().map(|x| -x).collect::<Vec<_>>());
    assert!(inverted[..3].iter().all(|x| *x < 0.));
    println!(
        "M4A_REACTION normal_loads={lambda:?} inverted_running_only={inverted:?} net_wrench={reaction:?}"
    );

    let mut redundant = j.clone();
    redundant.push(j[0].iter().map(|x| -x).collect());
    rank("C-running-plus-zero-clearance-upstop", &redundant, 5);
    let all_centers: Matrix = [-0.3, 0.3]
        .into_iter()
        .flat_map(|x| {
            [-0.6, 0.6].into_iter().flat_map(move |z| {
                [Vec3::Y, Vec3::Z]
                    .into_iter()
                    .map(move |n| point_row(1, 0, Vec3::new(x, 0.18, z), n))
            })
        })
        .collect();
    rank("C-four-wheel-center-loci-straight", &all_centers, 5);
    // Too little gauge or wheelbase is a genuine rank/conditioning issue.
    for (name, a, g, expected) in [
        ("zero-wheelbase", 0., 0.6, 3),
        ("zero-gauge", 0.3, 0., 4),
        ("tiny-gauge", 0.3, 1e-10, 4),
    ] {
        let j: Matrix = [
            (Vec3::new(-a, 0., -g), Vec3::Y),
            (Vec3::new(a, 0., -g), Vec3::Y),
            (Vec3::new(0., 0., g), Vec3::Y),
            (Vec3::new(-a, 0., 0.), Vec3::Z),
            (Vec3::new(a, 0., 0.), Vec3::Z),
        ]
        .iter()
        .map(|&(p, n)| point_row(1, 0, p, n))
        .collect();
        rank(name, &j, expected);
    }
}

#[test]
fn incompatibility_and_second_order_flex_are_not_counted_as_articulation() {
    // Redundant straight rows can become inconsistent off straight track.
    // Four target centers on a circular pair of rails have different same-rail
    // chord lengths at fixed station rows, while both rigid local pairs are .6m.
    let path = Path::Turn;
    let center = 20.;
    let distances = [-0.6, 0.6]
        .map(|z| (path.locus(center + 0.3, 0.18, z) - path.locus(center - 0.3, 0.18, z)).length());
    assert!((distances[0] - distances[1]).abs() > 0.04);
    println!(
        "M4A_INCOMPATIBLE same_rail_target_distances={distances:?} rigid_pair=.6 (fixed-station witness only)"
    );
    let m = Mechanism::new(Path::Bank, 20., false, Vec3::ZERO, false);
    let rear = &m.edges[3];
    let roll_gap = twist(m.poses[rear.a].orientation, m.poses[rear.b].orientation).abs();
    assert!(roll_gap > 0.001);
    println!("M4A_INCOMPATIBLE additional_rear_roll_lock_gap_rad={roll_gap:.9e}");
    // A tangent rigid bar joining two path points at exactly its diameter has
    // a first-order relative-station null direction but fails finite closure.
    let radius = 8_f64;
    let gap =
        |d: f64| 2. * radius * ((std::f64::consts::PI + d / radius) * 0.5).sin() - 2. * radius;
    let derivative = (gap(1e-4) - gap(-1e-4)) / (2e-4);
    assert!(derivative.abs() < 1e-10 && gap(0.01) < -1e-6);
    println!(
        "M4A_SINGULAR diameter_first_derivative={derivative:.9e} finite_gap={:.9e}",
        gap(0.01)
    );
}

#[test]
fn current_impulse_reactions_remain_real_and_repeatable() {
    let track = baseline::straight();
    let train = baseline::conventional(1);
    let (mut w, mut g, mut a, _) =
        initialize(&train, &track, 25., Vec3::new(0., -9.81, 0.), 32, 4).unwrap();
    let mut twin = w.clone();
    let mut tg = g.clone();
    let mut ta = a.clone();
    let mut force = Vec3::ZERO;
    for _ in 0..8 {
        let momentum_before = w.bodies.iter().fold(Vec3::ZERO, |sum, b| {
            sum + b.linear_velocity_mps * b.mass.mass_kg
        });
        let d = w.step_on_track(&track, &mut g, &mut a).unwrap();
        assert_eq!(d, twin.step_on_track(&track, &mut tg, &mut ta).unwrap());
        assert_eq!(w, twin);
        force = d
            .guide_force_n
            .iter()
            .copied()
            .fold(Vec3::ZERO, |a, b| a + b)
            + d.attitude_force_n
                .iter()
                .copied()
                .fold(Vec3::ZERO, |a, b| a + b);
        let momentum_after = w.bodies.iter().fold(Vec3::ZERO, |sum, b| {
            sum + b.linear_velocity_mps * b.mass.mass_kg
        });
        let balance = (momentum_after - momentum_before) / qx_physics::FIXED_STEP_SECONDS
            - (force + Vec3::new(0., -800. * 9.81, 0.));
        assert!(balance.length() < 1e-8);
        println!(
            "M4A_CURRENT_STEP force_y={} joint_gap={} guide_gap={} momentum_balance={}",
            force.y,
            d.max_joint_error_m,
            d.max_guide_error_m,
            balance.length()
        );
    }
    println!("M4A_CURRENT_REACTION total_rail_force_n={force:?}");
}

#[test]
fn experimental_carriage_acceleration_reactions_are_consistent_without_stepping() {
    for path in [Path::Straight, Path::Bank, Path::Helix, Path::Loop] {
        let lead = 20.;
        let h = 0.0005;
        let m = Mechanism::new(path, lead, false, Vec3::ZERO, false);
        let plus = Mechanism::new(path, lead + h, false, Vec3::ZERO, false);
        let minus = Mechanism::new(path, lead - h, false, Vec3::ZERO, false);
        let reduced = |m: &Mechanism| {
            condensed(
                m,
                &finite_jacobian(m.coordinates(), 1e-5, |d| m.residual(d)),
            )
        };
        let j = reduced(&m);
        let jp = reduced(&plus);
        let jm = reduced(&minus);
        let mut v = vec![];
        let mut acceleration = vec![];
        for ((p, a), b) in m.poses.iter().zip(&plus.poses).zip(&minus.poses) {
            let linear = (a.position_m - b.position_m) / (2. * h);
            let angular = a
                .orientation
                .compose(b.orientation.conjugate())
                .rotation_vector()
                .unwrap()
                / (2. * h);
            v.extend([
                linear.x, linear.y, linear.z, angular.x, angular.y, angular.z,
            ]);
            let linear = (a.position_m + b.position_m - p.position_m * 2.) / (h * h);
            let angular = (a
                .orientation
                .compose(p.orientation.conjugate())
                .rotation_vector()
                .unwrap()
                + b.orientation
                    .compose(p.orientation.conjugate())
                    .rotation_vector()
                    .unwrap())
                / (h * h);
            acceleration.extend([
                linear.x, linear.y, linear.z, angular.x, angular.y, angular.z,
            ]);
        }
        let jdot: Matrix = jp
            .iter()
            .zip(&jm)
            .map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b) / (2. * h)).collect())
            .collect();
        let bias = multiply(&jdot, &v);
        let curvature = max_abs(
            &multiply(&j, &acceleration)
                .iter()
                .zip(&bias)
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>(),
        );
        assert!(curvature < 2e-6, "{path:?}: {curvature}");
        assert!(max_abs(&multiply(&j, &v)) < 1e-7);
        // Explicit synthetic masses/inertias: 100kg and isotropic 20kg m²
        // for EACH body. No fictitious mass is attached to rail parameters.
        let minv: Vec<_> = (0..v.len())
            .map(|i| if i % 6 < 3 { 0.01 } else { 0.05 })
            .collect();
        let free: Vec<_> = (0..v.len())
            .map(|i| if i % 6 == 1 { -9.81 } else { 0. })
            .collect();
        let k: Matrix = j
            .iter()
            .map(|a| {
                j.iter()
                    .map(|b| {
                        a.iter()
                            .zip(b)
                            .zip(&minv)
                            .map(|((a, b), m)| a * b * m)
                            .sum()
                    })
                    .collect()
            })
            .collect();
        let rhs: Vec<_> = multiply(&j, &free)
            .iter()
            .zip(&bias)
            .map(|(a, b)| -a - b)
            .collect();
        let lambda = solve(&k, &rhs);
        let mut reaction = vec![0.; v.len()];
        for (row, l) in j.iter().zip(lambda) {
            for (r, a) in reaction.iter_mut().zip(row) {
                *r += a * l;
            }
        }
        let actual: Vec<_> = free
            .iter()
            .zip(&reaction)
            .zip(&minv)
            .map(|((a, r), m)| a + r * m)
            .collect();
        let error = max_abs(
            &multiply(&j, &actual)
                .iter()
                .zip(&bias)
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>(),
        );
        let power = dot(&reaction, &v).abs();
        assert!(error < 1e-9 && power < 1e-5);
        println!(
            "M4A_ACCELERATION {path:?}: curvature_check={curvature:.9e} acceleration_closure={error:.9e} reaction_power_at_unit_station_speed={power:.9e}"
        );
    }
}

#[test]
fn suspended_null_direction_has_finite_bearing_closed_swing() {
    let mut m = Mechanism::new(Path::Bank, 20., true, Vec3::ZERO, false);
    let car = m.cars[0];
    let edges: Vec<_> = m.edges.iter().filter(|e| e.b == car).collect();
    let a = point(m.poses[car], edges[0].local_b);
    let b = point(m.poses[car], edges[1].local_b);
    let rotation = Quaternion::axis_angle(a - b, 0.2).unwrap();
    let original = m.poses[car];
    let orientation = rotation.compose(original.orientation);
    m.poses[car] = qx_physics::Pose {
        position_m: a - Frame::new(orientation)
            .unwrap()
            .local_to_world(edges[0].local_b),
        orientation,
    };
    assert!(max_abs(&m.residual(&vec![0.; m.coordinates()])) < 1e-12);
    assert!((m.poses[car].position_m - original.position_m).length() > 0.1);
}

#[test]
fn characterize_existing_nonzero_speed_startup_without_changing_it() {
    let train = baseline::conventional(2);
    let track = baseline::arc(45., 0.25, 500., true);
    for speed in [0., 12.] {
        let (mut world, mut guides, mut attitudes, inspection) =
            initialize(&train, &track, 200., Vec3::new(0., -9.81, 0.), 32, 4).unwrap();
        // Exactly the existing playback initialization; no velocity projection.
        for (body, component) in world.bodies.iter_mut().zip(&inspection.components) {
            body.linear_velocity_mps = track
                .sample_at_distance(component.station_m)
                .unwrap()
                .frame
                .tangent()
                * speed;
        }
        let before = velocity_residuals(&world, &track, &guides, &attitudes).unwrap();
        let d = world
            .step_on_track(&track, &mut guides, &mut attitudes)
            .unwrap();
        let after = velocity_residuals(&world, &track, &guides, &attitudes).unwrap();
        let joint_force = d
            .joint_force_on_b_n
            .iter()
            .map(|v| v.length())
            .fold(0., f64::max);
        let guide_force = d
            .guide_force_n
            .iter()
            .map(|v| v.length())
            .fold(0., f64::max);
        if speed == 0. {
            assert_eq!(before, Default::default());
        } else {
            assert!(
                before.max_joint_anchor_speed_mps > 0.5
                    && before.max_guide_transverse_speed_mps > 0.1
                    && before.max_rail_twist_speed_radps > 0.01
            );
        }
        assert!(joint_force.is_finite() && guide_force.is_finite());
        println!(
            "M4A_STARTUP speed={speed} before={before:?} after={after:?} max_joint_force_n={joint_force:.9e} max_guide_force_n={guide_force:.9e}"
        );
    }
}
