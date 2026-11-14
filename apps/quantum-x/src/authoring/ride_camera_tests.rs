use super::*;
use crate::authoring::{
    session::{blank_project, scene},
    train_playback::Snapshot,
};
use qx_document::{command::Command, train::DevelopmentArrangement};
use qx_geometry::authoring::Section;
use qx_physics::TrackSimulation;
use std::{sync::Arc, time::Duration};

fn session() -> (Session, InspectionScene) {
    let mut p = blank_project();
    p.sections[0] = Section::straight(1, "Ride camera regression incline", 150.);
    p.initial.orientation = Quaternion::axis_angle(Vec3::Z, -0.2).unwrap();
    p.trains.push(
        Train::development_consist(
            "camera train".into(),
            3,
            DevelopmentArrangement::RotatingSeats,
        )
        .unwrap(),
    );
    p.heartline.train_configuration_id = Some("camera train".into());
    let mut s = Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap();
    s.train_station_m = Some(60.);
    s.frame_requested = false;
    let scene = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    (s, scene)
}
#[test]
fn pov_keeps_bank_pitch_yaw_and_inversion_without_world_up_clamping() {
    for roll in [0., 0.7, std::f64::consts::PI, 5.5] {
        let q = Quaternion::axis_angle(Vec3::Y, 0.8)
            .unwrap()
            .compose(Quaternion::axis_angle(Vec3::Z, 1.8).unwrap())
            .compose(Quaternion::axis_angle(Vec3::X, roll).unwrap());
        let pose = Pose {
            position_m: Vec3::new(50., 70., 90.),
            orientation: q,
        };
        let v = attached(pose, Vec3::new(0.3, 1.2, -0.4), Vec3::ZERO, false, false).unwrap();
        let f = Frame::new(q).unwrap();
        assert!((v.forward - f.tangent()).length() < 1e-12);
        assert!((v.up - f.normal()).length() < 1e-12);
        assert!(
            (f.world_to_local(v.eye_m - pose.position_m) - Vec3::new(0.3, 1.2, -0.4)).length()
                < 1e-12
        );
        let c = attached(pose, Vec3::new(-8., 3., 4.), Vec3::ZERO, true, true).unwrap();
        assert!(c.forward.dot(c.up).abs() < 1e-12);
    }
    for angle in [std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2] {
        let p = Pose {
            position_m: Vec3::ZERO,
            orientation: Quaternion::axis_angle(Vec3::Z, angle).unwrap(),
        };
        assert!(attached(p, Vec3::new(-8., 0., 0.), Vec3::ZERO, true, true).is_ok());
    }
}
#[test]
fn all_modes_use_component_ids_and_the_exact_displayed_pose() {
    let (mut s, scene) = session();
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    for mode in Mode::ALL {
        s.ride.mode = mode;
        s.ride.component = Some("seat-2".into());
        let mut output = ctx.run_pass(Default::default(), |ctx| {
            present(&mut s, ctx, &scene, &mut display)
        });
        output.textures_delta.clear();
        if mode == Mode::Free {
            assert!(display.camera_view.is_none());
            continue;
        }
        assert!(
            display.camera_view.is_some(),
            "{mode:?}: {:?}",
            s.ride.error
        );
        if mode == Mode::Track {
            continue;
        }
        let id = match mode {
            Mode::Front => "car-1",
            Mode::Rear => "car-3",
            _ => "seat-2",
        };
        let p = s
            .ride
            .presentation
            .as_ref()
            .unwrap()
            .components
            .iter()
            .find(|p| p.id == id)
            .unwrap();
        let instance = display
            .asset_instances
            .iter()
            .find(|i| i.component_id == id)
            .unwrap();
        assert_eq!(
            [
                p.pose.position_m.x,
                p.pose.position_m.y,
                p.pose.position_m.z
            ],
            instance.transform[3][..3]
        );
        if mode != Mode::Chase {
            let view = display.camera_view.unwrap();
            let local = Frame::new(p.pose.orientation)
                .unwrap()
                .world_to_local(view.eye_m - p.pose.position_m);
            assert!((local - s.ride.offset_m).length() < 1e-12);
        }
    }
    s.ride.mode = Mode::Rider;
    s.ride.component = Some("removed component".into());
    let mut output = ctx.run_pass(Default::default(), |ctx| {
        present(&mut s, ctx, &scene, &mut display)
    });
    output.textures_delta.clear();
    assert!(display.camera_view.is_none());
    assert!(s.ride.error.as_ref().unwrap().contains("removed component"));
}
#[test]
fn rotating_passenger_camera_and_render_interpolation_share_one_time_at_all_frame_rates() {
    let (mut s, scene) = session();
    let train = s.editor.project().trains[0].clone();
    let mut sim = TrackSimulation::new(train, Arc::new(scene.track), 60., 2.).unwrap();
    let now = Instant::now();
    for _ in 0..120 {
        sim.step().unwrap();
    }
    s.playback.snapshot = Some(Snapshot {
        velocity_initialization: None,
        poses: sim.snapshot().unwrap(),
        previous: sim.previous_snapshot().clone(),
        committed_at: now,
        ticks: sim.ticks(),
        running: true,
        diagnostics: None,
        speed_mps: vec![],
        lag_events: 0,
        error: None,
        telemetry: Default::default(),
        timing: Default::default(),
        timeline: vec![],
    });
    s.ride.mode = Mode::Rider;
    s.ride.component = Some("seat-1".into());
    let body = sim.snapshot().unwrap();
    let a = body.components.iter().find(|p| p.id == "seat-1").unwrap();
    let b = body.components.iter().find(|p| p.id == "car-1").unwrap();
    assert!(
        (Frame::new(a.pose.orientation).unwrap().tangent()
            - Frame::new(b.pose.orientation).unwrap().tangent())
        .length()
            > 0.01
    );
    for hz in [30, 60, 144] {
        for sample in 0..hz {
            let time = now
                + Duration::from_secs_f64(
                    sample as f64 / hz as f64 * qx_physics::FIXED_STEP_SECONDS,
                );
            let pose = s.playback.presentation(time).unwrap();
            s.ride.presentation = Some(pose.clone());
            let camera = camera_pose(&s.ride, Some(&s.editor.project().trains[0])).unwrap();
            assert_eq!(
                camera,
                pose.components
                    .iter()
                    .find(|p| p.id == "seat-1")
                    .unwrap()
                    .pose
            );
            let v = attached(camera, s.ride.offset_m, Vec3::ZERO, false, false).unwrap();
            assert!(
                (v.forward - Frame::new(camera.orientation).unwrap().tangent()).length() < 1e-12
            );
        }
    }
    assert_eq!(sim.ticks(), 120);
}
#[test]
fn camera_settings_preview_and_asset_replacement_do_not_change_project_format_or_mechanics() {
    let (mut s, scene) = session();
    let before = s.editor.project().clone();
    let catalog = s.catalog.clone();
    s.ride.mode = Mode::Rear;
    s.ride.offset_m = Vec3::new(0.3, 1.4, 0.2);
    s.ride.preview_playing = true;
    let json = qx_document::persistence::to_json(s.editor.project(), &catalog).unwrap();
    assert_eq!(
        qx_document::persistence::from_json(&json, &catalog).unwrap(),
        before
    );
    let poses = qx_physics::consist::inspect(&before.trains[0], &scene.track, 60.).unwrap();
    let mut trains = before.trains.clone();
    let mut asset = super::super::train_assets::identity("assets/replaced.glb".into());
    asset.scale = Vec3::new(2., 3., 4.);
    asset.pivot_m = Vec3::new(0.2, 0.3, 0.4);
    trains[0].components[0].asset = Some(asset);
    assert!(s.execute(vec![Command::SetTrains(trains)]));
    assert_eq!(
        poses,
        qx_physics::consist::inspect(&s.editor.project().trains[0], &scene.track, 60.).unwrap()
    );
}
#[test]
fn gray_box_creation_and_association_are_one_undoable_transaction() {
    let (mut s, _) = session();
    let before = s.editor.project().clone();
    s.playback.fixture_count = 4;
    add_gray_box(&mut s);
    let p = s.editor.project();
    assert_eq!(p.trains.last().unwrap().cars.len(), 4);
    assert_eq!(
        p.heartline.train_configuration_id.as_ref(),
        Some(&p.trains.last().unwrap().id)
    );
    assert_eq!(p.sections, before.sections);
    s.undo(false);
    assert_eq!(s.editor.project(), &before);
}

fn clock_pose(tick: u64) -> ConsistInspection {
    ConsistInspection {
        components: vec![qx_physics::consist::ComponentPose {
            id: "timed component".into(),
            station_m: tick as f64 / 240.,
            pose: Pose {
                position_m: Vec3::X * (tick as f64 / 60.),
                orientation: Quaternion::axis_angle(Vec3::X, tick as f64 * 0.002).unwrap(),
            },
        }],
        joints: vec![],
    }
}
#[test]
fn buffered_deadlines_remove_completion_time_jitter_at_30_60_and_144_hz() {
    use super::super::train_playback::{PRESENTATION_DELAY_TICKS, State, TimedPose};
    let epoch = Instant::now();
    let dt = qx_physics::FIXED_STEP_SECONDS;
    let mut maximum_error: f64 = 0.;
    let mut legacy_error: f64 = 0.;
    for hz in [30, 60, 144] {
        let mut state = State::default();
        let mut previous_x = -1.;
        let mut previous_up = None;
        for index in 0..hz {
            let t = 0.15 + index as f64 / hz as f64;
            // Unequal solver completion times; packet poses retain the exact
            // fixed-step deadlines rather than assigning these late timestamps.
            let delay = [0.0001, 0.0019, 0.0004, 0.0014][index as usize % 4];
            let latest = ((t - delay) / dt).floor() as u64;
            let at = epoch + Duration::from_secs_f64(latest as f64 * dt);
            state.snapshot = Some(Snapshot {
                velocity_initialization: None,
                poses: clock_pose(latest),
                previous: clock_pose(latest - 1),
                committed_at: at,
                ticks: latest,
                running: true,
                diagnostics: None,
                speed_mps: vec![],
                lag_events: 0,
                error: None,
                telemetry: Default::default(),
                timing: Default::default(),
                timeline: ((latest - 7)..=latest)
                    .map(|tick| TimedPose {
                        at: epoch + Duration::from_secs_f64(tick as f64 * dt),
                        tick,
                        poses: Arc::new(clock_pose(tick)),
                    })
                    .collect(),
            });
            let sample = state
                .presentation_sample(epoch + Duration::from_secs_f64(t))
                .unwrap();
            let expected = t / dt - PRESENTATION_DELAY_TICKS;
            maximum_error = maximum_error.max((sample.tick - expected).abs() * dt);
            assert!(!sample.starved);
            assert!((sample.tick - expected).abs() < 1e-6);
            let pose = sample.poses.components[0].pose;
            assert!((pose.position_m.x - expected / 60.).abs() < 1e-8);
            assert!(pose.position_m.x > previous_x);
            previous_x = pose.position_m.x;
            let up = Frame::new(pose.orientation).unwrap().normal();
            if let Some(previous) = previous_up {
                assert!(up.dot(previous) > 0.999);
            }
            previous_up = Some(up);
            let old_alpha = ((t - latest as f64 * dt - delay) / dt).clamp(0., 1.);
            legacy_error =
                legacy_error.max(((latest as f64 - 1. + old_alpha) - (t / dt - 1.)).abs() * dt);
        }
    }
    assert!(maximum_error < 1e-8);
    assert!(legacy_error > 0.0015);
}
#[test]
fn exhausted_history_is_reported_without_extrapolation_and_pause_reset_show_committed_state() {
    use super::super::train_playback::{State, TimedPose};
    let now = Instant::now();
    let mut state = State::default();
    state.snapshot = Some(Snapshot {
        velocity_initialization: None,
        poses: clock_pose(20),
        previous: clock_pose(19),
        committed_at: now,
        ticks: 20,
        running: true,
        diagnostics: None,
        speed_mps: vec![],
        lag_events: 0,
        error: None,
        telemetry: Default::default(),
        timing: Default::default(),
        timeline: vec![TimedPose {
            at: now,
            tick: 20,
            poses: Arc::new(clock_pose(20)),
        }],
    });
    let sample = state
        .presentation_sample(now + Duration::from_millis(30))
        .unwrap();
    assert!(sample.starved);
    assert_eq!(sample.poses, clock_pose(20));
    state.snapshot.as_mut().unwrap().running = false;
    assert_eq!(
        state.presentation(now + Duration::from_secs(5)).unwrap(),
        clock_pose(20)
    );
    let p = state.snapshot.as_mut().unwrap();
    p.poses = clock_pose(0);
    p.previous = clock_pose(0);
    p.ticks = 0;
    p.timeline.clear();
    assert_eq!(state.presentation(now).unwrap(), clock_pose(0));
}
#[test]
fn a_frame_latches_one_timestamp_and_pose_despite_newer_worker_packets() {
    let (mut s, scene) = session();
    let mut sim = TrackSimulation::new(
        s.editor.project().trains[0].clone(),
        Arc::new(scene.track.clone()),
        60.,
        2.,
    )
    .unwrap();
    sim.step().unwrap();
    let at = Instant::now();
    s.playback.snapshot = Some(Snapshot {
        velocity_initialization: None,
        poses: sim.snapshot().unwrap(),
        previous: sim.previous_snapshot().clone(),
        committed_at: at,
        ticks: 1,
        running: true,
        diagnostics: None,
        speed_mps: vec![],
        lag_events: 0,
        error: None,
        telemetry: Default::default(),
        timing: Default::default(),
        timeline: vec![],
    });
    s.ride.mode = Mode::Rider;
    let ctx = egui::Context::default();
    let mut d = DisplayOptions::default();
    let mut result = ctx.run_pass(Default::default(), |ctx| {
        present(&mut s, ctx, &scene, &mut d);
        let camera = d.camera_view;
        let instances = d.asset_instances.clone();
        let sampled = s.ride.sample_time;
        sim.step().unwrap();
        s.playback.snapshot.as_mut().unwrap().poses = sim.snapshot().unwrap();
        present(&mut s, ctx, &scene, &mut d);
        assert_eq!(sampled, s.ride.sample_time);
        assert_eq!(camera, d.camera_view);
        assert_eq!(instances, d.asset_instances);
    });
    result.textures_delta.clear();
}
#[test]
fn rider_orientation_is_continuous_through_inversions_and_quaternion_sign_changes() {
    let axis = Vec3::new(1., 2., 3.).normalized().unwrap();
    let mut previous = None;
    for step in 0..1260 {
        let angle = step as f64 * 0.01;
        let mut a = clock_pose(step);
        let mut b = clock_pose(step + 1);
        a.components[0].pose.orientation = Quaternion::axis_angle(axis, angle).unwrap();
        let q = Quaternion::axis_angle(axis, angle + 0.01).unwrap();
        b.components[0].pose.orientation = if step % 2 == 0 {
            Quaternion {
                w: -q.w,
                x: -q.x,
                y: -q.y,
                z: -q.z,
            }
        } else {
            q
        };
        let p = qx_physics::consist::interpolate(&a, &b, 0.5)
            .unwrap()
            .components[0]
            .pose;
        let camera = attached(p, Vec3::new(0., 1.2, 0.), Vec3::ZERO, false, false).unwrap();
        let expected = Frame::new(Quaternion::axis_angle(axis, angle + 0.005).unwrap()).unwrap();
        assert!((camera.up - expected.normal()).length() < 1e-12);
        assert!((camera.forward - expected.tangent()).length() < 1e-12);
        if let Some(up) = previous {
            assert!(camera.up.dot(up) > 0.9999);
        }
        previous = Some(camera.up);
    }
}
