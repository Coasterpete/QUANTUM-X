#[path = "support/articulation.rs"]
mod articulation;
#[path = "support/bogie_fixtures.rs"]
mod fixtures;
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;

use fixtures::*;
use qx_document::train::mechanism::{JointModel, Placement};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    rail_geometry::PhysicalRailGeometry,
    topology::LayoutMode,
};
use qx_physics::{
    bogie_fitting::{FitOptions, FitReport, FitSeed, FitTermination, frozen_target_linearization},
    wheel_geometry::{TargetObservation, WheelGeometry, WheelTarget},
};
use qx_physics::{consist::inspect, wheel_geometry::diagnose};
use std::{hint::black_box, time::Instant};

fn errors(r: &qx_physics::wheel_geometry::WheelAlignmentReport) -> Vec<f64> {
    r.wheels
        .iter()
        .map(|w| match &w.target {
            TargetObservation::Measured(n) => n.candidates[0].perpendicular_error_m,
            _ => f64::NAN,
        })
        .collect()
}
fn print_report(name: &str, r: &FitReport) {
    println!(
        "{name}: accepted={} termination={:?} iterations={} evaluations={} rms={:?}->{:?} max={:?}->{:?} joint={:.9e} guide={:.9e} angular={:.9e} rail={:.9e} rigid={:.9e} before={:?} after={:?} rejected={:?}",
        r.accepted,
        r.termination,
        r.iterations,
        r.evaluation_count,
        r.baseline.rms_error_m,
        r.final_alignment.rms_error_m,
        r.baseline.max_error_m,
        r.final_alignment.max_error_m,
        r.final_constraints.max_joint_anchor_gap_m,
        r.final_constraints.max_centerline_guide_error_m,
        r.final_constraints.max_angular_joint_error_rad,
        r.final_constraints.max_rail_attitude_error_rad,
        r.final_alignment.max_rigid_attachment_error_m,
        errors(&r.baseline),
        errors(&r.final_alignment),
        r.evaluations
            .iter()
            .filter_map(|e| e.rejection.as_ref())
            .collect::<Vec<_>>()
    );
}
fn fit(seed: &FitSeed<'_>, wheels: &[WheelGeometry]) -> FitReport {
    let r = seed.fit(HINGE, wheels, FitOptions::default()).unwrap();
    assert_eq!(r, seed.fit(HINGE, wheels, FitOptions::default()).unwrap());
    assert!(r.iterations <= 24 && r.evaluation_count <= 32);
    if !r.accepted {
        assert_eq!(&r.pose, seed.original());
    }
    if r.accepted {
        for (a, b) in errors(&r.baseline).iter().zip(errors(&r.final_alignment)) {
            assert!(b <= a + 1e-10);
        }
        assert!(r.final_constraints.max_joint_anchor_gap_m <= 1e-5);
        assert!(r.final_constraints.max_centerline_guide_error_m <= 1e-5);
        assert!(r.final_constraints.max_angular_joint_error_rad <= 1e-5);
        assert!(r.final_constraints.max_rail_attitude_error_rad <= 1e-5);
    }
    r
}

#[test]
fn independent_hinge_witness_improves_all_wheels_without_relaxing_guidance() {
    for suspended in [false, true] {
        let train = carrier(suspended);
        let track = articulation::straight();
        let original = inspect(&train, &track, 25.).unwrap();
        let wheels = wheels(&train, &original, true);
        let geometry = PhysicalRailGeometry::new(&track, LayoutMode::Open).unwrap();
        let before = diagnose(&train, &original, &geometry, &wheels, Default::default()).unwrap();
        let mut witness = original.clone();
        let parent = original
            .components
            .iter()
            .find(|c| c.id == "gear-1-1")
            .unwrap()
            .pose;
        let c = witness
            .components
            .iter_mut()
            .find(|c| c.id == CARRIER)
            .unwrap();
        // Closed-form yaw about the AUTHORED offset anchor. This predates the
        // fitting implementation and does not call a numerical optimizer.
        let q = Quaternion::axis_angle(Vec3::Y, TARGET_ANGLE).unwrap();
        c.pose.orientation = q;
        let (sn, cs) = TARGET_ANGLE.sin_cos();
        let rotated_pivot = Vec3::new(
            cs * PIVOT.x + sn * PIVOT.z,
            PIVOT.y,
            -sn * PIVOT.x + cs * PIVOT.z,
        );
        c.pose.position_m = parent.position_m - rotated_pivot;
        let pose = c.pose;
        let after = diagnose(&train, &witness, &geometry, &wheels, Default::default()).unwrap();
        assert!(before.max_error_m.unwrap() > 0.03);
        assert!(after.max_error_m.unwrap() < 1e-12);
        for j in &after.joints {
            assert!(j.anchor_gap_m.length() < 1e-12);
            assert!(j.angular_error_rad < 1e-12);
        }
        let frame = Frame::new(pose.orientation).unwrap();
        assert!(
            (pose.position_m + frame.local_to_world(PIVOT) - parent.position_m).length() < 1e-12
        );
        // Independent swing/twist: relative rail quaternion has no X twist.
        assert!(2. * q.x.atan2(q.w).abs() < 1e-12);
        let actual_angle = 2. * q.y.atan2(q.w);
        let JointModel::Hinge {
            limits: Some(l), ..
        } = train.connections.last().unwrap().model
        else {
            panic!("expected authored limited hinge")
        };
        assert!(actual_angle >= l.min_rad && actual_angle <= l.max_rad);
        assert!(after.max_rigid_attachment_error_m < 1e-12);
        println!(
            "analytic-witness suspended={suspended} before={:?}/{:?} after={:?}/{:?}",
            before.rms_error_m, before.max_error_m, after.rms_error_m, after.max_error_m
        );
    }
}

#[test]
fn paired_guides_and_rail_twist_remove_independent_bogie_rotation() {
    // On a straight track, each guide contributes its transverse point motion.
    // Guide +/-a on X yields |pitch|*a and |yaw|*a; the rail adds roll.
    // All three independent rotations are observable: the 3x3 rotational
    // constraint Jacobian has determinant a*a, rather than a null direction.
    let a = 0.2;
    let eps = 1e-6;
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let f = Frame::new(Quaternion::axis_angle(axis, eps).unwrap()).unwrap();
        let point = f.local_to_world(Vec3::X * a);
        let guide_error = point.y.hypot(point.z);
        if axis == Vec3::X {
            assert_eq!(guide_error, 0.);
            assert!((2. * f.orientation().x.atan2(f.orientation().w) - eps).abs() < 1e-15);
        } else {
            assert!((guide_error / eps - a).abs() < 1e-12);
        }
    }
}

#[test]
fn bounded_fit_matches_analytic_witness_and_preserves_source_and_parent_poses() {
    for suspended in [false, true] {
        let train = carrier(suspended);
        let track = articulation::straight();
        let saved = (train.clone(), track.clone());
        let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
        let w = wheels(&train, seed.original(), false);
        let original_wheels = w.clone();
        let r = fit(&seed, &w);
        print_report(&format!("carrier-suspended-{suspended}"), &r);
        assert!(r.accepted);
        assert!(r.final_alignment.max_error_m.unwrap() < 1e-8);
        let child = r.pose.components.iter().find(|c| c.id == CARRIER).unwrap();
        assert!(
            (2. * child.pose.orientation.y.atan2(child.pose.orientation.w) - TARGET_ANGLE).abs()
                < 1e-8
        );
        for (a, b) in seed.original().components.iter().zip(&r.pose.components) {
            if a.id != CARRIER {
                assert_eq!(a, b);
            }
        }
        for e in &r.evaluations {
            assert!(e.angle_rad >= -0.25 && e.angle_rad <= 0.25);
        }
        for p in r.evaluations.windows(2) {
            // All successful fixture proposals are accepted; compare steps.
            assert!((p[1].angle_rad - p[0].angle_rad).abs() <= 0.05 + 1e-14);
        }
        for j in &r.pose.joints {
            assert!(j.error_m < 1e-12);
        }
        // Explicit rigid pair test includes dimensions, guides and wheel centers.
        let original = seed
            .original()
            .components
            .iter()
            .find(|c| c.id == CARRIER)
            .unwrap()
            .pose;
        let points = [
            Vec3::ZERO,
            PIVOT,
            Vec3::new(0.3, 0.15, 0.45),
            w[w.len() - 1].center_local_m,
        ];
        for a in points {
            for b in points {
                let before = Frame::new(original.orientation)
                    .unwrap()
                    .local_to_world(a - b)
                    .length();
                let after = Frame::new(child.pose.orientation)
                    .unwrap()
                    .local_to_world(a - b)
                    .length();
                assert!((after - before).abs() < 1e-12);
            }
        }
        assert_eq!((train, track), saved);
        assert_eq!(w, original_wheels);
    }
}

#[test]
fn conventional_curved_tight_bank_helix_offset_and_suspended_poses_are_not_detached() {
    let straight = articulation::straight();
    let arc = articulation::arc(12., 0.6, 120., false);
    let tight = articulation::arc(8., 0., 120., false);
    let bank = articulation::arc(12., 0.6, 120., true);
    let helix = rail_fixtures::sampled(60., 0.02, |s| {
        rail_fixtures::helix(s, 8., 0.3, 0.4 + 0.04 * s)
    });
    // Also independently evaluate the analytic helix offset, retaining the M3A
    // test helper's exact world conventions.
    assert!(
        (rail_fixtures::helix_locus(0., 8., 0.3, 0., Vec3::ZERO) - Vec3::ZERO).length() < 1e-12
    );
    for (name, track) in [
        ("straight", &straight),
        ("curve", &arc),
        ("tight", &tight),
        ("bank", &bank),
        ("helix", &helix),
    ] {
        let train = articulation::conventional(1);
        let seed = FitSeed::assemble(&train, track, 25.).unwrap();
        let w = wheels(&train, seed.original(), false);
        let r = seed.fit("suspension-1-2", &w, Default::default()).unwrap();
        print_report(name, &r);
        assert!(!r.accepted);
        assert_eq!(r.termination, FitTermination::UnsupportedGraph);
        assert_eq!(&r.pose, seed.original());
    }
    let mut offset = articulation::conventional(1);
    for (i, a, b) in [
        (0, Vec3::new(1.5, -0.7, 0.1), Vec3::new(0.05, 0.15, 0.)),
        (1, Vec3::new(-0.8, -0.9, -0.05), Vec3::new(-0.03, 0.1, 0.)),
    ] {
        offset.connections[i].anchor_a_m = a;
        offset.connections[i].anchor_b_m = b;
    }
    let suspended = qx_document::train::Train::development_consist(
        "m3b-below".into(),
        2,
        qx_document::train::DevelopmentArrangement::BelowTrack,
    )
    .unwrap();
    for (name, train) in [("offset", &offset), ("suspended-paired", &suspended)] {
        let seed = FitSeed::assemble(train, &bank, 25.).unwrap();
        let r = seed
            .fit(
                "suspension-1-2",
                &wheels(train, seed.original(), false),
                Default::default(),
            )
            .unwrap();
        print_report(name, &r);
        assert_eq!(r.termination, FitTermination::UnsupportedGraph);
        assert_eq!(&r.pose, seed.original());
    }
}

#[test]
fn paired_guides_coupled_edges_fixed_and_roll_locked_graphs_are_explicitly_unsupported() {
    let track = articulation::straight();
    for mode in ["paired-guides", "coupled", "fixed", "roll-lock", "ball"] {
        let mut train = carrier(false);
        match mode {
            "paired-guides" => train
                .components
                .last_mut()
                .unwrap()
                .guide_references_m
                .push(PIVOT + Vec3::X * 0.2),
            "coupled" => {
                let mut j = train.connections.last().unwrap().clone();
                j.id = "redundant-bearing".into();
                train.connections.push(j);
            }
            _ => {
                train.connections.last_mut().unwrap().model = match mode {
                    "fixed" => JointModel::Fixed {
                        frame_a: Quaternion::IDENTITY,
                        frame_b: Quaternion::IDENTITY,
                    },
                    "roll-lock" => JointModel::RollLock {
                        frame_a: Quaternion::IDENTITY,
                        frame_b: Quaternion::IDENTITY,
                    },
                    _ => JointModel::Ball,
                }
            }
        }
        let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
        let r = fit(&seed, &wheels(&train, seed.original(), true));
        assert_eq!(
            r.termination,
            FitTermination::UnsupportedGraph,
            "{mode}: {r:?}"
        );
        assert!(!r.accepted);
        println!("unsupported {mode}: {}", r.detail);
    }
}

#[test]
fn ambiguous_missing_boundary_and_budget_exhausted_targets_leave_seed_intact() {
    let train = carrier(false);
    let track = articulation::straight();
    let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
    for mode in ["missing", "boundary", "query-budget"] {
        let mut w = wheels(&train, seed.original(), true);
        match mode {
            "missing" => w[0].target = WheelTarget::Unavailable("no mechanical locus".into()),
            "boundary" => w[0].window.min_m += 1.4,
            _ => w[0].window.max_m += 100.,
        }
        let r = fit(&seed, &w);
        assert!(!r.accepted);
        assert_eq!(r.termination, FitTermination::InvalidTarget);
    }
    // A twice-traversed circle's two station branches lie in one bounded
    // window; no remote branch is designated as the wheel's intended one.
    let radius = 0.1;
    let curve = rail_fixtures::sampled(4. * std::f64::consts::PI * radius, 0.001, |s| {
        rail_fixtures::helix(s, radius, 0., 0.)
    });
    // Assembly of the ordinary car is not possible on this tiny curve: that is
    // also an explicit seed rejection, not an excuse to fit a failed assembly.
    assert!(FitSeed::assemble(&train, &curve, 0.3).is_err());
    let mut small = train.clone();
    for c in &mut small.components {
        c.dimensions_m = c.dimensions_m * 0.01;
        for g in &mut c.guide_references_m {
            *g = *g * 0.01;
        }
        if let Some(p) = &mut c.placement {
            match p {
                Placement::Track { offset_m, frame } => {
                    *offset_m *= 0.01;
                    frame.translation_m = frame.translation_m * 0.01;
                }
                Placement::Mounted { frame, .. } => {
                    frame.translation_m = frame.translation_m * 0.01
                }
            }
        }
    }
    for j in &mut small.connections {
        j.anchor_a_m = j.anchor_a_m * 0.01;
        j.anchor_b_m = j.anchor_b_m * 0.01;
    }
    // Separate two-body query fixture, not a relaxed conventional train: a
    // point-guided base and the small hinge carrier share one track pivot.
    small
        .components
        .retain(|c| c.id == "car-1" || c.id == CARRIER);
    small.connections.retain(|j| j.id == HINGE);
    small.connections[0].component_a = "car-1".into();
    small.cars[0].bogie_ids = vec![CARRIER.into()];
    small.components[0].guide_references_m = vec![Vec3::ZERO];
    if let Some(Placement::Track { frame, .. }) = &mut small.components[0].placement {
        frame.translation_m = Vec3::ZERO;
    }
    if let Some(Placement::Mounted { parent, .. }) = &mut small.components[1].placement {
        *parent = "car-1".into();
    }
    let small_seed = FitSeed::assemble(&small, &curve, 0.3).unwrap();
    let mut small_w = wheels(&small, small_seed.original(), true);
    for v in &mut small_w {
        v.center_local_m = v.center_local_m * 0.01;
        if let WheelTarget::Declared(p) = &mut v.target {
            p.rail.offset_m = p.rail.offset_m * 0.01;
            p.center_offset_m = p.center_offset_m * 0.01;
        }
        v.window.min_m = 0.;
        v.window.max_m = curve.length_m();
    }
    let r = fit(&small_seed, &small_w);
    assert_eq!(r.termination, FitTermination::InvalidTarget);
    assert!(r.baseline.wheels.iter().any(|w| matches!(&w.target, TargetObservation::Measured(n)
        if n.status == qx_geometry::rail_geometry::SearchStatus::Ambiguous && n.candidates.len() >= 2)));
    let mut invalid = wheels(&train, seed.original(), true);
    invalid[0].center_local_m.x = f64::NAN;
    assert!(seed.fit(HINGE, &invalid, Default::default()).is_err());
}

#[test]
fn limits_station_movement_and_evaluation_budgets_are_obeyed() {
    let mut train = carrier(false);
    if let JointModel::Hinge {
        limits: Some(l), ..
    } = &mut train.connections.last_mut().unwrap().model
    {
        l.max_rad = 0.04;
    }
    let track = articulation::straight();
    let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
    let w = wheels(&train, seed.original(), true);
    let r = fit(&seed, &w);
    print_report("active-limit", &r);
    assert!(r.accepted);
    assert_eq!(r.termination, FitTermination::AngularBound);
    assert!(r.evaluations.iter().all(|e| e.angle_rad <= 0.04));
    assert!(r.final_alignment.max_error_m.unwrap() > 0.02);
    let r = seed
        .fit(
            HINGE,
            &w,
            FitOptions {
                max_evaluations: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.termination, FitTermination::EvaluationLimit);
    assert_eq!(r.evaluations.len(), 1);
    assert!(!r.accepted);
    assert_eq!(&r.pose, seed.original());
    let r = seed
        .fit(
            HINGE,
            &w,
            FitOptions {
                max_station_travel_m: 1e-6,
                max_iterations: 2,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!r.accepted);
    assert!(
        r.evaluations.iter().skip(1).all(|e| e
            .rejection
            .as_deref()
            .unwrap()
            .contains("station-travel"))
    );
    assert_eq!(&r.pose, seed.original());
    assert!(
        seed.fit(
            HINGE,
            &w,
            FitOptions {
                max_step_rad: f64::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn incompatible_targets_rank_deficiency_and_individual_regression_are_not_hidden_by_rms() {
    let train = carrier(false);
    let track = articulation::straight();
    let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
    let mut w = wheels(&train, seed.original(), true);
    // Coincident centers at the hinge axis cannot reach two parallel loci
    // 1.2m apart: max error >= .6m by triangle inequality, independently of fit.
    w.truncate(2);
    for v in &mut w {
        v.center_local_m = PIVOT + Vec3::Y * 0.18;
    }
    let r = fit(&seed, &w);
    print_report("incompatible-rank-zero", &r);
    assert_eq!(r.termination, FitTermination::RankDeficient);
    assert!(!r.accepted);
    assert!(r.final_alignment.max_error_m.unwrap() >= 0.6 - 1e-12);
    let mut w = wheels(&train, seed.original(), true);
    let mut initially_correct = w[0].clone();
    initially_correct.id = "individual-outlier-guard".into();
    initially_correct.center_local_m = PIVOT + Vec3::new(0.3, 0.18, -0.6);
    w.push(initially_correct);
    let r = fit(&seed, &w);
    print_report("individual-veto", &r);
    assert!(!r.accepted);
    assert!(r.evaluations.iter().skip(1).any(|e| {
        e.wheels.rms_error_m < r.baseline.rms_error_m
            && e.rejection
                .as_deref()
                .is_some_and(|s| s.contains("individual wheel"))
    }));
}

#[test]
fn severe_twist_exposes_unavoidable_residuals_and_rail_attitude_is_a_hard_gate() {
    let train = carrier(false);
    let track = rail_fixtures::sampled(60., 0.02, |s| {
        (Vec3::X * s, Quaternion::axis_angle(Vec3::X, s).unwrap())
    });
    let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
    let w = wheels(&train, seed.original(), true);
    let r = fit(&seed, &w);
    print_report("severe-twist", &r);
    assert!(r.final_alignment.max_error_m.unwrap() > 0.1);
    let separation = (0.6_f64.powi(2)
        + 4. * (0.18_f64.powi(2) + 0.6_f64.powi(2)) * 0.3_f64.sin().powi(2))
    .sqrt();
    assert!(separation - 0.6 > 0.1);
    println!(
        "severe-twist independent prescribed-station pair distance={separation:.12} rigid_distance=0.6 mismatch={:.12}",
        separation - 0.6
    );
    // Two same-station opposite-rail target points have invariant 1.2m
    // separation. Across distinct stations, twist also changes required rigid
    // cross-row distances. No exact rigid fit is asserted by a smaller RMS.
    // For a pure roll hinge, the rail-attitude constraint vetoes a wheel-only
    // improvement. Rotate the fixture's joint frames, without deleting guides.
    let mut train = carrier(false);
    if let JointModel::Hinge {
        frame_a, frame_b, ..
    } = &mut train.connections.last_mut().unwrap().model
    {
        *frame_a = Quaternion::axis_angle(Vec3::Y, std::f64::consts::FRAC_PI_2).unwrap();
        *frame_b = *frame_a;
    }
    let straight = articulation::straight();
    let seed = FitSeed::assemble(&train, &straight, 25.).unwrap();
    let mut w = wheels(&train, seed.original(), true);
    let roll = Frame::new(Quaternion::axis_angle(Vec3::X, -0.12).unwrap()).unwrap();
    for v in &mut w {
        let z = if v.id.ends_with("--0.6") { -0.6 } else { 0.6 };
        let x = if v.id.contains("--0.3-") { -0.3 } else { 0.3 };
        v.center_local_m = PIVOT + roll.local_to_world(Vec3::new(x, 0.18, z));
    }
    let r = fit(&seed, &w);
    assert!(!r.accepted);
    assert!(
        r.evaluations
            .iter()
            .any(|e| e.constraints.max_rail_attitude_error_rad > 1e-5
                && e.wheels.rms_error_m < r.baseline.rms_error_m
                && e.rejection.is_some())
    );
}

#[test]
fn analytic_rigid_point_jacobian_matches_independent_centered_differences() {
    let pivot = Vec3::new(3., -2., 4.);
    let axis = Vec3::new(1., 2., -3.).normalized().unwrap();
    let p = Vec3::new(2.7, -1.4, 4.8);
    let target = Vec3::new(2.9, -1.1, 5.1);
    let tangent = Vec3::new(-0.2, 0.7, 0.4).normalized().unwrap();
    let (_, j) = frozen_target_linearization(p, target, tangent, pivot, axis);
    // Independent Rodrigues evaluation; no quaternion rotation or production
    // Jacobian in the two samples. This differentiates the declared frozen-
    // target model, not an incorrectly assumed exact curved-path projector.
    let residual = |angle: f64| {
        let v = p - pivot;
        let q = pivot
            + v * angle.cos()
            + axis.cross(v) * angle.sin()
            + axis * (axis.dot(v) * (1. - angle.cos()));
        let d = q - target;
        d - tangent * d.dot(tangent)
    };
    for h in [1e-4, 1e-5, 1e-6] {
        assert!(((residual(h) - residual(-h)) / (2. * h) - j).length() < 3e-9);
    }
    // The independently reprojected straight-path residual has the SAME
    // derivative; nearest station movement is along the constant tangent.
    let station_residual = |angle: f64| {
        let v = p - pivot;
        let q = pivot
            + v * angle.cos()
            + axis.cross(v) * angle.sin()
            + axis * (axis.dot(v) * (1. - angle.cos()));
        let station = (q - target).dot(tangent);
        q - (target + tangent * station)
    };
    assert!(((station_residual(1e-5) - station_residual(-1e-5)) / 2e-5 - j).length() < 1e-9);
}

#[test]
fn distinct_authored_joint_frames_on_banked_and_inverted_track_recover_known_pose() {
    for bank in [0.4, std::f64::consts::PI] {
        let mut train = carrier(false);
        let gamma = 0.07;
        let initial = Quaternion::axis_angle(Vec3::Y, gamma).unwrap();
        if let Some(Placement::Mounted { frame, .. }) =
            &mut train.components.last_mut().unwrap().placement
        {
            frame.rotation = initial;
            frame.translation_m = -Frame::new(initial).unwrap().local_to_world(PIVOT);
        }
        if let JointModel::Hinge {
            frame_a, frame_b, ..
        } = &mut train.connections.last_mut().unwrap().model
        {
            *frame_b = initial.conjugate().compose(*frame_a);
        }
        let track = rail_fixtures::sampled(60., 0.1, |s| {
            (Vec3::X * s, Quaternion::axis_angle(Vec3::X, bank).unwrap())
        });
        let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
        let w = wheels(&train, seed.original(), false);
        let r = fit(&seed, &w);
        print_report(&format!("distinct-frames-bank-{bank}"), &r);
        assert!(r.accepted);
        assert!(r.final_alignment.max_error_m.unwrap() < 1e-8);
        let q = Quaternion::axis_angle(Vec3::X, bank)
            .unwrap()
            .compose(Quaternion::axis_angle(Vec3::Y, TARGET_ANGLE).unwrap());
        let c = r.pose.components.iter().find(|c| c.id == CARRIER).unwrap();
        assert!(
            q.conjugate()
                .compose(c.pose.orientation)
                .rotation_vector()
                .unwrap()
                .length()
                < 1e-8
        );
        assert!((r.evaluations.last().unwrap().angle_rad - (TARGET_ANGLE - gamma)).abs() < 1e-8);
    }
}

#[test]
fn fitting_is_not_installed_in_playback_and_does_not_mutate_physics() {
    let mut train = carrier(false);
    train.components.last_mut().unwrap().center_of_mass_m = Vec3::new(0.03, -0.02, 0.01);
    let track = articulation::straight();
    let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
    let w = wheels(&train, seed.original(), true);
    let mut control = qx_physics::TrackSimulation::new(
        train.clone(),
        std::sync::Arc::new(track.clone()),
        25.,
        5.,
    )
    .unwrap();
    let mut observed = qx_physics::TrackSimulation::new(
        train.clone(),
        std::sync::Arc::new(track.clone()),
        25.,
        5.,
    )
    .unwrap();
    let report = fit(&seed, &w);
    assert!(report.accepted);
    for _ in 0..4 {
        assert!(fit(&seed, &w).accepted);
        assert_eq!(control.step().unwrap(), observed.step().unwrap());
        assert_eq!(control.world(), observed.world());
        assert_eq!(control.snapshot().unwrap(), observed.snapshot().unwrap());
    }
    assert_ne!(report.pose, observed.snapshot().unwrap());
}

#[test]
#[ignore = "native Release fit timing; shared-machine observation, not a CI threshold"]
fn release_fit_duration() {
    let measure = |name: &str, seed: &FitSeed<'_>, w: &[WheelGeometry], expected: bool| {
        let r = fit(seed, w);
        assert_eq!(r.accepted, expected);
        let mut samples = vec![];
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..5 {
                black_box(seed.fit(HINGE, w, Default::default()).unwrap());
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 5.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "fit-timing {name} accepted={} median_us={:.3} min_us={:.3} max_us={:.3} iterations={} evaluations={}",
            r.accepted, samples[3], samples[0], samples[6], r.iterations, r.evaluation_count
        );
    };
    for suspended in [false, true] {
        let train = carrier(suspended);
        let track = articulation::straight();
        let seed = FitSeed::assemble(&train, &track, 25.).unwrap();
        let w = wheels(&train, seed.original(), true);
        measure(&format!("suspended-{suspended}"), &seed, &w, true);
    }
    let train = carrier(false);
    let straight = articulation::straight();
    let seed = FitSeed::assemble(&train, &straight, 25.).unwrap();
    let mut w = wheels(&train, seed.original(), true);
    let mut correct = w[0].clone();
    correct.id = "timing-individual-veto".into();
    correct.center_local_m = PIVOT + Vec3::new(0.3, 0.18, -0.6);
    w.push(correct);
    measure("individual-veto", &seed, &w, false);
    let twist = rail_fixtures::sampled(60., 0.02, |s| {
        (Vec3::X * s, Quaternion::axis_angle(Vec3::X, s).unwrap())
    });
    let seed = FitSeed::assemble(&train, &twist, 25.).unwrap();
    measure(
        "severe-twist",
        &seed,
        &wheels(&train, seed.original(), true),
        false,
    );
}
