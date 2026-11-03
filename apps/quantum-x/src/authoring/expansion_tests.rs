use super::{
    material_editor_scenario::{click, frame},
    ride_camera::{self, Mode},
    session::{Session, blank_project, scene},
    train_playback::Snapshot,
    workbench::Profile,
};
use qx_document::{
    operations::{DeviceKind, TrackDevice},
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    supports::*,
    topology::LayoutMode,
};
use qx_rendering::{DisplayOptions, EditorSurface};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn session() -> Session {
    let mut p = blank_project();
    p.initial.position_m.y = 10.;
    p.sections = vec![Section::straight(1, "lead", 100.)];
    Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap()
}
fn solve(s: &mut Session) {
    let ctx = egui::Context::default();
    let until = Instant::now() + Duration::from_secs(10);
    while s.expansion.completion_job.is_some() {
        s.poll_completion(&ctx);
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn real_editor_device_support_apply_cancel_history_and_reopen_work_together() {
    let mut s = session();
    let original = s.editor.project().clone();
    let sc = scene(&original, &s.catalog, s.options).unwrap();
    let ctx = egui::Context::default();
    s.track_tab = 4;
    for _ in 0..3 {
        frame(&mut s, &ctx, &sc, vec![]);
    }
    click(&mut s, &ctx, &sc, "Add launch", 0).unwrap();
    assert_eq!(s.expansion.operations.devices.len(), 1);
    assert_eq!(s.editor.project(), &original);
    assert!(s.has_drafts());
    click(&mut s, &ctx, &sc, "Create structure", 0).unwrap();
    assert_eq!(s.expansion.supports.len(), 1);
    click(&mut s, &ctx, &sc, "Track mount", 0).unwrap();
    click(&mut s, &ctx, &sc, "Foundation", 0).unwrap();
    let structure = &mut s.expansion.supports[0];
    structure.members.push(Member {
        id: 1,
        start_node: 1,
        end_node: 2,
        profile: MemberProfile::Rectangular {
            width_m: 0.2,
            height_m: 0.3,
        },
        roll_rad: 0.,
    });
    assert!(
        s.apply_expansion(),
        "{}",
        s.expansion.message.as_deref().unwrap_or("")
    );
    let committed = s.editor.project().clone();
    assert!(s.editor.can_undo());
    assert!(!s.has_drafts());
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    s.undo(true);
    assert_eq!(s.editor.project(), &committed);
    s.expansion.operations.devices[0].acceleration_mps2 = 3.;
    s.expansion.supports.clear();
    s.cancel_expansion();
    assert!(!s.has_drafts());
    assert_eq!(s.editor.project(), &committed);
    let json = persistence::to_json(s.editor.project(), &s.catalog).unwrap();
    assert_eq!(
        persistence::from_json(&json, &s.catalog).unwrap(),
        committed
    );
    let rendered = scene(&committed, &s.catalog, s.options).unwrap();
    assert!(
        rendered
            .mesh
            .as_ref()
            .unwrap()
            .components
            .iter()
            .any(|c| c.id == "support/1/1")
    );
    s.select_support(1, 1);
    let mut d = DisplayOptions::default();
    frame(&mut s, &ctx, &rendered, vec![]);
    let mut o = ctx.run_pass(Default::default(), |ctx| {
        s.ui(ctx, &rendered, &mut d);
    });
    o.textures_delta.clear();
    assert_eq!(d.selected_support, Some((1, Some(1))));
}
#[test]
fn completion_worker_apply_cancel_failed_solve_and_stale_results_are_transactional() {
    let mut s = session();
    let mut p = s.editor.project().clone();
    let mut arc = Section::straight(1, "270 degree test arc", 30. * std::f64::consts::PI);
    arc.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    p.sections = vec![arc];
    s = Session::new(p.clone(), None, s.catalog.clone()).unwrap();
    s.expansion.completion_settings.preferred_length_m = 31.5;
    s.expansion.operations.layout = LayoutMode::Circuit;
    s.start_completion();
    solve(&mut s);
    assert!(
        s.expansion.completion.is_some(),
        "{:?}",
        s.expansion.message
    );
    assert_eq!(s.editor.project(), &p);
    let preview = s.preview_project();
    assert!(
        qx_geometry::topology::diagnose(&preview.generate(&s.catalog, s.options).unwrap())
            .unwrap()
            .valid
    );
    s.cancel_expansion();
    assert_eq!(s.editor.project(), &p);
    assert!(!s.has_drafts());
    s.expansion.operations.layout = LayoutMode::Circuit;
    s.start_completion();
    solve(&mut s);
    assert!(s.apply_all_drafts());
    assert_eq!(s.editor.project().sections.len(), 2);
    assert!(!s.has_drafts());
    s.undo(false);
    assert_eq!(s.editor.project(), &p);
    s.expansion.completion_settings.preferred_length_m = 1.;
    s.start_completion();
    solve(&mut s);
    assert!(s.expansion.completion.is_none());
    assert_eq!(s.editor.project(), &p);
    s.expansion.completion_settings.preferred_length_m = 31.5;
    s.start_completion();
    s.draft.name = "new draft".into();
    solve(&mut s);
    assert!(s.expansion.completion.is_none());
    assert_eq!(s.editor.project(), &p);
}
#[test]
fn rider_and_chase_cameras_share_powered_and_unpowered_physics_poses() {
    for powered in [false, true] {
        let mut s = session();
        let mut p = s.editor.project().clone();
        let train =
            Train::development_consist("train".into(), 2, DevelopmentArrangement::Conventional)
                .unwrap();
        p.trains.push(train.clone());
        p.heartline.train_configuration_id = Some(train.id.clone());
        p.operations.physical.initial_speed_mps = 3.;
        if powered {
            p.operations
                .devices
                .push(TrackDevice::new(1, DeviceKind::Launch, 0., 100.));
        }
        s = Session::new(p.clone(), None, s.catalog.clone()).unwrap();
        let sc = scene(&p, &s.catalog, s.options).unwrap();
        let mut sim = qx_physics::TrackSimulation::with_operations(
            train,
            Arc::new(sc.track.clone()),
            40.,
            p.operations.clone(),
        )
        .unwrap();
        for _ in 0..60 {
            sim.step().unwrap();
        }
        let time = Instant::now();
        s.workbench.active = Profile::Train;
        s.playback.snapshot = Some(Snapshot {
            poses: sim.snapshot().unwrap(),
            previous: sim.previous_snapshot().clone(),
            committed_at: time,
            ticks: sim.ticks(),
            running: false,
            diagnostics: None,
            speed_mps: vec![],
            lag_events: 0,
            error: None,
            telemetry: sim.telemetry().clone(),
            timing: Default::default(),
            timeline: vec![],
        });
        let ctx = egui::Context::default();
        let mut d = DisplayOptions::default();
        for mode in [Mode::Rider, Mode::Chase] {
            s.ride.mode = mode;
            let mut output = ctx.run_pass(Default::default(), |ctx| {
                ride_camera::present(&mut s, ctx, &sc, &mut d)
            });
            output.textures_delta.clear();
            assert!(s.ride.error.is_none(), "{:?}", s.ride.error);
            assert!(d.camera_view.is_some());
            assert_eq!(
                s.ride.presentation.as_ref().unwrap(),
                &sim.snapshot().unwrap()
            );
            assert!(!d.asset_instances.is_empty());
        }
        assert!((sim.telemetry().speed_mps - if powered { 3.5 } else { 3. }).abs() < 1e-6);
    }
}
#[test]
fn engineering_station_selection_and_viewport_hover_share_the_inspection_marker() {
    let mut s = session();
    let p = s.editor.project().clone();
    let sc = scene(&p, &s.catalog, s.options).unwrap();
    s.expansion.engineering = true;
    s.expansion.selected_station_m = Some(25.);
    let ctx = egui::Context::default();
    let mut d = DisplayOptions::default();
    let draw = |s: &mut Session, d: &mut DisplayOptions| {
        let mut result = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 960.),
                )),
                ..Default::default()
            },
            |ctx| {
                s.ui(ctx, &sc, d);
            },
        );
        result.textures_delta.clear();
    };
    for _ in 0..4 {
        draw(&mut s, &mut d);
    }
    assert!((d.inspection_distance_m.unwrap() - 25.).abs() < 0.1);
    s.inspect_station(Some((s.selected, 70.)));
    draw(&mut s, &mut d);
    assert!((d.inspection_distance_m.unwrap() - 70.).abs() < 0.1);
    assert_eq!(s.editor.project(), &p);
    assert!(!s.editor.can_undo());
    assert!(!s.has_drafts());
}
