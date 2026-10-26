use super::{
    files::{Dialog, FileAction},
    graph, graph_canvas, panels,
    session::{Session, scene},
    timeline,
};
use qx_document::{persistence, sample};
use qx_geometry::authoring::{ChannelKey, SectionId};
use qx_style::Catalog;

pub(super) fn session() -> Session {
    Session::new(
        sample::foundation_project(),
        None,
        Catalog::built_in().unwrap(),
    )
    .unwrap()
}
pub(super) fn pointer(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
pub(super) fn graph_frame(
    session: &mut Session,
    ctx: &egui::Context,
    mut events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> (egui::Pos2, egui::Pos2, egui::Rect) {
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let scene = scene(session.editor.project(), &session.catalog, session.options).unwrap();
    let (mut end, mut boundary, mut canvas) =
        (egui::Pos2::ZERO, egui::Pos2::ZERO, egui::Rect::NOTHING);
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 440.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            panels::shortcuts(session, ctx);
            ctx.root_ui(|root| {
                egui::CentralPanel::default().show(root, |ui| {
                    let end_id = egui::Id::unique((
                        "graph endpoint",
                        2_u64,
                        ChannelKey::Normal as u8,
                        1_u64,
                        true,
                    ));
                    let boundary_id =
                        egui::Id::unique(("boundary", 2_u64, ChannelKey::Normal as u8, 1_u64));
                    graph::editor(session, ui, &scene);
                    end = ctx
                        .read_response(end_id)
                        .map_or(egui::Pos2::ZERO, |r| r.rect.center());
                    boundary = ctx
                        .read_response(boundary_id)
                        .map_or(egui::Pos2::ZERO, |r| r.rect.center());
                    canvas = ctx
                        .read_response(egui::Id::unique((
                            "graph canvas",
                            2_u64,
                            ChannelKey::Normal as u8,
                        )))
                        .unwrap()
                        .rect;
                });
            });
        },
    );
    output.textures_delta.clear();
    (end, boundary, canvas)
}
#[test]
fn graph_draft_gesture_stays_staged_and_escape_restores_prior_draft() {
    for cancel in [false, true] {
        let mut session = session();
        session.select(SectionId(2));
        session.channel = ChannelKey::Normal;
        session.draft.name = "Pending owner name".into();
        let draft = session.draft.clone();
        let project = session.editor.project().clone();
        let ctx = egui::Context::default();
        graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE);
        let pos = graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE).0;
        graph_frame(
            &mut session,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                pointer(pos, egui::PointerButton::Primary, true),
            ],
            egui::Modifiers::NONE,
        );
        let moved = pos - egui::vec2(0., 20.);
        graph_frame(
            &mut session,
            &ctx,
            vec![egui::Event::PointerMoved(moved)],
            egui::Modifiers::NONE,
        );
        assert_ne!(session.draft, draft);
        if cancel {
            graph_frame(
                &mut session,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                egui::Modifiers::NONE,
            );
        }
        graph_frame(
            &mut session,
            &ctx,
            vec![pointer(moved, egui::PointerButton::Primary, false)],
            egui::Modifiers::NONE,
        );
        assert_eq!(session.editor.project(), &project);
        assert!(!session.editor.can_undo());
        if cancel {
            assert_eq!(session.draft, draft);
        } else {
            assert_eq!(session.draft.name, draft.name);
            session.apply_draft();
            session.undo(false);
            assert_eq!(session.editor.project(), &project);
        }
    }
}
#[test]
fn boundary_drag_preserves_extent_and_other_channels_as_one_transaction() {
    for fitted in [false, true] {
        let mut session = session();
        session.select(SectionId(2));
        session.channel = ChannelKey::Normal;
        if fitted {
            graph_canvas::fit(&mut session);
        }
        let project = session.editor.project().clone();
        let ctx = egui::Context::default();
        graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE);
        let pos = graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE).1;
        assert!(pos.is_finite());
        graph_frame(
            &mut session,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                pointer(pos, egui::PointerButton::Primary, true),
            ],
            egui::Modifiers::NONE,
        );
        let moved = pos + egui::vec2(20., 0.);
        graph_frame(
            &mut session,
            &ctx,
            vec![egui::Event::PointerMoved(moved)],
            egui::Modifiers::NONE,
        );
        assert_eq!(session.editor.project(), &project);
        graph_frame(
            &mut session,
            &ctx,
            vec![pointer(moved, egui::PointerButton::Primary, false)],
            egui::Modifiers::NONE,
        );
        assert_ne!(session.editor.project(), &project);
        assert_eq!(session.draft.extent, project.sections[1].extent);
        assert_eq!(session.draft.roll_rate, project.sections[1].roll_rate);
        assert!(graph_canvas::boundary(&session.draft, ChannelKey::Normal, 0, -1.).is_err());
        assert!(graph_canvas::boundary(&session.draft, ChannelKey::Normal, 0, f64::NAN).is_err());
        let new_channel = session.draft.channel_mut(ChannelKey::Normal).unwrap();
        let mut before = project.sections[1].clone();
        let old_channel = before.channel_mut(ChannelKey::Normal).unwrap();
        assert_ne!(
            new_channel.transitions[0].length, old_channel.transitions[0].length,
            "boundary must edit timing after fit={fitted}"
        );
        for (new, old) in new_channel.transitions.iter().zip(&old_channel.transitions) {
            assert_eq!((new.start, new.end), (old.start, old.end));
        }
        session.undo(false);
        assert_eq!(session.editor.project(), &project);
        assert!(!session.editor.can_undo());
    }
}
#[test]
fn graph_wheel_zoom_and_middle_pan_change_only_view_then_fit_recovers() {
    let mut session = session();
    session.select(SectionId(2));
    session.channel = ChannelKey::Normal;
    let project = session.editor.project().clone();
    let ctx = egui::Context::default();
    graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE);
    let pos = graph_frame(&mut session, &ctx, vec![], egui::Modifiers::NONE)
        .2
        .center();
    graph_frame(
        &mut session,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., 80.),
                modifiers: egui::Modifiers::CTRL,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
            },
        ],
        egui::Modifiers::CTRL,
    );
    let range = session.graph_view.view.x;
    assert!(range.max - range.min < session.draft.extent);
    graph_frame(
        &mut session,
        &ctx,
        vec![pointer(pos, egui::PointerButton::Middle, true)],
        egui::Modifiers::NONE,
    );
    let moved = pos + egui::vec2(24., 10.);
    graph_frame(
        &mut session,
        &ctx,
        vec![egui::Event::PointerMoved(moved)],
        egui::Modifiers::NONE,
    );
    graph_frame(
        &mut session,
        &ctx,
        vec![pointer(moved, egui::PointerButton::Middle, false)],
        egui::Modifiers::NONE,
    );
    assert_ne!(session.graph_view.view.x, range);
    assert_eq!(session.editor.project(), &project);
    assert_eq!(session.draft, project.sections[1]);
    assert!(!session.editor.can_undo());
    graph_canvas::fit(&mut session);
    assert_eq!(session.graph_view.view.x.min, 0.);
    assert_eq!(session.graph_view.view.x.max, session.draft.extent);
    assert!(session.graph_view.view.y.max > session.graph_view.view.y.min);
}
#[test]
fn before_after_insertion_reordering_and_draft_guards_share_history() {
    let mut session = session();
    let original = session.editor.project().clone();
    panels::insert_section(&mut session, 3, true);
    let id = session.selected;
    assert_eq!(session.editor.project().sections[0].id, id);
    timeline::move_selected(&mut session, true);
    assert_eq!(session.editor.project().sections[1].id, id);
    assert_eq!(session.selected, id);
    session.undo(false);
    assert_eq!(session.editor.project().sections[0].id, id);
    session.undo(false);
    assert_eq!(session.editor.project(), &original);
    session.draft.name = "Keep staged".into();
    timeline::move_selected(&mut session, true);
    panels::insert_section(&mut session, 3, false);
    assert_eq!(session.editor.project(), &original);
    assert!(session.status.contains("Apply or cancel"));
    session.sync_draft();
    panels::insert_section(&mut session, 3, false);
    let index = session
        .editor
        .project()
        .sections
        .iter()
        .position(|s| s.id == session.selected)
        .unwrap();
    assert_eq!(index, 1);
}
#[test]
fn native_dialog_completion_cancel_and_failure_preserve_document_and_save_identity() {
    let mut session = session();
    let original = session.editor.project().clone();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("qx-m5-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("Précision coaster.qx.json");
    let ctx = egui::Context::default();
    let (send, result) = std::sync::mpsc::channel();
    session.file_dialog = Some(Dialog {
        action: FileAction::SaveAs,
        result,
    });
    send.send(Some(path.clone())).unwrap();
    session.poll_file_dialog(&ctx);
    assert!(!session.dirty());
    assert_eq!(session.path, path.to_string_lossy());
    assert_eq!(
        persistence::load(&path, &session.catalog).unwrap(),
        original
    );
    let original_path = session.path.clone();
    session.save_to(&directory.join("absent parent/fail.qx.json"));
    assert!(session.status.contains("Save failed"));
    assert_eq!(session.path, original_path);
    let (send, result) = std::sync::mpsc::channel();
    session.file_dialog = Some(Dialog {
        action: FileAction::Open,
        result,
    });
    send.send(None).unwrap();
    session.poll_file_dialog(&ctx);
    assert_eq!(session.editor.project(), &original);
    assert_eq!(session.path, original_path);
    let (send, result) = std::sync::mpsc::channel();
    session.file_dialog = Some(Dialog {
        action: FileAction::Open,
        result,
    });
    drop(send);
    session.poll_file_dialog(&ctx);
    assert!(session.status.contains("dialog stopped"));
    assert!(session.file_dialog.is_none());
    session.draft.name = "Unsaved name".into();
    let (send, result) = std::sync::mpsc::channel();
    session.file_dialog = Some(Dialog {
        action: FileAction::Open,
        result,
    });
    send.send(Some(path.clone())).unwrap();
    session.poll_file_dialog(&ctx);
    assert_eq!(session.confirm_replace, Some(Some(path.clone())));
    assert_eq!(session.draft.name, "Unsaved name");
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn clicking_each_section_navigator_updates_the_same_selection_and_respects_drafts() {
    let mut session = session();
    let project = session.editor.project().clone();
    let ctx = egui::Context::default();
    let run = |session: &mut Session, events| {
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
                ctx.root_ui(|root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        timeline::sequence(session, ui);
                        panels::track(session, ui);
                    });
                });
            },
        );
        output.textures_delta.clear();
    };
    run(&mut session, vec![]);
    run(&mut session, vec![]);
    for (timeline, id) in [(true, 2_u64), (false, 1_u64)] {
        let pos = ctx
            .read_response(egui::Id::unique(("section navigator", timeline, id)))
            .unwrap()
            .rect
            .center();
        run(
            &mut session,
            vec![
                egui::Event::PointerMoved(pos),
                pointer(pos, egui::PointerButton::Primary, true),
            ],
        );
        run(
            &mut session,
            vec![pointer(pos, egui::PointerButton::Primary, false)],
        );
        assert_eq!(session.selected, SectionId(id));
        assert_eq!(session.draft.id, session.selected);
    }
    session.draft.name = "Pending precision changes".into();
    let pos = ctx
        .read_response(egui::Id::unique(("section navigator", true, 2_u64)))
        .unwrap()
        .rect
        .center();
    run(
        &mut session,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    run(
        &mut session,
        vec![pointer(pos, egui::PointerButton::Primary, false)],
    );
    assert_eq!(session.selected, SectionId(1));
    assert_eq!(session.draft.name, "Pending precision changes");
    assert_eq!(session.editor.project(), &project);
    assert!(!session.editor.can_undo());
}

#[test]
fn small_windows_at_each_ui_scale_keep_a_bounded_viewport_and_document() {
    use super::workbench::Profile;
    let mut session = session();
    let project = session.editor.project().clone();
    let scene = scene(&project, &session.catalog, session.options).unwrap();
    for scale in [0.75, 1., 1.25, 1.5, 2.] {
        for profile in [Profile::Track, Profile::Train] {
            let ctx = egui::Context::default();
            session.workbench.window.ui_scale = scale;
            session.workbench.active = profile;
            let mut viewport = egui::Rect::NOTHING;
            for _ in 0..5 {
                let mut output = ctx.run_pass(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(960. / scale, 640. / scale),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        viewport =
                            panels::workspace(&mut session, ctx, &scene, &mut Default::default())
                                .viewport;
                    },
                );
                output.textures_delta.clear();
            }
            assert!(
                viewport.is_finite() && viewport.width() > 40. && viewport.height() > 20.,
                "{profile:?} scale {scale}: {viewport:?}"
            );
            assert!(
                viewport.min.x >= 0.
                    && viewport.min.y >= 0.
                    && viewport.max.x <= 960. / scale + 1.
                    && viewport.max.y <= 640. / scale + 1.,
                "{profile:?} scale {scale}: {viewport:?}"
            );
            assert_eq!(session.editor.project(), &project);
            assert_eq!(session.revision, 0);
        }
    }
}
