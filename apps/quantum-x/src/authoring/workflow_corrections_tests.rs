use super::{
    graph_canvas,
    graph_gesture::{Kind, Target},
    ride_camera::{self, Mode},
    session::{Session, blank_project, scene},
    workbench::Profile,
};
use qx_document::{
    operations::{DeviceKind, TrackDevice},
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    heartline::{AuthoredReference, OffsetProfile},
    math::{Frame, Vec3},
    transition::Function,
};
use qx_rendering::DisplayOptions;

fn session() -> Session {
    let mut p = blank_project();
    p.sections = vec![Section::straight(1, "authored line", 150.)];
    p.operations
        .devices
        .push(TrackDevice::new(1, DeviceKind::Launch, 0., 100.));
    p.trains.push(
        Train::development_consist("three cars".into(), 3, DevelopmentArrangement::Conventional)
            .unwrap(),
    );
    p.heartline.train_configuration_id = Some("three cars".into());
    Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap()
}

#[test]
fn precision_launch_and_brake_plateau_gestures_edit_real_command_profiles() {
    for kind in [DeviceKind::Launch, DeviceKind::Brake] {
        let mut s = session();
        s.expansion.operations.devices[0].kind = kind;
        assert!(s.apply_expansion());
        let original = s.editor.project().clone();
        s.open_device_curve(1, 100.);
        let key = s.graph_key();
        let section = qx_document::region_edit::ramp_plateau(&s.graph_section(), key, 0).unwrap();
        s.accept_graph_section(section);
        s.set_graph_transition(1);
        s.workbench.snap.enabled = true;
        s.workbench.snap.graph.acceleration_mps2 = 0.25;
        s.workbench.snap.graph.rate_rad_per_m = 0.000001;
        graph_canvas::fit(&mut s);
        let ctx = egui::Context::default();
        let frame = |s: &mut Session, events| {
            ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    ctx.root_ui(|ui| {
                        super::graph::editor(s, ui, &qx_rendering::InspectionScene::empty())
                    })
                },
            )
            .textures_delta
            .clear();
        };
        for _ in 0..3 {
            frame(&mut s, vec![]);
        }
        let target = Target {
            device: Some(1),
            section: s.graph_id(),
            channel: key,
            transition: 2,
            kind: Kind::Plateau,
        };
        let pos = ctx.read_response(target.id()).unwrap().rect.center();
        let plot = graph_canvas::plot_rect(ctx.read_response(s.graph_canvas_id()).unwrap().rect);
        let scale = s.graph_view.view.y.span() / plot.height() as f64;
        let moved = pos - egui::vec2(0., (0.64 / scale) as f32);
        frame(
            &mut s,
            vec![
                egui::Event::PointerMoved(pos),
                super::m5_tests::pointer(pos, egui::PointerButton::Primary, true),
            ],
        );
        frame(&mut s, vec![egui::Event::PointerMoved(moved)]);
        frame(
            &mut s,
            vec![super::m5_tests::pointer(
                moved,
                egui::PointerButton::Primary,
                false,
            )],
        );
        let d = &s.expansion.operations.devices[0];
        assert!(
            (d.command(50.) - 2.75).abs() < 1e-10,
            "{:?}",
            s.graph_feedback
        );
        assert_eq!(d.kind, kind);
        assert_eq!(
            d.acceleration_curve.as_ref().unwrap().unit,
            qx_geometry::transition::Unit::MetresPerSecondSquared
        );
        assert_eq!(s.editor.project(), &original);
        assert!(s.apply_graph(), "{:?}", s.expansion.message);
        let applied = s.editor.project().clone();
        s.undo(false);
        assert_eq!(s.editor.project(), &original);
        s.undo(true);
        assert_eq!(s.editor.project(), &applied);
        let json = persistence::to_json(&applied, &s.catalog).unwrap();
        let reopened = persistence::from_json(&json, &s.catalog).unwrap();
        assert!((reopened.operations.devices[0].command(50.) - 2.75).abs() < 1e-10);
        s.open_device_curve(1, 100.);
        let section =
            qx_document::region_edit::plateau_level(&s.graph_section(), key, 1, 4.).unwrap();
        s.accept_graph_section(section);
        s.cancel_graph();
        assert_eq!(s.expansion.operations, applied.operations);
    }
}
#[test]
fn device_graph_pointer_numeric_validation_history_and_persistence_share_one_profile() {
    let mut s = session();
    let original = s.editor.project().clone();
    let track_draft = s.draft.clone();
    s.open_device_curve(1, 100.);
    let ctx = egui::Context::default();
    let frame = |s: &mut Session, events| {
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400., 600.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    super::graph::editor(s, ui, &qx_rendering::InspectionScene::empty())
                });
            },
        )
        .textures_delta
        .clear();
    };
    for _ in 0..3 {
        frame(&mut s, vec![]);
    }
    let target = Target {
        device: Some(1),
        section: s.graph_id(),
        channel: s.graph_key(),
        transition: 1,
        kind: Kind::Endpoint { end: true },
    };
    let pos = ctx.read_response(target.id()).unwrap().rect.center();
    let pointer = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    // Native input can batch a complete quick drag into one egui frame.
    let quick = pos - egui::vec2(0., 24.);
    frame(
        &mut s,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, true),
            egui::Event::PointerMoved(quick),
            pointer(quick, false),
        ],
    );
    assert!(
        s.graph_section().roll_rate.transitions[0].end > 2.,
        "batched native drag was lost"
    );
    s.open_device_curve(1, 100.);
    // Let the preceding value/layout update settle before hit-testing a second
    // gesture; an earlier response still describes its preceding painted key.
    for _ in 0..3 {
        frame(&mut s, vec![]);
    }
    let pos = ctx.read_response(target.id()).unwrap().rect.center();
    // Observed native batch: press + movement outside the handle in one frame,
    // followed by release in the next. Widget down-on is false at capture time.
    let moved = pos - egui::vec2(0., 50.);
    frame(
        &mut s,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, true),
            egui::Event::PointerMoved(moved),
        ],
    );
    assert!(
        s.graph_drag.is_some(),
        "native press/move failed to capture: press={pos:?}, handle={:?}, feedback={:?}, status={}, input={:?}",
        ctx.read_response(target.id()).map(|r| r.interact_rect),
        s.graph_feedback,
        s.status,
        ctx.input(|i| i.raw.events.clone())
    );
    frame(&mut s, vec![egui::Event::PointerMoved(moved)]);
    frame(&mut s, vec![pointer(moved, false)]);
    let changed = s.expansion.operations.devices[0]
        .acceleration_curve
        .as_ref()
        .unwrap()
        .transitions[0]
        .end;
    assert!(changed > 2.);
    assert_eq!(s.editor.project(), &original);
    assert_eq!(s.draft, track_draft);
    assert_eq!(s.graph_section().roll_rate.transitions[0].end, changed);
    let valid = s.graph_section();
    let mut invalid = valid.clone();
    invalid.roll_rate.transitions[0].end = -3.;
    assert!(s.set_graph_section(invalid).is_err());
    assert_eq!(s.graph_section(), valid);
    super::graph_regions::split(&mut s, 40.);
    assert_eq!(s.graph_section().roll_rate.transitions.len(), 2);
    let key = s.graph_key();
    let candidate = graph_canvas::boundary(&s.graph_section(), key, 0, 45.).unwrap();
    s.set_graph_section(candidate).unwrap();
    assert!((s.graph_section().roll_rate.transitions[0].length - 45.).abs() < 1e-10);
    assert!(s.apply_graph(), "{:?}", s.expansion.message);
    let applied = s.editor.project().clone();
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    s.undo(true);
    assert_eq!(s.editor.project(), &applied);
    let json = persistence::to_json(&applied, &s.catalog).unwrap();
    assert_eq!(persistence::from_json(&json, &s.catalog).unwrap(), applied);
    s.expansion.curve_device = None;
    assert_eq!(s.graph_section(), track_draft);
    s.open_device_curve(1, 100.);
    let mut c = s.graph_section();
    c.roll_rate.transitions[0].function = Function::Linear {};
    c.roll_rate.transitions[0].amplitude = 0.;
    s.accept_graph_section(c);
    s.cancel_graph();
    assert_eq!(s.editor.project(), &applied);
    assert_eq!(s.expansion.operations, applied.operations);
}

#[test]
fn snap_acceleration_graph_points_boundaries_and_handles_use_metres_and_mps2() {
    use super::snap::Settings;
    use qx_geometry::transition::{Transition, Unit};
    for kind in [
        Kind::Endpoint { end: true },
        Kind::Boundary,
        Kind::Control { index: 3 },
    ] {
        for enabled in [false, true] {
            let mut s = session();
            let original = s.editor.project().clone();
            s.open_device_curve(1, 100.);
            let mut section = s.graph_section();
            section.roll_rate.transitions = vec![Transition::nurbs(1, 100., 2., 3.)];
            s.accept_graph_section(section);
            super::graph_regions::split(&mut s, 40.);
            s.set_graph_transition(0);
            s.workbench.snap = Settings {
                enabled,
                ..Default::default()
            };
            s.workbench.snap.graph.station_m = 3.;
            s.workbench.snap.graph.acceleration_mps2 = 0.25;
            s.workbench.snap.graph.rate_rad_per_m = 0.0001; // adapter units must never determine snapping
            graph_canvas::fit(&mut s);
            let ctx = egui::Context::default();
            let frame = |s: &mut Session, events| {
                ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1400., 600.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        ctx.root_ui(|ui| {
                            super::graph::editor(s, ui, &qx_rendering::InspectionScene::empty())
                        });
                    },
                )
                .textures_delta
                .clear();
            };
            for _ in 0..3 {
                frame(&mut s, vec![]);
            }
            let target = Target {
                device: Some(1),
                section: s.graph_id(),
                channel: s.graph_key(),
                transition: 1,
                kind,
            };
            let pos = ctx
                .read_response(target.id())
                .expect("device handle")
                .rect
                .center();
            let outer = ctx.read_response(s.graph_canvas_id()).unwrap().rect;
            let plot = graph_canvas::plot_rect(outer);
            let before = s.graph_section();
            let t = &before.roll_rate.transitions[0];
            let moved = if kind == Kind::Boundary {
                pos + egui::vec2(17.25, 0.)
            } else {
                pos - egui::vec2(0., 17.25)
            };
            let raw = match kind {
                Kind::Plateau => unreachable!("this test covers point, boundary and shape handles"),
                Kind::Boundary => {
                    40. + (moved.x - pos.x) as f64 * s.graph_view.view.x.span()
                        / plot.width() as f64
                }
                Kind::Endpoint { .. } => {
                    t.end
                        + (pos.y - moved.y) as f64 * s.graph_view.view.y.span()
                            / plot.height() as f64
                }
                Kind::Control { index } => {
                    let Function::Nurbs { profile } = &t.function else {
                        unreachable!()
                    };
                    let mut p = profile.clone();
                    p.refine_for_edit().unwrap();
                    p.values[index]
                        + (pos.y - moved.y) as f64 * s.graph_view.view.y.span()
                            / plot.height() as f64
                }
            };
            let step = if kind == Kind::Boundary { 3. } else { 0.25 };
            let expected = if enabled {
                (raw / step).round() * step
            } else {
                raw
            };
            frame(
                &mut s,
                vec![
                    egui::Event::PointerMoved(pos),
                    super::m5_tests::pointer(pos, egui::PointerButton::Primary, true),
                ],
            );
            frame(&mut s, vec![egui::Event::PointerMoved(moved)]);
            assert!(s.graph_drag.is_some(), "{kind:?}");
            frame(
                &mut s,
                vec![super::m5_tests::pointer(
                    moved,
                    egui::PointerButton::Primary,
                    false,
                )],
            );
            let current = s.graph_section();
            let t = &current.roll_rate.transitions[0];
            let actual = match kind {
                Kind::Plateau => unreachable!("this test covers point, boundary and shape handles"),
                Kind::Boundary => t.length,
                Kind::Endpoint { .. } => t.end,
                Kind::Control { index } => {
                    let Function::Nurbs { profile } = &t.function else {
                        unreachable!()
                    };
                    profile.values[index]
                }
            };
            assert!(
                (actual - expected).abs() < 1e-10,
                "{kind:?}: {actual} != {expected}"
            );
            assert!((current.roll_rate.length() - 100.).abs() < 1e-10);
            assert_eq!(s.editor.project(), &original);
            assert_eq!(s.draft, original.sections[0]);
            assert!(s.apply_graph(), "{:?}", s.expansion.message);
            let applied = s.editor.project().clone();
            assert_eq!(
                applied.operations.devices[0]
                    .acceleration_curve
                    .as_ref()
                    .unwrap()
                    .unit,
                Unit::MetresPerSecondSquared
            );
            s.undo(false);
            assert_eq!(s.editor.project(), &original);
            s.undo(true);
            assert_eq!(s.editor.project(), &applied);
            let json = persistence::to_json(&applied, &s.catalog).unwrap();
            assert_eq!(persistence::from_json(&json, &s.catalog).unwrap(), applied);
        }
    }
}
#[test]
fn rider_and_force_geometry_paths_place_fallback_cars_and_attach_to_identical_poses() {
    for force in [false, true] {
        let mut s = session();
        let mut p = s.editor.project().clone();
        if force {
            p.sections[0].kind = SectionKind::Force {
                normal_g: qx_geometry::transition::Channel::constant(
                    qx_geometry::transition::Unit::G,
                    150.,
                    1.,
                ),
                lateral_g: qx_geometry::transition::Channel::constant(
                    qx_geometry::transition::Unit::G,
                    150.,
                    0.,
                ),
            };
        }
        p.heartline.authored_reference = AuthoredReference::RiderReference;
        p.heartline.rider_offset = OffsetProfile::Constant {
            offset_m: Vec3::new(0., 1.2, 0.),
        };
        s = Session::new(p.clone(), None, s.catalog.clone()).unwrap();
        s.train_station_m = Some(70.);
        s.workbench.active = Profile::Train;
        let scene = scene(&p, &s.catalog, s.options).unwrap();
        let physical = scene.train_track().unwrap();
        assert!(
            (physical.samples[0].position_m.y - scene.track.samples[0].position_m.y + 1.2).abs()
                < 1e-8
        );
        let ctx = egui::Context::default();
        let mut display = DisplayOptions::default();
        for station in [65., 75.] {
            s.train_station_m = Some(station);
            for mode in [Mode::Free, Mode::Rider, Mode::Chase] {
                s.ride.mode = mode;
                ctx.run_pass(Default::default(), |ctx| {
                    ride_camera::present(&mut s, ctx, &scene, &mut display)
                })
                .textures_delta
                .clear();
                assert!(s.ride.error.is_none(), "{:?}", s.ride.error);
                assert_eq!(display.asset_instances.len(), p.trains[0].components.len());
                assert!(display.train_bounds.is_some() && display.selected_car_bounds.is_some());
                assert!(!s.playback.active());
                if mode != Mode::Free {
                    assert!(display.camera_view.is_some());
                }
                let poses = s.ride.presentation.as_ref().unwrap();
                for pose in &poses.components {
                    let instance = display
                        .asset_instances
                        .iter()
                        .find(|i| i.component_id == pose.id)
                        .unwrap();
                    assert!((instance.transform[3][1] - pose.pose.position_m.y).abs() < 1e-12);
                }
                if mode == Mode::Rider {
                    let body = &poses.components[0];
                    let offset = Frame::new(body.pose.orientation)
                        .unwrap()
                        .world_to_local(display.camera_view.unwrap().eye_m - body.pose.position_m);
                    assert!((offset - s.ride.offset_m).length() < 1e-10);
                }
            }
        }
        assert_eq!(s.editor.project(), &p);
        s.playback.initialize(&p, s.options, physical, 70.);
        assert!(s.playback.pending, "{:?}", s.playback.error);
    }
}
