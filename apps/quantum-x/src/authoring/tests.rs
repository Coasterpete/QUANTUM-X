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
    session.apply_draft();
    wait(&mut session).unwrap();
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
    session.apply_draft();
    assert!(wait(&mut session).is_none());
    assert!(session.status.contains("Previous view retained"));
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
                        let id = ui
                            .scope_id()
                            .with((2_u64, ChannelKey::Normal as u8, 1_u64, true));
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
    let moved = pos + egui::vec2(0., -15.);
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
