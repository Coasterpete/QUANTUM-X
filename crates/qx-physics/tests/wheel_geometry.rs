#[path = "support/articulation.rs"]
mod articulation;
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
use qx_document::train::{
    DevelopmentArrangement, Train,
    mechanism::{LocalFrame, Placement},
};
use qx_geometry::{
    math::{Quaternion, Vec3},
    rail_geometry::{
        PhysicalRailGeometry, RailReference, SearchOptions, SearchStatus, StationWindow,
        WheelCenterPath,
    },
    solver::GeneratedTrack,
    topology::LayoutMode,
};
use qx_physics::{
    assembly_diagnostics::AssemblyDiagnostics,
    consist::{ConsistInspection, inspect, inspect_with_diagnostics},
    wheel_geometry::*,
};
use std::{hint::black_box, time::Instant};

fn wheels(train: &Train, poses: &ConsistInspection) -> Vec<WheelGeometry> {
    let mut result = vec![];
    for c in &train.components {
        if !c.kind.is_running_gear() {
            continue;
        }
        let pose = poses.components.iter().find(|p| p.id == c.id).unwrap();
        for x in [-0.3, 0.3] {
            for z in [-0.6, 0.6] {
                result.push(WheelGeometry {
                    id: format!("{}-{x}-{z}", c.id),
                    component_id: c.id.clone(),
                    center_local_m: Vec3::new(x, 0.18, z),
                    orientation_local: Quaternion::IDENTITY,
                    role: Some(WheelRole::Running),
                    target: WheelTarget::Declared(WheelCenterPath {
                        rail: RailReference {
                            id: format!("declared-rail-{z}"),
                            offset_m: Vec3::new(0., 0., z),
                        },
                        center_offset_m: Vec3::Y * 0.18,
                    }),
                    window: StationWindow {
                        min_m: pose.station_m + x - 1.,
                        max_m: pose.station_m + x + 1.,
                    },
                });
            }
        }
    }
    result
}

// Independent quaternion rotation matrix, deliberately not Frame/world_point.
fn rotate(q: Quaternion, v: Vec3) -> Vec3 {
    Vec3::new(
        (1. - 2. * (q.y * q.y + q.z * q.z)) * v.x
            + 2. * (q.x * q.y - q.z * q.w) * v.y
            + 2. * (q.x * q.z + q.y * q.w) * v.z,
        2. * (q.x * q.y + q.z * q.w) * v.x
            + (1. - 2. * (q.x * q.x + q.z * q.z)) * v.y
            + 2. * (q.y * q.z - q.x * q.w) * v.z,
        2. * (q.x * q.z - q.y * q.w) * v.x
            + 2. * (q.y * q.z + q.x * q.w) * v.y
            + (1. - 2. * (q.x * q.x + q.y * q.y)) * v.z,
    )
}
fn check_world(wheels: &[WheelGeometry], poses: &ConsistInspection, report: &WheelAlignmentReport) {
    for (w, row) in wheels.iter().zip(&report.wheels) {
        let pose = poses
            .components
            .iter()
            .find(|p| p.id == w.component_id)
            .unwrap()
            .pose;
        let expected = pose.position_m + rotate(pose.orientation, w.center_local_m);
        assert!((expected - row.center_world_m).length() < 1e-12);
        assert!(
            (rotate(pose.orientation, rotate(w.orientation_local, Vec3::Z))
                - row.wheel_frame_world.lateral())
            .length()
                < 1e-12
        );
    }
}
fn measured(row: &WheelAlignment) -> &qx_geometry::rail_geometry::PathCandidate {
    let TargetObservation::Measured(n) = &row.target else {
        panic!("{:?}", row.target);
    };
    assert_eq!(n.status, SearchStatus::Resolved);
    &n.candidates[0]
}
fn characterize(
    name: &str,
    train: &Train,
    track: &GeneratedTrack,
    station: f64,
) -> WheelAlignmentReport {
    let original_train = train.clone();
    let original_track = track.clone();
    let mut d = AssemblyDiagnostics::default();
    let poses = inspect_with_diagnostics(train, track, station, &mut d).unwrap();
    let poses_before = poses.clone();
    let w = wheels(train, &poses);
    let original_wheels = w.clone();
    let g = PhysicalRailGeometry::new(track, LayoutMode::Open).unwrap();
    let r = diagnose(train, &poses, &g, &w, Default::default()).unwrap();
    assert_eq!(
        r,
        diagnose(train, &poses, &g, &w, Default::default()).unwrap()
    );
    check_world(&w, &poses, &r);
    assert_eq!(
        r.resolved_wheels,
        w.len(),
        "{name}: {:?}",
        r.wheels.iter().map(|w| &w.target).collect::<Vec<_>>()
    );
    assert!(r.max_rigid_attachment_error_m < 1e-11);
    let terminal = d.terminal_residuals.as_ref().unwrap();
    assert!(terminal.max_joint_anchor_gap_m <= 1e-5);
    for (joint, existing) in r.joints.iter().zip(&terminal.connections) {
        assert_eq!(joint.id, existing.id);
        assert!((joint.anchor_gap_m - existing.gap_m).length() < 1e-12);
        assert!((joint.angular_error_rad - existing.angular_error_rad).abs() < 1e-12);
    }
    let stations: Vec<_> = r
        .wheels
        .iter()
        .take(4)
        .map(|w| measured(w).sample.station_m)
        .collect();
    println!(
        "{name}: wheels={} sweeps={} rms={:.9e} max={:.9e} joint={:.9e} rigid={:.9e} segments={} first_stations={stations:?}",
        w.len(),
        d.projection_sweeps,
        r.rms_error_m.unwrap(),
        r.max_error_m.unwrap(),
        terminal.max_joint_anchor_gap_m,
        r.max_rigid_attachment_error_m,
        r.segments_tested
    );
    assert_eq!(poses, poses_before);
    assert_eq!(w, original_wheels);
    assert_eq!(train, &original_train);
    assert_eq!(track, &original_track);
    assert_eq!(poses, inspect(train, track, station).unwrap());
    r
}

#[test]
fn straight_two_bogie_cars_and_individual_outlier_metrics() {
    let track = articulation::straight();
    let train = articulation::conventional(2);
    let r = characterize("straight-2", &train, &track, 200.);
    assert_eq!(r.wheels.len(), 16);
    assert!(r.max_error_m.unwrap() < 1e-12);
    let poses = inspect(&train, &track, 200.).unwrap();
    let g = PhysicalRailGeometry::new(&track, LayoutMode::Open).unwrap();
    let mut w = wheels(&train, &poses);
    // A second car grouping reference does not duplicate the shared owner's
    // wheels or create a new physical attachment. Mechanical edges are unchanged.
    let mut shared_grouping = train.clone();
    shared_grouping.cars[1]
        .bogie_ids
        .push(train.cars[0].bogie_ids[0].clone());
    assert_eq!(
        r,
        diagnose(&shared_grouping, &poses, &g, &w, Default::default()).unwrap()
    );
    for (wheel, row) in w.iter().zip(&r.wheels) {
        let parts: Vec<_> = wheel.component_id.split('-').collect();
        let car = parts[1].parse::<usize>().unwrap();
        let origin = 200. - 4. * (car - 1) as f64 + if parts[2] == "1" { 1.2 } else { -1.2 };
        let c = measured(row);
        assert!((c.sample.station_m - origin - wheel.center_local_m.x).abs() < 1e-11);
        assert!(
            (c.sample.wheel_center_m
                - Vec3::new(
                    origin + wheel.center_local_m.x,
                    0.18,
                    wheel.center_local_m.z
                ))
            .length()
                < 1e-11
        );
    }
    w[0].center_local_m = w[0].center_local_m + Vec3::new(0., 0.02, 0.03);
    let bad = diagnose(&train, &poses, &g, &w, Default::default()).unwrap();
    let expected = 0.02_f64.hypot(0.03);
    assert!((bad.max_error_m.unwrap() - expected).abs() < 1e-12);
    assert!((bad.rms_error_m.unwrap() - expected / 4.).abs() < 1e-12);
    assert!((measured(&bad.wheels[0]).vertical_error_m.unwrap() - 0.02).abs() < 1e-12);
    assert!((measured(&bad.wheels[0]).lateral_error_m.unwrap() - 0.03).abs() < 1e-12);
    assert_eq!(bad.joints, r.joints);
    assert!(bad.max_error_m.unwrap() > 3. * bad.rms_error_m.unwrap());
}

#[test]
fn tight_banked_twisting_and_suspended_configurations_are_observed_without_fitting() {
    let train = articulation::conventional(1);
    let tight = articulation::arc(8., 0., 120., false);
    let banked = articulation::arc(12., 0.6, 120., false);
    let transition = articulation::arc(12., 0.6, 120., true);
    for (name, t) in [
        ("tight-horizontal", &tight),
        ("banked-turn", &banked),
        ("bank-transition", &transition),
    ] {
        let r = characterize(name, &train, t, 25.);
        assert!(r.max_error_m.unwrap() > 1e-4);
    }
    for (twist, acceleration) in [(0.04, 0.), (0.02, 0.001)] {
        let t = rail_fixtures::sampled(60., 0.02, |s| {
            rail_fixtures::helix(s, 8., 0.3, 0.4 + twist * s + acceleration * s * s)
        });
        let r = characterize(&format!("helix-{twist}-{acceleration}"), &train, &t, 25.);
        assert!(r.max_error_m.unwrap() > 1e-3);
        // Independently evaluate the intended analytic locus at each result's
        // station, instead of comparing two calls to the same target function.
        for row in &r.wheels {
            let c = measured(row);
            let s = c.sample.station_m;
            let z = if row.id.ends_with("--0.6") { -0.6 } else { 0.6 };
            let p = rail_fixtures::helix_locus(
                s,
                8.,
                0.3,
                0.4 + twist * s + acceleration * s * s,
                Vec3::new(0., 0.18, z),
            );
            assert!((p - c.sample.wheel_center_m).length() < 5e-7);
        }
    }
    let below = Train::development_consist(
        "suspended-test".into(),
        2,
        DevelopmentArrangement::BelowTrack,
    )
    .unwrap();
    characterize("suspended-bank-transition", &below, &transition, 25.);
}

#[test]
fn asymmetric_pivots_multiple_wheel_roles_and_visual_independence() {
    let t = articulation::arc(12., 0.6, 120., false);
    let mut train = articulation::conventional(1);
    for (i, j) in train.connections.iter_mut().enumerate() {
        j.anchor_a_m = if i == 0 {
            Vec3::new(1.5, -0.7, 0.1)
        } else {
            Vec3::new(-0.8, -0.9, -0.05)
        };
        j.anchor_b_m = if i == 0 {
            Vec3::new(0.05, 0.15, 0.)
        } else {
            Vec3::new(-0.03, 0.1, 0.)
        };
    }
    characterize("offset-asymmetric-pivots", &train, &t, 25.);
    let flat = articulation::straight();
    let mut train = articulation::conventional(1);
    let p = inspect(&train, &flat, 100.).unwrap();
    let g = PhysicalRailGeometry::new(&flat, LayoutMode::Open).unwrap();
    let mut w = wheels(&train, &p);
    for (role, rotation, center) in [
        (
            WheelRole::SideGuide,
            std::f64::consts::FRAC_PI_2,
            Vec3::new(0.07, 0., 0.8),
        ),
        (
            WheelRole::UpStop,
            std::f64::consts::PI,
            Vec3::new(-0.15, -0.15, 0.6),
        ),
    ] {
        let mut wheel = w[0].clone();
        wheel.id = format!("extra-{role:?}");
        wheel.role = Some(role);
        wheel.center_local_m = center;
        wheel.orientation_local = Quaternion::axis_angle(Vec3::X, rotation).unwrap();
        let WheelTarget::Declared(path) = &mut wheel.target else {
            unreachable!()
        };
        path.rail.offset_m = Vec3::new(0., 0., 0.6);
        path.center_offset_m = Vec3::new(0., center.y, center.z - 0.6);
        w.push(wheel);
    }
    let r = diagnose(&train, &p, &g, &w, Default::default()).unwrap();
    check_world(&w, &p, &r);
    assert!(r.max_error_m.unwrap() < 1e-12);
    train.components[1].asset = Some(qx_document::train::Asset {
        path: "deliberately-unloaded.glb".into(),
        translation_m: Vec3::new(4., 5., 6.),
        rotation: Quaternion::axis_angle(Vec3::Y, 1.).unwrap(),
        scale: Vec3::new(3., 2., 1.),
        pivot_m: Vec3::X,
    });
    train.components[1].center_of_mass_m = Vec3::new(0.2, -0.1, 0.03);
    assert_eq!(r, diagnose(&train, &p, &g, &w, Default::default()).unwrap());
}

#[test]
fn rigid_bogie_twist_incompatibility_is_geometric_not_numerical() {
    let k = 1.;
    let t = rail_fixtures::sampled(20., 0.02, |s| {
        (Vec3::X * s, Quaternion::axis_angle(Vec3::X, k * s).unwrap())
    });
    let train = articulation::conventional(1);
    // Explicit read-only test pose, with wheel rows separated by 1 m. No solver
    // is asked to deform this gear to meet the twisting wheel-center paths.
    let mut p =
        qx_physics::consist::diagnostic_seed(&train, &articulation::straight(), 10.).unwrap();
    let gear = p
        .components
        .iter_mut()
        .find(|p| p.id == "gear-1-1")
        .unwrap();
    gear.pose.position_m = Vec3::X * 10.;
    gear.pose.orientation = Quaternion::axis_angle(Vec3::X, 10.).unwrap();
    let mut w = wheels(&train, &p);
    w.retain(|w| w.component_id == "gear-1-1");
    for wheel in &mut w {
        wheel.center_local_m.x = if wheel.center_local_m.x < 0. {
            -0.5
        } else {
            0.5
        };
        wheel.window = StationWindow {
            min_m: 8.,
            max_m: 12.,
        };
    }
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let r = diagnose(&train, &p, &g, &w, Default::default()).unwrap();
    let refined = diagnose(
        &train,
        &p,
        &g,
        &w,
        SearchOptions {
            approximation_m: 1e-8,
            max_segments: 16384,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.max_error_m.unwrap() > 0.15);
    assert!((r.max_error_m.unwrap() - refined.max_error_m.unwrap()).abs() < 1e-5);
    // Necessary rigidity test at the symmetric intended stations. Two
    // rows on one rail have a different separation from their local
    // wheel centers, so these four exact targets cannot form this rigid bogie.
    let local_a = Vec3::new(-0.5, 0.18, 0.6);
    let local_b = Vec3::new(0.5, 0.18, 0.6);
    let expected = |s: f64, z: f64| {
        Vec3::new(
            s,
            0.18 * s.cos() - z * s.sin(),
            0.18 * s.sin() + z * s.cos(),
        )
    };
    let mismatch =
        ((expected(9.5, 0.6) - expected(10.5, 0.6)).length() - (local_a - local_b).length()).abs();
    assert!(mismatch > 0.05);
    println!(
        "rigid-twist-incompatible: rms={:.9e} max={:.9e} refined_max={:.9e} prescribed_target_pair_mismatch={mismatch:.9e}",
        r.rms_error_m.unwrap(),
        r.max_error_m.unwrap(),
        refined.max_error_m.unwrap()
    );
}

#[test]
fn unavailable_invalid_and_boundary_targets_do_not_produce_successful_averages() {
    let train = articulation::conventional(1);
    let t = articulation::straight();
    let p = inspect(&train, &t, 100.).unwrap();
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let mut w = wheels(&train, &p);
    w[0].target = WheelTarget::Unavailable("no mechanical tread/radius metadata".into());
    if let WheelTarget::Declared(path) = &mut w[1].target {
        path.center_offset_m.y = f64::NAN;
    }
    w[2].window = StationWindow {
        min_m: 0.,
        max_m: 1.,
    };
    let r = diagnose(&train, &p, &g, &w, Default::default()).unwrap();
    assert!(matches!(
        r.wheels[0].target,
        TargetObservation::Unavailable(_)
    ));
    assert!(matches!(r.wheels[1].target, TargetObservation::Invalid(_)));
    let TargetObservation::Measured(n) = &r.wheels[2].target else {
        panic!()
    };
    assert_eq!(n.status, SearchStatus::BoundaryMinimum);
    assert!(n.candidates[0].separation_m > 90.);
    assert!(r.rms_error_m.is_none() && r.max_error_m.is_none());
    assert!(
        diagnose(&train, &p, &g, &[], Default::default())
            .unwrap()
            .max_error_m
            .is_none()
    );
    w[0].orientation_local.w = 0.;
    assert!(diagnose(&train, &p, &g, &w, Default::default()).is_err());
}

#[test]
fn m2b_multicar_closure_is_unchanged_by_wheel_diagnostics() {
    for changing in [false, true] {
        let track = articulation::arc(45., 0.25, 500., changing);
        for count in [2, 8, 16, 32] {
            characterize(
                &format!("m2b-changing-{changing}-{count}"),
                &articulation::conventional(count),
                &track,
                200.,
            );
        }
    }
}

#[test]
fn loop_and_inverted_running_gear_world_centers_are_independent() {
    let r = 8.;
    let t = rail_fixtures::sampled(std::f64::consts::TAU * r, 0.02, |s| {
        let a = s / r;
        (
            Vec3::new(r * a.sin(), r * (1. - a.cos()), 0.),
            Quaternion::axis_angle(Vec3::Z, a).unwrap(),
        )
    });
    characterize(
        "vertical-loop-top",
        &articulation::conventional(1),
        &t,
        std::f64::consts::PI * r,
    );
    let mut inverted = articulation::conventional(1);
    // Inverted running gear arrangement, independent of above/below-track car placement.
    for c in &mut inverted.components {
        if c.kind.is_running_gear() {
            c.placement = Some(Placement::Track {
                offset_m: if c.id.ends_with("-1") { 1.2 } else { -1.2 },
                frame: LocalFrame::default(),
            });
        }
    }
    let t = rail_fixtures::sampled(60., 0.1, |s| {
        (
            Vec3::X * s,
            Quaternion::axis_angle(Vec3::X, std::f64::consts::PI).unwrap(),
        )
    });
    let r = characterize("inverted-straight", &inverted, &t, 25.);
    assert!(r.max_error_m.unwrap() < 1e-11);
}

#[test]
fn live_rigid_body_snapshots_and_com_conversion_are_read_only() {
    use qx_physics::TrackSimulation;
    use std::sync::Arc;
    let track = Arc::new(articulation::arc(45., 0.25, 500., true));
    let mut train = articulation::conventional(2);
    train.components[1].center_of_mass_m = Vec3::new(0.1, -0.05, 0.03);
    let mut observed = TrackSimulation::new(train.clone(), track.clone(), 200., 12.).unwrap();
    let mut control = observed.clone();
    let g = PhysicalRailGeometry::new(&track, LayoutMode::Open).unwrap();
    for _ in 0..4 {
        let before = observed.world().clone();
        let guides = observed.guides().to_vec();
        let poses = observed.snapshot().unwrap();
        let w = wheels(&train, &poses);
        let r = diagnose(&train, &poses, &g, &w, Default::default()).unwrap();
        check_world(&w, &poses, &r);
        for row in &r.wheels {
            let index = train
                .components
                .iter()
                .position(|c| c.id == row.component_id)
                .unwrap();
            let body = &observed.world().bodies[index];
            let local = w.iter().find(|w| w.id == row.id).unwrap().center_local_m
                - train.components[index].center_of_mass_m;
            assert!(
                (body.pose.position_m + rotate(body.pose.orientation, local) - row.center_world_m)
                    .length()
                    < 1e-11
            );
        }
        assert_eq!(observed.world(), &before);
        assert_eq!(observed.guides(), guides);
        observed.step().unwrap();
        control.step().unwrap();
        assert_eq!(observed.world(), control.world());
        assert_eq!(observed.snapshot().unwrap(), control.snapshot().unwrap());
        assert_eq!(observed.telemetry(), control.telemetry());
    }
    assert_eq!(qx_physics::FIXED_STEP_SECONDS, 1. / 240.);
}

#[test]
fn total_search_budget_and_ambiguous_targets_keep_aggregates_unavailable() {
    let train = articulation::conventional(1);
    let straight = articulation::straight();
    let poses = inspect(&train, &straight, 10.).unwrap();
    let dense = rail_fixtures::sampled(20., 0.002, |s| (Vec3::X * s, Quaternion::IDENTITY));
    let g = PhysicalRailGeometry::new(&dense, LayoutMode::Open).unwrap();
    let template = wheels(&train, &poses)[0].clone();
    let w: Vec<_> = (0..512)
        .map(|i| WheelGeometry {
            id: format!("budget-wheel-{i}"),
            ..template.clone()
        })
        .collect();
    let report = diagnose(&train, &poses, &g, &w, Default::default()).unwrap();
    assert_eq!(report.segments_tested, 262144);
    assert!(
        report
            .wheels
            .iter()
            .any(|w| matches!(w.target, TargetObservation::BudgetExceeded))
    );
    assert!(report.resolved_wheels < 512);
    assert!(report.rms_error_m.is_none() && report.max_error_m.is_none());

    let r = 3.;
    let length = std::f64::consts::TAU * r;
    let twice = rail_fixtures::sampled(2. * length, 0.02, |s| rail_fixtures::helix(s, r, 0., 0.));
    let g = PhysicalRailGeometry::new(&twice, LayoutMode::Open).unwrap();
    let mut p = poses.clone();
    let gear = p
        .components
        .iter_mut()
        .find(|c| c.id == template.component_id)
        .unwrap();
    gear.pose.position_m = Vec3::new(r * 1_f64.sin(), 0., r * (1. - 1_f64.cos()));
    gear.pose.orientation = Quaternion::IDENTITY;
    let wheel = WheelGeometry {
        center_local_m: Vec3::ZERO,
        target: WheelTarget::Declared(WheelCenterPath {
            rail: RailReference {
                id: "two-laps".into(),
                offset_m: Vec3::ZERO,
            },
            center_offset_m: Vec3::ZERO,
        }),
        window: StationWindow {
            min_m: 1.,
            max_m: length + 5.,
        },
        ..template
    };
    let report = diagnose(
        &train,
        &p,
        &g,
        &[wheel],
        SearchOptions {
            max_span_m: 30.,
            max_segments: 16384,
            ..Default::default()
        },
    )
    .unwrap();
    let TargetObservation::Measured(n) = &report.wheels[0].target else {
        panic!()
    };
    assert_eq!(n.status, SearchStatus::Ambiguous);
    assert_eq!(n.candidates.len(), 2);
    assert!(report.rms_error_m.is_none() && report.max_error_m.is_none());
}

#[test]
fn incompatible_coincident_centers_cannot_fit_separated_parallel_rails() {
    let train = articulation::conventional(1);
    let t = articulation::straight();
    let poses = inspect(&train, &t, 100.).unwrap();
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let mut w = wheels(&train, &poses);
    w.truncate(2);
    w[1].center_local_m = w[0].center_local_m;
    let before = poses.clone();
    let report = diagnose(&train, &poses, &g, &w, Default::default()).unwrap();
    assert_eq!(
        report.wheels[0].center_world_m,
        report.wheels[1].center_world_m
    );
    // The same world point cannot be on two parallel loci 1.2 m apart. The
    // triangle inequality forces at least one error >= .6 m in EVERY pose.
    assert!((report.max_error_m.unwrap() - 1.2).abs() < 1e-12);
    assert!(report.max_error_m.unwrap() >= 0.6);
    assert_eq!(poses, before);
    println!(
        "incompatible-coincident: rms={:.9e} max={:.9e} lower_bound_max=0.6",
        report.rms_error_m.unwrap(),
        report.max_error_m.unwrap()
    );
}

#[test]
#[ignore = "native Release wheel observations; shared-machine timings, not CI thresholds"]
fn release_wheel_diagnostic_performance() {
    for (name, t, count) in [
        ("straight-8", articulation::straight(), 8),
        ("curve-8", articulation::arc(45., 0.25, 500., false), 8),
        ("twisting-32", articulation::arc(45., 0.25, 500., true), 32),
    ] {
        let train = articulation::conventional(count);
        let poses = inspect(&train, &t, 200.).unwrap();
        let w = wheels(&train, &poses);
        let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
        let warm = diagnose(&train, &poses, &g, &w, Default::default()).unwrap();
        assert_eq!(warm.resolved_wheels, w.len());
        for setup in [false, true] {
            let mut durations = vec![];
            for _ in 0..7 {
                let start = Instant::now();
                for _ in 0..5 {
                    if setup {
                        black_box(
                            PhysicalRailGeometry::new(black_box(&t), LayoutMode::Open).unwrap(),
                        );
                    } else {
                        black_box(
                            diagnose(
                                black_box(&train),
                                black_box(&poses),
                                black_box(&g),
                                black_box(&w),
                                Default::default(),
                            )
                            .unwrap(),
                        );
                    }
                }
                durations.push(start.elapsed().as_secs_f64() * 1e6 / 5.);
            }
            durations.sort_by(f64::total_cmp);
            println!(
                "{name} setup={setup} wheels={} segments={} median_us={:.3} min_us={:.3} max_us={:.3}",
                w.len(),
                warm.segments_tested,
                durations[3],
                durations[0],
                durations[6]
            );
        }
    }
}
