use qx_document::{
    operations::*,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::generate,
    topology::LayoutMode,
};
use qx_physics::{FIXED_STEP_SECONDS, TrackSimulation};
use std::sync::Arc;
fn train(cars: usize) -> Train {
    Train::development_consist("test".into(), cars, DevelopmentArrangement::Conventional).unwrap()
}
#[test]
fn transition_acceleration_profile_is_the_actual_longitudinal_force_command() {
    use qx_geometry::transition::{Channel, Function, Transition, Unit};
    let mut op = command(DeviceKind::Launch);
    op.devices[0].acceleration_curve = Some(Channel {
        unit: Unit::MetresPerSecondSquared,
        transitions: vec![Transition {
            id: 1,
            length: 200.,
            start: 0.5,
            end: 5.,
            amplitude: 0.,
            function: Function::QuinticSmoothstep {},
        }],
    });
    op.devices[0].validate().unwrap();
    let mut moving = sim(3, 2., op.clone());
    moving.step().unwrap();
    let telemetry = &moving.telemetry().devices[0];
    assert!(
        telemetry.commanded_acceleration_mps2 > 0.5 && telemetry.commanded_acceleration_mps2 < 5.
    );
    let before = moving.telemetry().speed_mps;
    let curve = op.devices[0].acceleration_curve.as_mut().unwrap();
    curve.transitions[0].start = 5.;
    curve.transitions[0].end = 5.;
    let mut stronger = sim(3, 2., op);
    stronger.step().unwrap();
    assert!((stronger.telemetry().devices[0].commanded_acceleration_mps2 - 5.).abs() < 1e-12);
    assert!(stronger.telemetry().speed_mps > before);
}
fn sim(cars: usize, speed: f64, mut op: Operations) -> TrackSimulation {
    op.physical.initial_speed_mps = speed;
    let t = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "line", 200.)],
        Default::default(),
    )
    .unwrap();
    TrackSimulation::with_operations(train(cars), Arc::new(t), 80., op).unwrap()
}
fn command(kind: DeviceKind) -> Operations {
    Operations {
        devices: vec![TrackDevice::new(1, kind, 0., 200.)],
        ..Default::default()
    }
}
#[test]
fn launch_and_brake_accelerate_actual_multicar_bodies_with_analytic_force_limits() {
    let mut op = command(DeviceKind::Launch);
    op.devices[0].acceleration_mps2 = 2.;
    let mut a = sim(3, 3., op.clone());
    let mut b = a.clone();
    for _ in 0..240 {
        a.step().unwrap();
        b.step().unwrap();
    }
    assert_eq!(a.world(), b.world());
    assert!((a.telemetry().speed_mps - 5.).abs() < 1e-7);
    assert!((a.telemetry().acceleration_mps2.x - 2.).abs() < 1e-6);
    assert!((a.telemetry().elapsed_s - 1.).abs() < 1e-12);
    assert!((a.telemetry().distance_traveled_m - 4.).abs() < 1e-7);
    let mass: f64 = a.world().bodies.iter().map(|b| b.mass.mass_kg).sum();
    op.devices[0].maximum_force_n = mass * 0.5;
    let mut limited = sim(3, 3., op);
    for _ in 0..240 {
        limited.step().unwrap();
    }
    assert!((limited.telemetry().speed_mps - 3.5).abs() < 1e-6);
    assert!(limited.telemetry().devices[0].force_limited);
    assert!((limited.telemetry().applied_device_force_n - mass * 0.5).abs() < 1e-8);
    for speed in [3., -3.] {
        let mut brake = sim(3, speed, command(DeviceKind::Brake));
        for _ in 0..480 {
            brake.step().unwrap();
        }
        assert!(brake.telemetry().speed_mps.abs() < 1e-8);
    }
}
#[test]
fn boundaries_profiles_disabled_devices_and_passive_resistance_obey_domains() {
    let mut d = TrackDevice::new(1, DeviceKind::Launch, 10., 20.);
    assert_eq!(d.local_distance(10., 30., false), Some(0.));
    assert_eq!(d.local_distance(20., 30., false), None);
    assert_eq!(d.local_distance(9.999, 30., false), None);
    d.profile = vec![
        AccelerationKey {
            distance_m: 0.,
            acceleration_mps2: 0.,
        },
        AccelerationKey {
            distance_m: 10.,
            acceleration_mps2: 4.,
        },
    ];
    assert!((d.command(2.5) - 1.).abs() < 1e-12);
    d.end_m = 30.;
    assert!(d.length(40., false).is_err());
    d.fit_profile(40., false).unwrap();
    assert!((d.command(5.) - 1.).abs() < 1e-12);
    let mut op = command(DeviceKind::Launch);
    op.devices[0].enabled = false;
    let mut idle = sim(1, 2., op);
    for _ in 0..24 {
        idle.step().unwrap();
    }
    assert!((idle.telemetry().speed_mps - 2.).abs() < 1e-7);
    let mut op = Operations::default();
    op.physical.rolling_resistance_coefficient = 0.01;
    let mut r = sim(1, 4., op);
    for _ in 0..240 {
        r.step().unwrap();
    }
    assert!((r.telemetry().speed_mps - (4. - 0.0981)).abs() < 1e-6);
}
#[test]
fn spatial_profile_and_partial_train_occupancy_match_independent_momentum_reference() {
    let mut op = Operations::default();
    let mut d = TrackDevice::new(1, DeviceKind::Launch, 65., 80.);
    d.profile = vec![
        AccelerationKey {
            distance_m: 0.,
            acceleration_mps2: 1.,
        },
        AccelerationKey {
            distance_m: 15.,
            acceleration_mps2: 2.5,
        },
    ];
    op.devices.push(d);
    let mut s = sim(3, 2., op);
    let poses = s.snapshot().unwrap();
    let mass: f64 = s.world().bodies.iter().map(|b| b.mass.mass_kg).sum();
    let requested: f64 = poses
        .components
        .iter()
        .zip(&s.world().bodies)
        .filter(|(p, _)| p.station_m >= 65. && p.station_m < 80.)
        .map(|(p, b)| b.mass.mass_kg * (1. + 0.1 * (p.station_m - 65.)))
        .sum();
    assert!(requested > 0.);
    let occupied = poses
        .components
        .iter()
        .filter(|p| p.station_m >= 65. && p.station_m < 80.)
        .count();
    assert!(occupied < poses.components.len());
    s.step().unwrap();
    assert!((s.telemetry().speed_mps - (2. + requested / mass * FIXED_STEP_SECONDS)).abs() < 1e-8);
    assert!((s.telemetry().applied_device_force_n - requested).abs() < 1e-8);
    assert_eq!(s.telemetry().devices[0].occupied_bodies, occupied);
}
#[test]
fn reversal_and_failed_step_preserve_time_body_guides_and_telemetry() {
    let mut op = command(DeviceKind::Launch);
    op.devices[0].reverse_launch = true;
    let mut a = sim(2, 0.2, op);
    for _ in 0..120 {
        a.step().unwrap();
    }
    assert!(a.telemetry().speed_mps < -0.7);
    let t = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "short", 20.)],
        Default::default(),
    )
    .unwrap();
    let mut op = command(DeviceKind::Launch);
    op.devices[0].end_m = 20.;
    op.physical.initial_speed_mps = 20.;
    let mut a = TrackSimulation::with_operations(train(1), Arc::new(t), 10., op).unwrap();
    loop {
        let old = a.clone();
        if a.step().is_err() {
            assert_eq!(a.world(), old.world());
            assert_eq!(a.guides(), old.guides());
            assert_eq!(a.ticks(), old.ticks());
            assert_eq!(a.telemetry(), old.telemetry());
            break;
        }
        assert!(a.ticks() < 240);
    }
    a.reset();
    assert_eq!(a.ticks(), 0);
    assert_eq!(a.telemetry().elapsed_s, 0.);
}
#[test]
fn valid_circuit_seam_wraps_guides_and_wrapped_device_profile_both_directions() {
    let r = 30.;
    let length = r * std::f64::consts::TAU;
    let mut section = Section::straight(1, "circle", length);
    section.kind = SectionKind::Arc {
        radius_m: r,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    let t = Arc::new(
        generate(
            &InitialConditions {
                orientation: Quaternion::IDENTITY,
                position_m: Vec3::ZERO,
                ..Default::default()
            },
            &[section],
            Default::default(),
        )
        .unwrap(),
    );
    let mut op = Operations {
        layout: LayoutMode::Circuit,
        ..Default::default()
    };
    let mut d = TrackDevice::new(1, DeviceKind::Launch, length - 10., 10.);
    d.acceleration_mps2 = 1.;
    d.length(length, true).unwrap();
    assert_eq!(d.local_distance(0., length, true), Some(10.));
    assert_eq!(d.local_distance(length, length, true), Some(10.));
    assert!(d.local_distance(10., length, true).is_none());
    op.devices.push(d);
    for speed in [5., -5.] {
        let mut o = op.clone();
        o.physical.initial_speed_mps = speed;
        let station = if speed > 0. { length - 5. } else { 5. };
        let mut s = TrackSimulation::with_operations(train(1), t.clone(), station, o).unwrap();
        let mut crossed = false;
        let first = s.guides()[0].station_m;
        for _ in 0..360 {
            s.step().unwrap();
            if (s.guides()[0].station_m - first).abs() > length * 0.5 {
                crossed = true;
            }
        }
        assert!(crossed);
        assert_eq!(s.ticks(), 360);
        assert!((s.telemetry().elapsed_s - 360. * FIXED_STEP_SECONDS).abs() < 1e-12);
    }
}
