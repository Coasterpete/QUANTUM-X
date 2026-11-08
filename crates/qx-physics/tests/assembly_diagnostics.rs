#[path = "support/articulation.rs"]
mod fixtures;

use fixtures::*;
use qx_document::train::{DevelopmentArrangement, Train, mechanism::JointModel};
use qx_geometry::math::Vec3;
use qx_physics::{
    assembly_diagnostics::{AssemblyDiagnostics, AssemblyPhase, AssemblyTermination},
    consist::{inspect, inspect_with_diagnostics},
};

fn characterize(
    name: &str,
    train: &Train,
    track: &qx_geometry::solver::GeneratedTrack,
    station: f64,
) -> AssemblyDiagnostics {
    let before_train = train.clone();
    let before_track = track.clone();
    let ordinary = inspect(train, track, station);
    let mut d = AssemblyDiagnostics::default();
    let observed = inspect_with_diagnostics(train, track, station, &mut d);
    assert_eq!(ordinary, observed, "observations changed result for {name}");
    let mut replay = AssemblyDiagnostics::default();
    assert_eq!(
        observed,
        inspect_with_diagnostics(train, track, station, &mut replay)
    );
    assert_eq!(d, replay, "nondeterministic observations for {name}");
    assert_eq!(train, &before_train);
    assert_eq!(track, &before_track);
    assert!(
        d.measurement_error.is_none(),
        "{name}: {:?}",
        d.measurement_error
    );
    println!(
        "{name}: components={} connections={} termination={:?} sweeps={} checks={} chord_probes={} bisections={} seed={:?} terminal={:?} error={:?}",
        train.components.len(),
        train.connections.len(),
        d.termination,
        d.projection_sweeps,
        d.convergence_checks.len(),
        d.chords.iter().map(|c| c.bracket_probes).sum::<u32>(),
        d.chords.iter().map(|c| c.bisection_iterations).sum::<u32>(),
        d.seed_residuals.as_ref().map(|r| (
            r.max_joint_anchor_gap_m,
            r.max_centerline_guide_error_m,
            r.max_angular_joint_error_rad
        )),
        d.terminal_residuals.as_ref().map(|r| (
            r.max_joint_anchor_gap_m,
            r.max_centerline_guide_error_m,
            r.max_angular_joint_error_rad
        )),
        d.error
    );
    if observed.is_ok() {
        assert_eq!(d.termination, AssemblyTermination::Converged);
        let r = d.terminal_residuals.as_ref().unwrap();
        assert!(r.max_rigid_attachment_error_m < 1e-10);
        assert!(r.max_joint_anchor_gap_m <= 1e-5);
        assert!(r.max_centerline_guide_error_m <= 1e-5);
        assert!(r.max_angular_joint_error_rad <= 1e-5);
        assert!(r.max_rail_attitude_error_rad <= 1e-5);
        assert!(d.projection_sweeps % 8 == 1 || d.projection_sweeps == 2400);
        assert_eq!(d.convergence_checks.last().unwrap(), r);
    } else {
        assert_ne!(d.termination, AssemblyTermination::Converged);
        assert_eq!(
            d.error.as_deref(),
            observed.as_ref().err().map(String::as_str)
        );
    }
    assert!(d.projection_sweeps <= 2400);
    assert!(d.convergence_checks.len() <= 301);
    d
}

#[test]
fn conventional_lengths_have_measured_bounded_convergence() {
    let flat = straight();
    let curve = arc(45., 0.25, 500., false);
    let twisting = arc(45., 0.25, 500., true);
    for count in [2, 8, 16, 32] {
        let train = conventional(count);
        let d = characterize(&format!("straight-{count}"), &train, &flat, 200.);
        assert_eq!(d.termination, AssemblyTermination::Converged);
        assert_eq!(d.projection_sweeps, 1);
    }
    for (name, track) in [("curve", &curve), ("twisting", &twisting)] {
        for count in [2, 8, 16, 32] {
            let d = characterize(
                &format!("{name}-{count}"),
                &conventional(count),
                track,
                200.,
            );
            // M2B closes the same independently feasible long-chain fixtures;
            // retain all original per-unit and rigid-dimension assertions.
            assert_eq!(d.termination, AssemblyTermination::Converged);
        }
    }
}

#[test]
fn existing_mechanical_variants_keep_rigid_attachments() {
    let flat = straight();
    let curve = arc(12., 0.6, 120., false);
    let mut asymmetric = conventional(1);
    asymmetric.connections[0].anchor_a_m = Vec3::new(1.5, -0.7, 0.1);
    asymmetric.connections[1].anchor_a_m = Vec3::new(-0.8, -0.9, -0.05);
    asymmetric.connections[0].anchor_b_m = Vec3::new(0.05, 0.15, 0.);
    asymmetric.connections[1].anchor_b_m = Vec3::new(-0.03, 0.1, 0.);
    assert_eq!(
        characterize("off-axis-asymmetric", &asymmetric, &curve, 25.).termination,
        AssemblyTermination::Converged
    );

    let mut shared = conventional(2);
    shared.cars[1].bogie_ids.push("gear-1-2".into());
    let mut joint = shared.connections[0].clone();
    joint.id = "shared".into();
    joint.component_a = "car-2".into();
    joint.component_b = "gear-1-2".into();
    joint.anchor_a_m = Vec3::new(2.8, -0.8, 0.);
    shared.connections.push(joint);
    assert_eq!(
        characterize("shared-three-gear", &shared, &flat, 200.).termination,
        AssemblyTermination::Converged
    );
    assert_eq!(
        characterize("shared-three-gear-curved", &shared, &curve, 25.).termination,
        AssemblyTermination::SweepLimit
    );

    for arrangement in [
        DevelopmentArrangement::BelowTrack,
        DevelopmentArrangement::RotatingSeats,
    ] {
        let train = Train::development_consist("m1-variant".into(), 2, arrangement).unwrap();
        assert_eq!(
            characterize(&format!("{arrangement:?}"), &train, &curve, 25.).termination,
            AssemblyTermination::Converged
        );
    }
    let legacy = Train::development_fixture("legacy-offset-hitch".into());
    assert_eq!(
        characterize("legacy-offset-hitch-straight", &legacy, &flat, 200.).termination,
        AssemblyTermination::Converged
    );
    assert_eq!(
        characterize("legacy-offset-hitch-curved", &legacy, &curve, 25.).termination,
        AssemblyTermination::Converged
    );

    let mut fixed = conventional(1);
    fixed.connections[0].model = JointModel::Fixed {
        frame_a: Default::default(),
        frame_b: Default::default(),
    };
    assert_eq!(
        characterize("fixed-bearing-straight", &fixed, &flat, 200.).termination,
        AssemblyTermination::Converged
    );
}

#[test]
fn failures_preserve_partial_reports_and_distinguish_seed_from_exhaustion() {
    let track = straight();
    let train = conventional(1);
    let mut d = characterize("success-before-report-reuse", &train, &track, 200.);
    assert!(inspect_with_diagnostics(&train, &track, 0., &mut d).is_err());
    assert_eq!(d.phase, AssemblyPhase::PlacementSeed);
    assert_eq!(d.termination, AssemblyTermination::Aborted);
    assert_eq!(d.projection_sweeps, 0);
    assert!(
        d.chords.is_empty() && d.convergence_checks.is_empty() && d.terminal_residuals.is_none()
    );

    let mut impossible = train;
    impossible.components[1].guide_references_m.push(Vec3::Y);
    let d = characterize("incompatible-guides", &impossible, &track, 200.);
    assert_eq!(d.termination, AssemblyTermination::SweepLimit);
    assert_eq!(d.projection_sweeps, 2400);
    assert_eq!(d.convergence_checks.len(), 301);
    assert_eq!(d.convergence_checks.last().unwrap().sweep, 2400);
    assert_eq!(d.terminal_residuals.as_ref().unwrap().sweep, 2400);
    assert!(d.terminal_residuals.unwrap().max_centerline_guide_error_m > 0.1);

    let curve = arc(8., 0., 100., false);
    let mut impossible_chord = conventional(1);
    impossible_chord.connections[0].anchor_a_m.x = 20.;
    impossible_chord.connections[1].anchor_a_m.x = -20.;
    let d = characterize("infeasible-car-chord", &impossible_chord, &curve, 40.);
    assert_eq!(d.phase, AssemblyPhase::ChordSeed);
    assert_eq!(d.termination, AssemblyTermination::Aborted);
    assert_eq!(d.chords.len(), 1);
    assert_eq!(
        d.chords[0].termination,
        qx_physics::assembly_diagnostics::ChordTermination::NoBracket
    );
    assert_eq!(d.projection_sweeps, 0);
}
