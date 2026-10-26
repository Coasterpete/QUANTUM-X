use super::{
    graph, panels,
    session::{Session, scene},
};
use qx_document::{command::Command, persistence, sample};
use qx_geometry::{
    authoring::{ChannelKey, SectionId, SectionKind},
    heartline::OffsetProfile,
    math::Vec3,
    solver::GenerationOptions,
};
use qx_rendering::{DisplayOptions, EditorSurface};
use qx_style::Catalog;
use std::time::{Duration, Instant};
fn session() -> Session {
    Session::new(
        sample::foundation_project(),
        None,
        Catalog::built_in().unwrap(),
    )
    .unwrap()
}
fn wait(session: &mut Session) -> Option<qx_rendering::InspectionScene> {
    let timeout = Instant::now() + Duration::from_secs(10);
    while Instant::now() < timeout {
        if let Some(scene) = session.poll() {
            return Some(scene);
        }
        if session.status.starts_with("Generation failed") {
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("generation worker timed out: {}", session.status);
}
#[test]
fn worker_preview_commit_undo_redo_and_generation_error_preserve_document() {
    let mut session = session();
    let original = session.editor.project().clone();
    session.draft = qx_document::editing::resize(&session.draft, 24.).unwrap();
    session.preview();
    let preview = wait(&mut session).unwrap();
    assert!((preview.track.length_m() - 164.).abs() < 1e-8);
    assert_eq!(session.editor.project(), &original);
    let preview_revision = session.revision;
    assert!(session.apply_draft());
    assert_eq!(
        session.revision, preview_revision,
        "commit must reuse identical preview geometry"
    );
    assert_eq!(session.visible_revision, preview_revision);
    assert!(!session.visible_preview);
    assert!(
        session.poll().is_none(),
        "identical commit cannot regenerate"
    );
    session.undo(false);
    wait(&mut session).unwrap();
    assert_eq!(session.editor.project(), &original);
    session.undo(true);
    wait(&mut session).unwrap();
    assert_eq!(session.editor.project().sections[0].extent, 24.);
    session.draft.kind = SectionKind::Arc {
        radius_m: 0.0001,
        plane_angle_rad: 0.,
    };
    let accepted = session.editor.project().clone();
    assert!(!session.apply_draft());
    assert_eq!(session.editor.project(), &accepted);
    assert!(session.status.contains("document/history retained"));
    assert!(session.draft_error.as_ref().unwrap().contains("rotation"));
    session.cancel_section();
    session.undo(false);
    assert!(wait(&mut session).is_some());
}
#[test]
fn generation_coalesces_rapid_edits_and_only_publishes_latest_revision() {
    let mut session = session();
    for index in 0..80 {
        session.draft = qx_document::editing::resize(&session.draft, 12. + index as f64).unwrap();
        session.preview();
    }
    let scene = wait(&mut session).unwrap();
    assert_eq!(session.visible_revision, session.revision);
    assert!((scene.track.length_m() - 231.).abs() < 1e-8);
    assert_eq!(session.editor.project().sections[0].extent, 12.);
}
#[test]
fn failed_open_and_staged_settings_never_discard_other_edits() {
    let mut session = session();
    session.heartline_draft.rider_offset = OffsetProfile::Constant {
        offset_m: Vec3::Y * 1.2,
    };
    session.select(SectionId(2));
    assert_ne!(session.heartline_draft, session.editor.project().heartline);
    session.draft.name = "My edited section".into();
    let draft = session.draft.clone();
    session.execute(vec![Command::SetHeartline(session.heartline_draft.clone())]);
    assert_eq!(session.draft, draft);
    let original = session.editor.project().clone();
    session.open(Some("target/absent-project-file.qx.json".into()), true);
    assert_eq!(session.editor.project(), &original);
    assert_eq!(session.draft, draft);
    assert!(session.status.contains("Open failed"));
    assert!(!session.request_close());
    assert!(session.close_requested);
    session.open(None, false);
    assert!(session.confirm_replace.is_some());
}
#[test]
fn all_insertable_methods_generate_and_save_reopen_without_json_editing() {
    let catalog = Catalog::built_in().unwrap();
    for kind in 0..9 {
        let mut project = sample::foundation_project();
        project.sections = vec![panels::new_section(1, kind)];
        project
            .generate(&catalog, GenerationOptions::default())
            .unwrap();
        let reopened =
            persistence::from_json(&persistence::to_json(&project, &catalog).unwrap(), &catalog)
                .unwrap();
        assert_eq!(project, reopened);
    }
}
#[test]
fn headless_workspace_has_viewport_and_keyboard_apply_undo() {
    let mut session = session();
    let scene = scene(session.editor.project(), &session.catalog, session.options).unwrap();
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    session.draft.name = "Precise authored section".into();
    let mut viewport = egui::Rect::NOTHING;
    for events in [
        vec![],
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        }],
    ] {
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 960.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                viewport = panels::workspace(&mut session, ctx, &scene, &mut display).viewport;
            },
        );
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }
    assert!(
        viewport.width() > 300. && viewport.height() > 200.,
        "{viewport:?}"
    );
    assert_eq!(
        session.editor.project().sections[0].name,
        "Precise authored section"
    );
    session.undo(false);
    assert_ne!(
        session.editor.project().sections[0].name,
        "Precise authored section"
    );
}
#[test]
fn graph_endpoint_drag_changes_solver_input_as_one_undo_transaction() {
    for distance in [-15., -180.] {
        let mut session = session();
        session.select(SectionId(2));
        session.channel = ChannelKey::Normal;
        let original = session.editor.project().clone();
        let scene = scene(&original, &session.catalog, session.options).unwrap();
        let ctx = egui::Context::default();
        let mut handle = None;
        let mut run = |events| {
            let mut output = ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    ctx.root_ui(|root| {
                        egui::CentralPanel::default().show(root, |ui| {
                            let id = egui::Id::unique((
                                "graph endpoint",
                                2_u64,
                                ChannelKey::Normal as u8,
                                1_u64,
                                true,
                            ));
                            graph::editor(&mut session, ui, &scene);
                            handle = ctx.read_response(id).map(|r| r.rect.center());
                        });
                    });
                },
            );
            output.textures_delta.clear();
            handle.unwrap()
        };
        run(vec![]);
        let pos = run(vec![]);
        let moved = pos + egui::vec2(0., distance);
        run(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        run(vec![egui::Event::PointerMoved(moved)]);
        run(vec![egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_ne!(session.editor.project().sections[1], original.sections[1]);
        assert!(session.editor.can_undo());
        session.undo(false);
        assert_eq!(session.editor.project(), &original);
        assert!(!session.editor.can_undo());
    }
}

#[test]
fn workspace_switch_resize_float_and_persist_preserve_document_drafts_and_history() {
    use super::workbench::{Pane, Profile, Workbench};
    let mut session = session();
    session.select(SectionId(2));
    session.draft.name = "Committed before switching".into();
    session.apply_draft();
    let scene = wait(&mut session).unwrap();
    session.draft.name = "Uncommitted section draft".into();
    session.heartline_draft.train_configuration_id = Some("Owner nominal ID".into());
    session.graph_view.view.x = super::plot::Axis::new(0., 5.).unwrap();
    let document = session.editor.project().clone();
    let draft = session.draft.clone();
    let heartline = session.heartline_draft.clone();
    let revision = session.revision;
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    session.workbench.layouts[0].float(Pane::Diagnostics);
    for (profile, size) in [
        (Profile::Track, [1440., 960.]),
        (Profile::Train, [960., 640.]),
        (Profile::Train, [1920., 1080.]),
        (Profile::Track, [960., 640.]),
    ] {
        session.workbench.active = profile;
        for _ in 0..3 {
            let mut viewport = egui::Rect::NOTHING;
            let mut output = ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(size[0], size[1]),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    viewport = panels::workspace(&mut session, ctx, &scene, &mut display).viewport;
                },
            );
            output.textures_delta.clear();
            assert!(!output.shapes.is_empty());
            assert!(
                viewport.is_finite() && viewport.width() >= 150. && viewport.height() >= 100.,
                "{profile:?} {size:?}: {viewport:?}"
            );
        }
        assert_eq!(session.editor.project(), &document);
        assert_eq!(session.draft, draft);
        assert_eq!(session.heartline_draft, heartline);
        assert_eq!(session.selected, SectionId(2));
        assert_eq!(
            session.graph_view.view.x,
            super::plot::Axis::new(0., 5.).unwrap()
        );
        assert_eq!(
            session.revision, revision,
            "layout events must not generate track"
        );
    }
    let bytes = serde_json::to_vec(&session.workbench).unwrap();
    let reopened = Workbench::decode(&bytes).unwrap();
    assert_eq!(reopened.layouts[0].floating[0].pane, Pane::Diagnostics);
    session.sync_draft();
    session.heartline_draft = session.editor.project().heartline.clone();
    session.undo(false);
    assert_ne!(session.editor.project(), &document);
    session.undo(true);
    assert_eq!(session.editor.project(), &document);
}

#[test]
fn previous_spline_projects_survive_both_workspaces_without_conversion() {
    use super::workbench::Profile;
    let catalog = Catalog::built_in().unwrap();
    let project = persistence::load(
        std::path::Path::new("../../examples/spline-comparison.qx.json"),
        &catalog,
    )
    .unwrap();
    let mut session = Session::new(project.clone(), None, catalog).unwrap();
    let scene = scene(&project, &session.catalog, session.options).unwrap();
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    for section in &project.sections {
        session.select(section.id);
        for profile in [Profile::Track, Profile::Train, Profile::Track] {
            session.workbench.active = profile;
            let mut output = ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 960.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    panels::workspace(&mut session, ctx, &scene, &mut display);
                },
            );
            output.textures_delta.clear();
            assert_eq!(session.editor.project(), &project);
            assert_eq!(&session.draft, section);
            assert!(!session.editor.can_undo());
            assert_eq!(session.revision, 0);
        }
    }
}

#[test]
fn keyboard_workspace_switch_and_single_pane_floating_are_recoverable() {
    use super::workbench::{Pane, Profile, Workbench};
    let mut session = session();
    let original = session.editor.project().clone();
    let scene = scene(&original, &session.catalog, session.options).unwrap();
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    for pane in [Pane::Track, Pane::Support, Pane::Transition] {
        session.workbench.layouts[0].float(pane);
    }
    for (key, expected) in [
        (egui::Key::Num2, Profile::Train),
        (egui::Key::Num1, Profile::Track),
    ] {
        for frame in 0..3 {
            let mut output = ctx.run_pass(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200., 800.),
                    )),
                    events: if frame == 0 {
                        vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::CTRL,
                        }]
                    } else {
                        vec![]
                    },
                    ..Default::default()
                },
                |ctx| {
                    panels::workspace(&mut session, ctx, &scene, &mut display);
                },
            );
            output.textures_delta.clear();
            assert_eq!(session.workbench.active, expected);
        }
    }
    session.workbench =
        Workbench::decode(&serde_json::to_vec(&session.workbench).unwrap()).unwrap();
    for pane in [Pane::Track, Pane::Support, Pane::Transition] {
        session.workbench.layouts[0].dock(pane);
        let id = session.workbench.layouts[0].pane_id(pane).unwrap();
        assert!(session.workbench.layouts[0].tree.is_visible(id));
    }
    assert!(session.workbench.layouts[0].floating.is_empty());
    assert_eq!(session.editor.project(), &original);
    assert_eq!(session.revision, 0);
    assert!(!session.editor.can_undo());
}
