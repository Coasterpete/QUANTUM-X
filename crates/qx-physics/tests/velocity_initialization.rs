//! Production M4J qualification. Synthetic mechanisms/geometry, not ride designs.
use qx_document::{
    operations::Operations,
    train::{DevelopmentArrangement, Train, mechanism::*},
};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Frame, Quaternion, Vec3},
    solver::{GeneratedTrack, generate},
};
use qx_physics::{
    Body, MassProperties, Pose, TrackAttitude, TrackGuide, TrackSimulation, World,
    assembly_diagnostics::velocity_residuals,
    velocity_initialization::{self as vi, Report},
};
use std::{sync::Arc, time::Instant};
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail;

fn track(name: &str) -> Arc<GeneratedTrack> {
    if name == "changing-bank" {
        return Arc::new(rail::sampled(120., 0.05, |s| {
            rail::helix(s, 80., 0., 0.2 + 0.01 * s)
        }));
    }
    if name == "transition" {
        let p = qx_document::sample::foundation_project();
        return Arc::new(generate(&p.initial, &p.sections, Default::default()).unwrap());
    }
    let mut initial = InitialConditions::default();
    let mut section = Section::straight(1, "M4J numerical reference", 180.);
    if name == "incline" {
        initial.orientation = Quaternion::axis_angle(Vec3::Z, -0.1).unwrap();
    }
    if name == "circle" || name == "banked" {
        let bank = if name == "banked" { 0.6 } else { 0. };
        initial.orientation = Quaternion::axis_angle(Vec3::X, bank).unwrap();
        section.kind = SectionKind::Arc {
            radius_m: 20.,
            plane_angle_rad: std::f64::consts::FRAC_PI_2 - bank,
        };
    }
    Arc::new(generate(&initial, &[section], Default::default()).unwrap())
}
fn train(n: usize, arrangement: DevelopmentArrangement) -> Train {
    Train::development_consist("m4j-fixture".into(), n, arrangement).unwrap()
}
fn operations(speed: f64) -> Operations {
    let mut op = Operations::default();
    op.physical.initial_speed_mps = speed;
    op.physical.gravity_mps2 = 9.81;
    op
}
fn experimental(train: Train, t: Arc<GeneratedTrack>, station: f64, speed: f64) -> TrackSimulation {
    TrackSimulation::with_experimental_velocity_initialization(train, t, station, operations(speed))
        .unwrap()
}
fn energy(w: &World) -> f64 {
    w.bodies
        .iter()
        .map(|b| {
            let v = b.linear_velocity_mps;
            let w = Frame::new(b.pose.orientation)
                .unwrap()
                .world_to_local(b.angular_velocity_radps);
            let i = b.mass.principal_inertia_kg_m2;
            0.5 * (b.mass.mass_kg * v.dot(v) + i.x * w.x * w.x + i.y * w.y * w.y + i.z * w.z * w.z)
                + b.mass.mass_kg * 9.81 * b.pose.position_m.y
        })
        .sum()
}
fn residual(s: &TrackSimulation, t: &GeneratedTrack, a: &[TrackAttitude]) -> f64 {
    let r = velocity_residuals(s.world(), t, s.guides(), a).unwrap();
    r.max_joint_anchor_speed_mps
        .max(r.max_guide_transverse_speed_mps)
        .max(r.max_angular_constraint_speed_radps)
        .max(r.max_rail_twist_speed_radps)
}
fn emit_reference(
    name: &str,
    seed: &World,
    final_world: &World,
    guides: &[TrackGuide],
    report: &Report,
    bank: f64,
) {
    let bodies = |w: &World| -> Vec<[f64; 17]> {
        w.bodies
            .iter()
            .map(|b| {
                let p = b.pose.position_m;
                let q = b.pose.orientation;
                let v = b.linear_velocity_mps;
                let a = b.angular_velocity_radps;
                let i = b.mass.principal_inertia_kg_m2;
                [
                    p.x,
                    p.y,
                    p.z,
                    q.w,
                    q.x,
                    q.y,
                    q.z,
                    v.x,
                    v.y,
                    v.z,
                    a.x,
                    a.y,
                    a.z,
                    b.mass.mass_kg,
                    i.x,
                    i.y,
                    i.z,
                ]
            })
            .collect()
    };
    let joints: Vec<_> = seed
        .joints
        .iter()
        .map(|j| {
            [
                j.body_a as f64,
                j.body_b as f64,
                j.anchor_a_m.x,
                j.anchor_a_m.y,
                j.anchor_a_m.z,
                j.anchor_b_m.x,
                j.anchor_b_m.y,
                j.anchor_b_m.z,
            ]
        })
        .collect();
    let guides: Vec<_> = guides
        .iter()
        .map(|g| {
            [
                g.body as f64,
                g.local_point_m.x,
                g.local_point_m.y,
                g.local_point_m.z,
                g.station_m,
            ]
        })
        .collect();
    let angular: Vec<_> = seed
        .angular_joints
        .iter()
        .map(|j| {
            assert!(matches!(
                j.model,
                JointModel::RollLock {
                    frame_a: Quaternion::IDENTITY,
                    frame_b: Quaternion::IDENTITY
                }
            ));
            [j.body_a, j.body_b]
        })
        .collect();
    println!(
        "M4J_REFERENCE {{\"name\":\"{name}\",\"bank\":{bank},\"seed\":{:?},\"final\":{:?},\"joints\":{joints:?},\"guides\":{guides:?},\"angular\":{angular:?},\"reference\":{},\"target\":{},\"metric\":{}}}",
        bodies(seed),
        bodies(final_world),
        report.reference_guide,
        report.requested_speed_mps,
        report.correction_metric_j
    );
}

#[test]
fn independent_r20_carriage_speed_contract_and_first_step_transient() {
    for name in ["circle", "banked"] {
        let t = track(name);
        let bank = if name == "banked" { 0.6 } else { 0. };
        let r = 20_f64;
        let half = 0.3_f64;
        let rho = (r * r - half * half).sqrt();
        let s = 15.;
        let theta = s / r;
        let orientation = t.sample_at_distance(s).unwrap().frame.orientation();
        let b = Body {
            mass: MassProperties {
                mass_kg: 100.,
                principal_inertia_kg_m2: Vec3::new(20., 20., 20.),
            },
            pose: Pose {
                position_m: Vec3::new(rho * theta.sin(), 0., r - rho * theta.cos()),
                orientation,
            },
            linear_velocity_mps: Vec3::new(theta.cos(), 0., theta.sin()) * 15.,
            angular_velocity_radps: Vec3::ZERO,
            force_n: Vec3::ZERO,
            torque_nm: Vec3::ZERO,
        };
        let mut legacy = World {
            bodies: vec![b],
            joints: vec![],
            angular_joints: vec![],
            guides: vec![],
            gravity_mps2: Vec3::new(0., -9.81, 0.),
            iterations: 32,
            substeps: 4,
        };
        let guides = vec![
            TrackGuide {
                body: 0,
                local_point_m: Vec3::X * half,
                station_m: s + r * (half / r).asin(),
            },
            TrackGuide {
                body: 0,
                local_point_m: -Vec3::X * half,
                station_m: s - r * (half / r).asin(),
            },
        ];
        let attitudes = vec![TrackAttitude {
            body: 0,
            station_m: s,
            local_frame: Quaternion::IDENTITY,
            local_point_m: Vec3::ZERO,
        }];
        let seed = legacy.clone();
        let mut compatible = seed.clone();
        let report =
            vi::initialize(&mut compatible, &t, &guides, &attitudes, 0, 15., false).unwrap();
        assert!((compatible.bodies[0].linear_velocity_mps.length() - 15. * rho / r).abs() < 1e-8);
        assert!(
            (compatible.bodies[0].angular_velocity_radps - Vec3::new(0., -15. / r, 0.)).length()
                < 1e-8
        );
        assert_eq!(compatible.bodies[0].pose, seed.bodies[0].pose);
        assert!(
            velocity_residuals(&compatible, &t, &guides, &attitudes)
                .unwrap()
                .max_guide_transverse_speed_mps
                < 1e-8
        );
        emit_reference(name, &seed, &compatible, &guides, &report, bank);
        let (e0, e1) = (energy(&legacy), energy(&compatible));
        let d0 = legacy
            .step_on_track(&t, &mut guides.clone(), &mut attitudes.clone())
            .unwrap();
        let d1 = compatible
            .step_on_track(&t, &mut guides.clone(), &mut attitudes.clone())
            .unwrap();
        let (de0, de1) = (energy(&legacy) - e0, energy(&compatible) - e1);
        assert!(de0 < -5. && de1.abs() < 0.03);
        println!(
            "M4J_SINGLE {{\"name\":\"{name}\",\"legacy_delta_j\":{de0},\"experimental_delta_j\":{de1},\"legacy_guide_n\":{:?},\"experimental_guide_n\":{:?}}}",
            d0.guide_force_n
                .iter()
                .map(|v| v.length())
                .collect::<Vec<_>>(),
            d1.guide_force_n
                .iter()
                .map(|v| v.length())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn actual_production_track_matrix_and_exact_reset() {
    for name in [
        "straight",
        "incline",
        "circle",
        "banked",
        "changing-bank",
        "transition",
    ] {
        let t = track(name);
        let station = if name == "transition" { 12.5 } else { 40. };
        for cars in [1, 3] {
            for speed in [0., 6., -6., 15.] {
                let definition = train(cars, DevelopmentArrangement::Conventional);
                let (_, _, attitudes, _) =
                    qx_physics::consist::initialize(&definition, &t, station, Vec3::ZERO, 32, 4)
                        .unwrap();
                let start = Instant::now();
                let mut s = experimental(definition.clone(), t.clone(), station, speed);
                let init_ms = start.elapsed().as_secs_f64() * 1000.;
                let mut legacy = TrackSimulation::with_operations(
                    definition,
                    t.clone(),
                    station,
                    operations(speed),
                )
                .unwrap();
                assert_eq!(s.snapshot().unwrap(), legacy.snapshot().unwrap());
                assert!(residual(&s, &t, &attitudes) < 1e-8);
                assert_eq!(s.ticks(), 0);
                assert!(
                    s.velocity_initialization().unwrap().max_residual <= vi::RESIDUAL_TOLERANCE
                );
                let world = s.world().clone();
                let guides = s.guides().to_vec();
                let telemetry = s.telemetry().clone();
                let report = s.velocity_initialization().unwrap().clone();
                let lead = &report.reference;
                let tangent = t
                    .sample_at_distance(lead.station_m)
                    .unwrap()
                    .frame
                    .tangent();
                let lead_error = |w: &World| {
                    let body = &w.bodies[lead.body];
                    let arm = Frame::new(body.pose.orientation)
                        .unwrap()
                        .local_to_world(lead.local_point_m);
                    (body.linear_velocity_mps + body.angular_velocity_radps.cross(arm)).dot(tangent)
                        - speed
                };
                let legacy_lead_error = lead_error(legacy.world());
                let experimental_lead_error = lead_error(s.world());
                assert!(experimental_lead_error.abs() < 1e-8);
                let seed_residual = residual(&legacy, &t, &attitudes);
                println!(
                    "M4J_SPEED {{\"name\":\"{name}\",\"cars\":{cars},\"speed\":{speed},\"legacy_error_mps\":{legacy_lead_error},\"experimental_error_mps\":{experimental_lead_error},\"stationarity_error\":{}}}",
                    report.max_stationarity_error
                );
                let (e0, e1) = (energy(legacy.world()), energy(s.world()));
                let a = legacy.step().unwrap();
                let b = s.step().unwrap();
                let first = s.world().clone();
                let (de0, de1) = (energy(legacy.world()) - e0, energy(s.world()) - e1);
                println!(
                    "M4J_TRAIN {{\"name\":\"{name}\",\"cars\":{cars},\"speed\":{speed},\"seed_residual\":{},\"final_residual\":{},\"sweeps\":{},\"init_ms\":{init_ms},\"legacy_delta_j\":{de0},\"experimental_delta_j\":{de1},\"legacy_joint_n\":{:?},\"experimental_joint_n\":{:?},\"legacy_guide_n\":{:?},\"experimental_guide_n\":{:?}}}",
                    seed_residual,
                    report.max_residual,
                    report.sweeps,
                    a.joint_force_on_b_n
                        .iter()
                        .map(|v| v.length())
                        .collect::<Vec<_>>(),
                    b.joint_force_on_b_n
                        .iter()
                        .map(|v| v.length())
                        .collect::<Vec<_>>(),
                    a.guide_force_n
                        .iter()
                        .map(|v| v.length())
                        .collect::<Vec<_>>(),
                    b.guide_force_n
                        .iter()
                        .map(|v| v.length())
                        .collect::<Vec<_>>()
                );
                if (name == "circle" || name == "banked") && speed == 15. {
                    assert!(de1.abs() < de0.abs() * 0.1);
                }
                s.reset();
                assert_eq!(s.world(), &world);
                assert_eq!(s.guides(), guides);
                assert_eq!(s.telemetry(), &telemetry);
                assert_eq!(s.ticks(), 0);
                s.step().unwrap();
                assert_eq!(s.world(), &first);
                if speed == 6. {
                    let mut replay = s.clone();
                    for _ in 0..119 {
                        assert_eq!(s.step().unwrap(), replay.step().unwrap());
                    }
                    assert_eq!(s.world(), replay.world());
                    assert_eq!(s.ticks(), 120);
                }
            }
        }
    }
}

#[test]
fn unequal_mass_anisotropic_inertia_off_axis_and_shared_bogie() {
    let t = track("banked");
    let mut definition = train(2, DevelopmentArrangement::Conventional);
    for (i, c) in definition.components.iter_mut().enumerate() {
        c.mass_kg *= 1. + i as f64 * 0.2;
        c.inertia_kg_m2 = Vec3::new(10. + i as f64, 15. + i as f64, 20. + i as f64);
        c.center_of_mass_m = Vec3::new(0.05, 0.07, -0.03);
    }
    let mut s = experimental(definition.clone(), t.clone(), 40., 15.);
    let legacy =
        TrackSimulation::with_operations(definition, t.clone(), 40., operations(15.)).unwrap();
    emit_reference(
        "unequal-off-axis",
        legacy.world(),
        s.world(),
        s.guides(),
        s.velocity_initialization().unwrap(),
        0.6,
    );
    for _ in 0..240 {
        s.step().unwrap();
    }

    let mut shared = train(2, DevelopmentArrangement::Conventional);
    shared
        .components
        .retain(|c| c.id != "gear-2-1" && c.id != "drawbar-1");
    shared.connections.retain(|j| !j.id.starts_with("coupler-"));
    for c in &mut shared.components {
        if (c.id == "car-2" || c.id == "gear-2-2")
            && let Some(Placement::Track { offset_m, .. }) = &mut c.placement
        {
            *offset_m += 1.6;
        }
    }
    for j in &mut shared.connections {
        if j.component_b == "gear-2-1" {
            j.component_b = "gear-1-2".into();
        }
    }
    shared.cars[1].offset_m = 2.4;
    shared.cars[1].bogie_ids[0] = "gear-1-2".into();
    shared.validate().unwrap();
    let mut s = experimental(shared.clone(), t.clone(), 40., 6.);
    assert_eq!(s.world().bodies.len(), 5);
    let legacy = TrackSimulation::with_operations(shared, t.clone(), 40., operations(6.)).unwrap();
    emit_reference(
        "shared",
        legacy.world(),
        s.world(),
        s.guides(),
        s.velocity_initialization().unwrap(),
        0.6,
    );
    for _ in 0..240 {
        s.step().unwrap();
    }
}

#[test]
fn single_point_guide_forces_and_transactional_failures() {
    let t = track("straight");
    let b = Body {
        mass: MassProperties {
            mass_kg: 10.,
            principal_inertia_kg_m2: Vec3::new(1., 2., 2.),
        },
        pose: Pose {
            position_m: Vec3::new(20., 0., 0.),
            orientation: Quaternion::IDENTITY,
        },
        linear_velocity_mps: Vec3::new(3., 2., 1.),
        angular_velocity_radps: Vec3::new(0.2, 0.3, 0.4),
        force_n: Vec3::new(10., 20., 30.),
        torque_nm: Vec3::new(2., 3., 4.),
    };
    let mut w = World {
        bodies: vec![b],
        joints: vec![],
        angular_joints: vec![],
        guides: vec![],
        gravity_mps2: Vec3::Y * -9.81,
        iterations: 32,
        substeps: 4,
    };
    let guides = vec![TrackGuide {
        body: 0,
        local_point_m: Vec3::ZERO,
        station_m: 20.,
    }];
    let seed = w.clone();
    vi::initialize(&mut w, &t, &guides, &[], 0, -3., false).unwrap();
    assert_eq!(w.bodies[0].linear_velocity_mps, Vec3::X * -3.);
    assert_eq!(
        w.bodies[0].angular_velocity_radps,
        seed.bodies[0].angular_velocity_radps
    );
    assert_eq!(w.bodies[0].force_n, seed.bodies[0].force_n);
    assert_eq!(w.bodies[0].torque_nm, seed.bodies[0].torque_nm);
    let before = w.clone();
    let transverse = TrackGuide {
        body: 0,
        local_point_m: Vec3::ZERO,
        station_m: 20.,
    };
    w.guides.push(qx_physics::Guide {
        body: 0,
        local_point_m: Vec3::ZERO,
        line_origin_m: Vec3::X * 20.,
        line_direction: Vec3::Y,
    });
    let locked = w.clone();
    let error = vi::initialize(&mut w, &t, &[transverse], &[], 0, 3., false).unwrap_err();
    assert!(error.contains("infeasible or unconverged"));
    assert_eq!(w, locked);
    w = before.clone();
    for (body, station, velocity) in [(10, 20., 3.), (0, f64::NAN, 3.), (0, 20., f64::NAN)] {
        let bad = vec![TrackGuide {
            body,
            local_point_m: Vec3::ZERO,
            station_m: station,
        }];
        assert!(vi::initialize(&mut w, &t, &bad, &[], 0, velocity, false).is_err());
        assert_eq!(w, before);
    }
    w.bodies = vec![seed.bodies[0].clone(); vi::MAX_BODIES + 1];
    let before = w.clone();
    assert!(
        vi::initialize(&mut w, &t, &guides, &[], 0, 3., false)
            .unwrap_err()
            .contains("bounded")
    );
    assert_eq!(w, before);
}

#[test]
fn passive_and_torque_hinges_reached_limits_and_no_motor_kick() {
    let t = track("straight");
    for bound in [
        None,
        Some(AngleLimits {
            min_rad: 0.,
            max_rad: 1.,
        }),
        Some(AngleLimits {
            min_rad: -1.,
            max_rad: 0.,
        }),
    ] {
        for rate in [-2., 2.] {
            for motor in [false, true] {
                let definition = train(1, DevelopmentArrangement::RotatingSeats);
                let (mut w, g, a, _) =
                    qx_physics::consist::initialize(&definition, &t, 40., Vec3::Y * -9.81, 32, 4)
                        .unwrap();
                let j = w
                    .angular_joints
                    .iter_mut()
                    .find(|j| matches!(j.model, JointModel::Hinge { .. }))
                    .unwrap();
                let seat = j.body_b;
                let car = j.body_a;
                // Put the joint exactly at zero; preserve anchor location, then test
                // its tangent cone, rather than asking velocity projection to seat it.
                let joint = w.joints.iter().find(|j| j.body_b == seat).unwrap();
                let anchor = w.bodies[car].world_point(joint.anchor_a_m).unwrap();
                w.bodies[seat].pose.orientation = w.bodies[car].pose.orientation;
                w.bodies[seat].pose.position_m = anchor
                    - Frame::new(w.bodies[seat].pose.orientation)
                        .unwrap()
                        .local_to_world(joint.anchor_b_m);
                if let JointModel::Hinge { limits, drive, .. } = &mut j.model {
                    *limits = bound;
                    *drive = if motor {
                        RotationDrive::TorqueMotor {
                            target_rad: 0.,
                            stiffness_nm_per_rad: 1000.,
                            damping_nms_per_rad: 50.,
                            max_torque_nm: 100.,
                        }
                    } else {
                        RotationDrive::Free
                    };
                }
                w.bodies[seat].angular_velocity_radps = Vec3::Z * rate;
                w.bodies[seat].linear_velocity_mps = -w.bodies[seat].angular_velocity_radps.cross(
                    Frame::new(w.bodies[seat].pose.orientation)
                        .unwrap()
                        .local_to_world(joint.anchor_b_m),
                );
                let seed = w.clone();
                let r = vi::initialize(&mut w, &t, &g, &a, 0, 0., false).unwrap();
                let actual = (w.bodies[seat].angular_velocity_radps
                    - w.bodies[car].angular_velocity_radps)
                    .z;
                let outward = bound.is_some_and(|b| {
                    (b.min_rad == 0. && rate < 0.) || (b.max_rad == 0. && rate > 0.)
                });
                if outward {
                    assert!(actual.abs() < 1e-8);
                } else {
                    assert!((actual - rate).abs() < 1e-8);
                }
                assert_eq!(
                    w.bodies.iter().map(|b| b.pose).collect::<Vec<_>>(),
                    seed.bodies.iter().map(|b| b.pose).collect::<Vec<_>>()
                );
                assert!(r.max_residual < 1e-8);
            }
        }
    }
}

#[test]
fn launch_brake_startup_and_large_train_bound() {
    use qx_document::operations::{DeviceKind, TrackDevice};
    let t = track("straight");
    for kind in [DeviceKind::Launch, DeviceKind::Brake] {
        for speed in [-3., 0., 3.] {
            let mut op = operations(speed);
            op.devices.push(TrackDevice::new(1, kind, 0., 180.));
            let mut s = TrackSimulation::with_experimental_velocity_initialization(
                train(3, DevelopmentArrangement::Conventional),
                t.clone(),
                80.,
                op,
            )
            .unwrap();
            assert_eq!(s.telemetry().applied_device_force_n, 0.);
            assert_eq!(s.telemetry().elapsed_s, 0.);
            let before = s.world().clone();
            s.step().unwrap();
            let first = s.world().clone();
            s.reset();
            assert_eq!(s.world(), &before);
            s.step().unwrap();
            assert_eq!(s.world(), &first);
        }
    }
    let mut large = experimental(
        train(16, DevelopmentArrangement::Conventional),
        t.clone(),
        100.,
        3.,
    );
    assert_eq!(large.world().bodies.len(), 63);
    large.step().unwrap();
    let error = TrackSimulation::with_experimental_velocity_initialization(
        train(17, DevelopmentArrangement::Conventional),
        t,
        100.,
        operations(3.),
    )
    .unwrap_err();
    assert!(error.contains("bounded"));
}

#[test]
#[ignore = "serial Release initialization versus steady-step timing observations"]
fn initialization_cost_separate_from_steady_stepping() {
    for name in ["straight", "circle"] {
        let t = track(name);
        for cars in [1, 3, 16] {
            let definition = train(cars, DevelopmentArrangement::Conventional);
            let clock = Instant::now();
            let legacy = TrackSimulation::with_operations(
                definition.clone(),
                t.clone(),
                100.,
                operations(6.),
            )
            .unwrap();
            let legacy_ms = clock.elapsed().as_secs_f64() * 1000.;
            let (_, _, attitudes, _) =
                qx_physics::consist::initialize(&definition, &t, 100., Vec3::ZERO, 32, 4).unwrap();
            let reference = legacy
                .guides()
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.station_m.total_cmp(&b.1.station_m))
                .unwrap()
                .0;
            let mut projection_ms = Vec::new();
            let mut last = None;
            for _ in 0..5 {
                let mut w = legacy.world().clone();
                let seed = w.clone();
                let clock = Instant::now();
                let result = vi::initialize(
                    &mut w,
                    &t,
                    legacy.guides(),
                    &attitudes,
                    reference,
                    6.,
                    false,
                );
                projection_ms.push(clock.elapsed().as_secs_f64() * 1000.);
                if result.is_err() {
                    assert_eq!(w, seed);
                }
                last = Some(result);
            }
            match last.unwrap() {
                Err(e) => println!(
                    "M4J_COST {{\"track\":\"{name}\",\"cars\":{cars},\"accepted\":false,\"projection_ms\":{projection_ms:?},\"error\":\"{e}\"}}"
                ),
                Ok(r) => {
                    let mut s = experimental(definition, t.clone(), 100., 6.);
                    let mut times = Vec::new();
                    for _ in 0..240 {
                        let clock = Instant::now();
                        s.step().unwrap();
                        times.push(clock.elapsed().as_secs_f64() * 1000.);
                    }
                    println!(
                        "M4J_COST {{\"track\":\"{name}\",\"cars\":{cars},\"accepted\":true,\"legacy_assembly_ms\":{legacy_ms},\"projection_ms\":{projection_ms:?},\"rows\":{},\"sweeps\":{},\"step_mean_ms\":{},\"step_max_ms\":{}}}",
                        r.rows,
                        r.sweeps,
                        times.iter().sum::<f64>() / times.len() as f64,
                        times.iter().copied().fold(0., f64::max)
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "serial Release four-second production continuation controls"]
fn four_second_production_continuation() {
    for name in ["incline", "circle", "banked", "changing-bank"] {
        let t = track(name);
        for policy in [false, true] {
            let definition = train(3, DevelopmentArrangement::Conventional);
            let mut s = if policy {
                experimental(definition, t.clone(), 40., 15.)
            } else {
                TrackSimulation::with_operations(definition, t.clone(), 40., operations(15.))
                    .unwrap()
            };
            let initial = energy(s.world());
            let mut gaps = [0_f64; 3];
            for _ in 0..960 {
                let d = s.step().unwrap();
                gaps[0] = gaps[0].max(d.max_joint_error_m);
                gaps[1] = gaps[1].max(d.max_guide_error_m);
                gaps[2] = gaps[2].max(d.max_attitude_error_rad);
            }
            assert!(gaps.iter().all(|e| e.is_finite() && *e < 0.02));
            println!(
                "M4J_CONTINUATION {{\"track\":\"{name}\",\"experimental\":{policy},\"ticks\":960,\"energy_change_j\":{},\"max_joint_guide_attitude_gap\":{gaps:?},\"speed_mps\":{}}}",
                energy(s.world()) - initial,
                s.telemetry().speed_mps
            );
        }
    }
}

#[test]
fn fixed_joint_rotated_full_inertia_and_unsupported_transmission() {
    let t = track("straight");
    let q = Quaternion::axis_angle(Vec3::new(1., 2., 3.), 0.7).unwrap();
    let frame = Frame::new(q).unwrap();
    let make = |mass, i, w| Body {
        mass: MassProperties {
            mass_kg: mass,
            principal_inertia_kg_m2: i,
        },
        pose: Pose {
            position_m: Vec3::X * 20.,
            orientation: q,
        },
        linear_velocity_mps: Vec3::new(1., 2., 3.),
        angular_velocity_radps: frame.local_to_world(w),
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    let mut w = World {
        bodies: vec![
            make(10., Vec3::new(1., 2., 2.), Vec3::new(1., 2., 3.)),
            make(20., Vec3::new(2., 3., 4.), Vec3::new(-2., 1., 0.)),
        ],
        joints: vec![qx_physics::BallJoint {
            body_a: 0,
            body_b: 1,
            anchor_a_m: Vec3::new(0.1, 0.2, 0.3),
            anchor_b_m: Vec3::new(0.1, 0.2, 0.3),
        }],
        angular_joints: vec![qx_physics::AngularJoint {
            body_a: 0,
            body_b: 1,
            model: JointModel::Fixed {
                frame_a: Quaternion::IDENTITY,
                frame_b: Quaternion::IDENTITY,
            },
        }],
        guides: vec![],
        gravity_mps2: Vec3::Y * -9.81,
        iterations: 32,
        substeps: 4,
    };
    let guides = vec![TrackGuide {
        body: 0,
        local_point_m: Vec3::ZERO,
        station_m: 20.,
    }];
    w.joints.push(w.joints[0].clone()); // Deliberate redundant production rows.
    vi::initialize(&mut w, &t, &guides, &[], 0, 3., false).unwrap();
    for b in &w.bodies {
        assert!((b.linear_velocity_mps - Vec3::X * 3.).length() < 1e-8);
        assert!(
            (frame.world_to_local(b.angular_velocity_radps) - Vec3::new(-1., 1.4, 1.)).length()
                < 1e-8
        );
    }
    w.angular_joints[0].model = JointModel::Hinge {
        frame_a: Quaternion::IDENTITY,
        frame_b: Quaternion::IDENTITY,
        limits: None,
        drive: RotationDrive::MechanicalProfile {
            domain: ProfileDomain::TimeSeconds,
            transmission_id: "unimplemented".into(),
            keys: vec![
                RotationKey {
                    input: 0.,
                    angle_rad: 0.,
                },
                RotationKey {
                    input: 1.,
                    angle_rad: 0.1,
                },
            ],
        },
    };
    let before = w.clone();
    assert!(
        vi::initialize(&mut w, &t, &guides, &[], 0, 3., false)
            .unwrap_err()
            .contains("not implemented")
    );
    assert_eq!(w, before);
    w.angular_joints[0].model = JointModel::Hinge {
        frame_a: Quaternion::IDENTITY,
        frame_b: Quaternion::IDENTITY,
        limits: Some(AngleLimits {
            min_rad: 0.1,
            max_rad: 1.,
        }),
        drive: RotationDrive::Free,
    };
    let before = w.clone();
    assert!(vi::initialize(&mut w, &t, &guides, &[], 0, 3., false).is_err());
    assert_eq!(w, before);
}

#[test]
fn open_endpoint_outward_velocity_is_rejected_without_end_stop() {
    let t = track("straight");
    let definition = train(1, DevelopmentArrangement::Conventional);
    let (mut w, _, _, _) =
        qx_physics::consist::initialize(&definition, &t, 40., Vec3::ZERO, 32, 4).unwrap();
    w.bodies.truncate(1);
    w.joints.clear();
    w.angular_joints.clear();
    w.bodies[0].pose.position_m = Vec3::ZERO;
    let guides = vec![TrackGuide {
        body: 0,
        local_point_m: Vec3::ZERO,
        station_m: 0.,
    }];
    let before = w.clone();
    assert!(
        vi::initialize(&mut w, &t, &guides, &[], 0, -3., false)
            .unwrap_err()
            .contains("open endpoint")
    );
    assert_eq!(w, before);
    vi::initialize(&mut w, &t, &guides, &[], 0, 3., false).unwrap();
    let before = w.clone();
    let repeated = vec![guides[0].clone(); 512];
    assert!(
        vi::initialize(&mut w, &t, &repeated, &[], 0, 3., false)
            .unwrap_err()
            .contains("row budget")
    );
    assert_eq!(w, before);
    w.bodies[0].mass.mass_kg = -1.;
    let before = w.clone();
    assert!(vi::initialize(&mut w, &t, &guides, &[], 0, 3., false).is_err());
    assert_eq!(w, before);
}
