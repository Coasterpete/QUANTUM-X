#[path = "support/feasible.rs"]
mod feasible;
#[path = "support/articulation.rs"]
mod fixtures;
use qx_physics::{
    assembly_diagnostics::AssemblyDiagnostics,
    consist::{inspect, inspect_with_diagnostics},
};

#[test]
fn independent_feasibility_witnesses() {
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            for asymmetric in [false, true] {
                let mut train = fixtures::conventional(count);
                let witness = feasible::witness(&mut train, &track, asymmetric);
                println!(
                    "witness changing={changing} count={count} asymmetric={asymmetric}: {:?}",
                    feasible::verify(&train, &track, &witness)
                );
            }
        }
    }
}

#[test]
fn characterize_edge_propagation() {
    // Also exercise the original straight helper without modifying M1 fixtures.
    assert!(inspect(&fixtures::conventional(2), &fixtures::straight(), 200.).is_ok());
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            let train = fixtures::conventional(count);
            let mut d = AssemblyDiagnostics::default();
            let result = inspect_with_diagnostics(&train, &track, 200., &mut d);
            let mut witness_train = train.clone();
            let witness = feasible::witness(&mut witness_train, &track, false);
            let gear = witness
                .components
                .iter()
                .find(|c| c.id == "gear-1-1")
                .unwrap();
            let pin = d.seed_residuals.as_ref().unwrap().guides[0].station_m;
            assert!(
                (feasible::point(gear.pose, qx_geometry::math::Vec3::new(-0.2, 0., 0.))
                    - track.sample_at_distance(pin).unwrap().position_m)
                    .length()
                    < 2e-10
            );
            if let Ok(inspection) = &result {
                feasible::verify_with_tolerance(&train, &track, inspection, 1e-5);
            }
            println!(
                "case changing={changing} count={count} result={} sweeps={}",
                result.is_ok(),
                d.projection_sweeps
            );
            let mut previous = d.seed_residuals.as_ref().unwrap();
            for r in d
                .convergence_checks
                .iter()
                .chain(d.terminal_residuals.iter())
            {
                if r.sweep == previous.sweep {
                    continue;
                }
                assert_eq!(r.connections.len(), train.connections.len());
                let maximum = r
                    .connections
                    .iter()
                    .map(|e| e.gap_m.length())
                    .fold(0., f64::max);
                assert_eq!(maximum, r.max_joint_anchor_gap_m);
                let mut translation: f64 = 0.;
                let mut rotation: f64 = 0.;
                for (a, b) in previous.component_poses.iter().zip(&r.component_poses) {
                    translation = translation.max((a.position_m - b.position_m).length());
                    rotation = rotation.max(
                        a.orientation
                            .conjugate()
                            .compose(b.orientation)
                            .rotation_vector()
                            .unwrap()
                            .length(),
                    );
                }
                if [1, 9, 65, 257, 513, 1025, 2049, 2393].contains(&r.sweep)
                    || r.sweep == d.projection_sweeps
                {
                    let ratio = (r.max_joint_anchor_gap_m / previous.max_joint_anchor_gap_m)
                        .powf(1. / (r.sweep - previous.sweep).max(1) as f64);
                    println!(
                        "motion sweep={} translation={translation:.9e} rotation={rotation:.9e} joint_ratio_per_sweep={ratio:.9e}",
                        r.sweep
                    );
                }
                previous = r;
            }
            for r in d
                .seed_residuals
                .iter()
                .chain(
                    d.convergence_checks
                        .iter()
                        .filter(|r| [1, 9, 65, 257, 513, 1025, 2049, 2393].contains(&r.sweep)),
                )
                .chain(d.terminal_residuals.iter())
            {
                let gaps: Vec<_> = r.connections.iter().map(|e| e.gap_m.length()).collect();
                println!(
                    "sweep={} joint={:.9e} guide={:.9e} angular={:.9e} gaps={gaps:?}",
                    r.sweep,
                    r.max_joint_anchor_gap_m,
                    r.max_centerline_guide_error_m,
                    r.max_angular_joint_error_rad
                );
            }
        }
    }
}

#[test]
fn asymmetric_hitches_have_measured_bounded_convergence() {
    use qx_physics::assembly_diagnostics::AssemblyTermination;
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            let mut train = fixtures::conventional(count);
            let witness = feasible::witness(&mut train, &track, true);
            feasible::verify(&train, &track, &witness);
            let original = train.clone();
            let original_track = track.clone();
            let mut d = AssemblyDiagnostics::default();
            let result = inspect_with_diagnostics(&train, &track, 200., &mut d);
            println!(
                "asymmetric changing={changing} count={count} termination={:?} sweeps={} terminal={:?}",
                d.termination,
                d.projection_sweeps,
                d.terminal_residuals.as_ref().map(|r| (
                    r.max_joint_anchor_gap_m,
                    r.max_centerline_guide_error_m,
                    r.max_angular_joint_error_rad
                ))
            );
            if count == 32 {
                // Independently feasible, but still unresolved within budget.
                // Retain the individual residuals instead of accepting a pose.
                assert!(result.is_err());
                assert_eq!(d.termination, AssemblyTermination::SweepLimit);
                assert!(
                    d.terminal_residuals
                        .as_ref()
                        .unwrap()
                        .max_joint_anchor_gap_m
                        > 1e-5
                );
                assert_eq!(train, original);
                assert_eq!(track, original_track);
                continue;
            }
            let result = result.unwrap();
            assert_eq!(d.termination, AssemblyTermination::Converged);
            let verified = feasible::verify_with_tolerance(&train, &track, &result, 1e-5);
            assert!(verified.0 <= 1e-5);
            assert_eq!(result, inspect(&train, &track, 200.).unwrap());
            let mut replay = AssemblyDiagnostics::default();
            assert_eq!(
                result,
                inspect_with_diagnostics(&train, &track, 200., &mut replay).unwrap()
            );
            assert_eq!(d, replay);
            assert_eq!(train, original);
            assert_eq!(track, original_track);
        }
    }
}

#[test]
fn contradictory_individual_connection_cannot_hide_in_an_average() {
    use qx_geometry::math::Vec3;
    use qx_physics::assembly_diagnostics::AssemblyTermination;
    let track = fixtures::straight();
    for separation in [0.1, 2.2e-5] {
        let mut train = fixtures::conventional(8);
        let mut contradiction = train.connections[0].clone();
        contradiction.id = "incompatible-parallel-edge".into();
        contradiction.anchor_a_m = contradiction.anchor_a_m + Vec3::Y * separation;
        train.connections.push(contradiction);
        let mut d = AssemblyDiagnostics::default();
        let observed = inspect_with_diagnostics(&train, &track, 200., &mut d);
        assert_eq!(observed, inspect(&train, &track, 200.));
        assert!(observed.is_err());
        assert_eq!(d.termination, AssemblyTermination::SweepLimit);
        let r = d.terminal_residuals.unwrap();
        // Same B anchor, separated rigid A anchors: at least one edge must
        // miss by half the separation, irrespective of the iteration method.
        let a = r
            .connections
            .iter()
            .find(|j| j.id == "suspension-1-1")
            .unwrap()
            .gap_m
            .length();
        let b = r
            .connections
            .iter()
            .find(|j| j.id == "incompatible-parallel-edge")
            .unwrap()
            .gap_m
            .length();
        assert!(a.max(b) >= separation * 0.5 - 1e-12);
        assert!(r.max_joint_anchor_gap_m >= a.max(b));
        assert_eq!(r.sweep, 2400);
        let mean = r.connections.iter().map(|j| j.gap_m.length()).sum::<f64>()
            / r.connections.len() as f64;
        if separation < 1e-3 {
            assert!(mean < 1e-5 && r.max_joint_anchor_gap_m > 1e-5);
        }
        println!(
            "contradiction separation={separation} max={:.9e} mean={mean:.9e} guide={:.9e}",
            r.max_joint_anchor_gap_m, r.max_centerline_guide_error_m
        );
    }
}

#[test]
fn shared_running_gear_keeps_one_body_and_the_general_graph() {
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        let (train, witness) = feasible::shared(&track);
        feasible::verify(&train, &track, &witness);
        let mut d = AssemblyDiagnostics::default();
        let result = inspect_with_diagnostics(&train, &track, 200., &mut d).unwrap();
        assert_eq!(result.components.len(), 5);
        assert_eq!(
            result
                .components
                .iter()
                .filter(|c| c.id == "gear-1-2")
                .count(),
            1
        );
        feasible::verify_with_tolerance(&train, &track, &result, 1e-5);
        assert_eq!(result, inspect(&train, &track, 200.).unwrap());
        println!(
            "shared changing={changing} sweeps={} joint={:.9e}",
            d.projection_sweeps,
            d.terminal_residuals.unwrap().max_joint_anchor_gap_m
        );
    }
}

#[test]
#[ignore = "Release sensitivity observations, not performance assertions"]
fn ordering_characterization() {
    let track = fixtures::arc(45., 0.25, 500., true);
    for count in [8, 16, 32] {
        for variant in [
            "authored",
            "reverse-edges",
            "reverse-components",
            "middle-pin",
            "reverse-all",
        ] {
            let mut train = fixtures::conventional(count);
            if variant.contains("edges") || variant == "reverse-all" {
                train.connections.reverse();
            }
            if variant.contains("components") || variant == "reverse-all" {
                train.components.reverse();
            }
            if variant == "middle-pin" {
                let middle = train
                    .components
                    .iter()
                    .position(|c| c.id == format!("gear-{}-1", count / 2))
                    .unwrap();
                train.components.swap(0, middle);
            }
            let mut d = AssemblyDiagnostics::default();
            let result = inspect_with_diagnostics(&train, &track, 200., &mut d);
            assert!(result.is_ok(), "{count} cars / {variant}: {result:?}");
            assert_eq!(result, inspect(&train, &track, 200.));
            let mut replay = AssemblyDiagnostics::default();
            assert_eq!(
                result,
                inspect_with_diagnostics(&train, &track, 200., &mut replay)
            );
            assert_eq!(d, replay);
            let r = d.terminal_residuals.unwrap();
            println!(
                "order count={count} variant={variant} success={} sweeps={} joint={:.9e} guide={:.9e} angular={:.9e}",
                result.is_ok(),
                d.projection_sweeps,
                r.max_joint_anchor_gap_m,
                r.max_centerline_guide_error_m,
                r.max_angular_joint_error_rad
            );
        }
    }
}

#[test]
#[ignore = "Release candidate comparison; selection is documented in M2B report"]
fn candidate_summary() {
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            for asymmetric in [false, true] {
                let mut train = fixtures::conventional(count);
                if asymmetric {
                    let w = feasible::witness(&mut train, &track, true);
                    feasible::verify(&train, &track, &w);
                }
                let mut d = AssemblyDiagnostics::default();
                let r = inspect_with_diagnostics(&train, &track, 200., &mut d);
                for sample in d.seed_residuals.iter().chain(d.terminal_residuals.iter()) {
                    let gaps: Vec<_> = sample
                        .connections
                        .iter()
                        .map(|c| c.gap_m.length())
                        .collect();
                    println!(
                        "edge-distribution changing={changing} count={count} asymmetric={asymmetric} sweep={} gaps={gaps:?}",
                        sample.sweep
                    );
                }
                let t = d.terminal_residuals.unwrap();
                println!(
                    "candidate changing={changing} count={count} asymmetric={asymmetric} success={} sweeps={} joint={:.9e} guide={:.9e} angular={:.9e} rail={:.9e} rigid={:.9e}",
                    r.is_ok(),
                    d.projection_sweeps,
                    t.max_joint_anchor_gap_m,
                    t.max_centerline_guide_error_m,
                    t.max_angular_joint_error_rad,
                    t.max_rail_attitude_error_rad,
                    t.max_rigid_attachment_error_m
                );
            }
        }
    }
}

#[test]
#[ignore = "manual Release timing; tracks, witnesses and observations outside timers"]
fn release_long_chain_duration() {
    use std::{hint::black_box, time::Instant};
    for changing in [false, true] {
        let track = fixtures::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            let train = fixtures::conventional(count);
            let mut d = AssemblyDiagnostics::default();
            let result = inspect_with_diagnostics(&train, &track, 200., &mut d);
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                for _ in 0..3 {
                    let _ = black_box(inspect(black_box(&train), black_box(&track), 200.));
                }
                times.push(start.elapsed().as_secs_f64() * 1e6 / 3.);
            }
            times.sort_by(f64::total_cmp);
            let r = d.terminal_residuals.unwrap();
            println!(
                "time changing={changing} count={count} success={} sweeps={} joint={:.9e} guide={:.9e} angular={:.9e} us={:.3} [{:.3},{:.3}]",
                result.is_ok(),
                d.projection_sweeps,
                r.max_joint_anchor_gap_m,
                r.max_centerline_guide_error_m,
                r.max_angular_joint_error_rad,
                times[3],
                times[0],
                times[6]
            );
        }
    }
}
