//! File/draft/history regressions; native dialog results are injected without desktop input.
use super::{
    draft_workflow::{DraftRequest, UiAction},
    files::{Dialog, FileAction},
    session::{Session, blank_project, scene},
    workbench::Pane,
};
use qx_document::{command::Command, persistence};
use qx_rendering::EditorSurface;
use qx_style::{
    Catalog,
    construction::{Profile, TrackConstruction},
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    dir: PathBuf,
    path: PathBuf,
    s: Session,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "qx-save-regression-{}-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Précision coaster.qx.json");
        let catalog = Catalog::built_in().unwrap();
        let p = blank_project();
        persistence::save(&path, &p, &catalog).unwrap();
        let s = Session::new(p, Some(&path), catalog).unwrap();
        Self { dir, path, s }
    }
    fn stage(&mut self, graph: bool, construction: bool) {
        if graph {
            self.s.draft.roll_rate.transitions[0].start = 0.021234567890123;
            self.s.draft.roll_rate.transitions[0].end = 0.021234567890123;
        }
        if construction {
            self.s.construction_draft = TrackConstruction::generic_single();
            self.s.construction_draft.continuous[0].profile = Profile::Box {
                height_m: 0.432123456789,
                width_m: 0.238765432109,
            };
            self.s.construction_draft.materials[0].base_color = [0.23, 0.57, 0.81];
            self.s.construction_draft.materials[0].roughness = 0.43;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
fn complete_dialog(s: &mut Session, action: FileAction, path: Option<PathBuf>) {
    let (send, result) = mpsc::channel();
    s.file_dialog = Some(Dialog { action, result });
    send.send(path).unwrap();
    s.poll_file_dialog(&egui::Context::default());
}

#[test]
fn saving_graph_construction_or_both_requires_explicit_apply_and_roundtrips_every_value() {
    for (graph, construction) in [(true, false), (false, true), (true, true)] {
        let mut f = Fixture::new();
        let original = f.s.editor.project().clone();
        f.stage(graph, construction);
        let intended = f.s.preview_project();
        let expected = scene(&intended, &f.s.catalog, f.s.options).unwrap();
        f.s.save();
        assert_eq!(f.s.draft_request, Some(DraftRequest::Save));
        assert_eq!(
            f.s.draft_names().len(),
            usize::from(graph) + usize::from(construction)
        );
        assert_eq!(f.s.editor.project(), &original);
        assert!(!f.s.editor.can_undo());
        assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
        assert_eq!(f.s.save_state(), "Unapplied drafts");
        assert!(f.s.pane_title(Pane::Track).contains("unapplied"));
        if graph {
            assert!(f.s.pane_title(Pane::Transition).contains("unapplied"));
        }
        f.s.resolve_drafts(true);
        assert!(f.s.draft_request.is_none());
        assert!(!f.s.pending_edits());
        assert_eq!(f.s.save_state(), "Saved");
        let reopened = persistence::load(&f.path, &f.s.catalog).unwrap();
        assert_eq!(reopened.schema_version, qx_document::SCHEMA_VERSION);
        assert_eq!(reopened, intended);
        let actual = scene(&reopened, &f.s.catalog, f.s.options).unwrap();
        assert_eq!(actual.track, expected.track);
        assert_eq!(actual.mesh, expected.mesh);
        f.s.open(Some(f.path.clone()), false);
        assert_eq!(f.s.editor.project(), &intended);
        assert!(!f.s.has_drafts());
    }
}

#[test]
fn apply_all_is_one_history_transaction_and_save_retries_add_no_history() {
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.stage(true, true);
    f.s.metadata_draft.name = "All staged settings".into();
    f.s.initial_draft.reference_speed_mps = 23.123456789;
    f.s.heartline_draft.rider_offset = qx_geometry::heartline::OffsetProfile::Constant {
        offset_m: qx_geometry::math::Vec3::Y * 1.123456789,
    };
    let mut intended = f.s.preview_project();
    intended.metadata = f.s.metadata_draft.clone();
    assert_eq!(f.s.draft_names().len(), 5);
    f.s.save();
    f.s.resolve_drafts(true);
    assert_eq!(f.s.editor.project(), &intended);
    assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), intended);
    f.s.save();
    assert!(f.s.apply_all_drafts());
    f.s.undo(false);
    assert_eq!(f.s.editor.project(), &original);
    assert!(!f.s.editor.can_undo());
    assert_eq!(f.s.save_state(), "Unsaved changes");
    f.s.undo(true);
    assert_eq!(f.s.editor.project(), &intended);
    assert_eq!(f.s.save_state(), "Saved");
}

#[test]
fn invalid_drafts_never_partially_apply_or_write_and_cancel_keeps_edits() {
    for invalid_graph in [true, false] {
        let mut f = Fixture::new();
        let original = f.s.editor.project().clone();
        f.stage(true, true);
        if invalid_graph {
            f.s.draft.extent = -1.;
        } else {
            f.s.construction_draft.continuous[0].profile = Profile::Box {
                height_m: -1.,
                width_m: 0.2,
            };
        }
        let draft = f.s.draft.clone();
        let construction = f.s.construction_draft.clone();
        f.s.save();
        f.s.resolve_drafts(true);
        assert!(
            f.s.resolution_error
                .as_ref()
                .unwrap()
                .contains("No drafts applied")
        );
        assert!(f.s.draft_request.is_some());
        assert_eq!(f.s.editor.project(), &original);
        assert!(!f.s.editor.can_undo());
        assert_eq!(f.s.draft, draft);
        assert_eq!(f.s.construction_draft, construction);
        assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
        // Cancel the save dialog, not the drafts; the UI uses these same fields.
        f.s.draft_request = None;
        f.s.resolution_error = None;
        assert!(f.s.has_drafts());
        assert_eq!(f.s.draft, draft);
        f.s.cancel_all_drafts();
        assert!(!f.s.has_drafts());
        assert!(!f.s.editor.can_undo());
    }
}

#[test]
fn history_explains_draft_block_and_supports_apply_or_explicit_cancel() {
    for redo in [false, true] {
        let mut f = Fixture::new();
        let original = f.s.editor.project().clone();
        f.s.draft.name = "Committed rename".into();
        assert!(f.s.apply_draft());
        let renamed = f.s.editor.project().clone();
        if redo {
            f.s.undo(false);
        }
        f.stage(true, true);
        let committed = f.s.editor.project().clone();
        f.s.undo(redo);
        assert_eq!(f.s.draft_request, Some(DraftRequest::History { redo }));
        assert_eq!(f.s.editor.project(), &committed);
        f.s.resolve_drafts(false);
        assert!(!f.s.has_drafts());
        assert_eq!(
            f.s.editor.project(),
            if redo { &renamed } else { &original }
        );
    }
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.stage(true, true);
    f.s.undo(false);
    f.s.resolve_drafts(true);
    assert_ne!(f.s.editor.project(), &original); // Apply does not immediately undo itself.
    f.s.undo(false);
    assert_eq!(f.s.editor.project(), &original);
    assert!(!f.s.editor.can_undo());
}

#[test]
fn save_as_and_cancelled_picker_preserve_current_file_identity() {
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.stage(true, true);
    f.s.file_dialog(FileAction::SaveAs);
    assert_eq!(f.s.draft_request, Some(DraftRequest::SaveAs));
    assert!(f.s.file_dialog.is_none());
    // Resolve Apply All without launching a native picker in this test.
    assert!(f.s.apply_all_drafts());
    f.s.draft_request = None;
    complete_dialog(&mut f.s, FileAction::SaveAs, None);
    assert_eq!(f.s.path, f.path.to_string_lossy());
    assert!(f.s.dirty());
    assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
    let other = f.dir.join("Save As 空間.qx.json");
    complete_dialog(&mut f.s, FileAction::SaveAs, Some(other.clone()));
    assert_eq!(f.s.path, other.to_string_lossy());
    assert_eq!(
        persistence::load(&other, &f.s.catalog).unwrap(),
        *f.s.editor.project()
    );
    assert!(!f.s.dirty());
    assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
    // Direct Save As destinations are also draft guarded.
    f.stage(true, false);
    f.s.draft.name = "Another draft".into();
    f.s.save_to(&f.path);
    assert_eq!(
        f.s.draft_request,
        Some(DraftRequest::SaveTo(f.path.clone()))
    );
    f.s.resolve_drafts(true);
    assert_eq!(f.s.path, f.path.to_string_lossy());
}

#[test]
fn disk_failure_retains_identity_intended_changes_and_persistent_error_until_retry() {
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.stage(true, true);
    let intended = f.s.preview_project();
    let bad = f.dir.join("missing/project.qx.json");
    f.s.save_to(&bad);
    f.s.resolve_drafts(true);
    assert_eq!(f.s.editor.project(), &intended);
    assert!(!f.s.has_drafts());
    assert!(f.s.dirty());
    assert_eq!(f.s.path, f.path.to_string_lossy());
    assert_eq!(f.s.save_state(), "Save failed");
    assert!(f.s.save_failure.as_ref().unwrap().contains("missing"));
    f.s.regenerate();
    f.s.poll();
    assert_eq!(f.s.save_state(), "Save failed");
    assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
    f.s.save();
    assert_eq!(f.s.save_state(), "Saved");
    f.s.undo(false);
    assert_eq!(f.s.editor.project(), &original);
    assert!(!f.s.editor.can_undo());
}

#[test]
fn open_new_and_exit_guard_committed_changes_drafts_or_both_before_replacement() {
    for (committed, drafts) in [(true, false), (false, true), (true, true)] {
        let mut f = Fixture::new();
        if committed {
            let mut metadata = f.s.editor.project().metadata.clone();
            metadata.author = "Unsaved owner".into();
            assert!(f.s.execute(vec![Command::SetMetadata(metadata)]));
        }
        if drafts {
            f.stage(true, true);
        }
        let before = f.s.editor.project().clone();
        let draft = f.s.draft.clone();
        f.s.file_dialog(FileAction::Open);
        assert!(f.s.open_dialog_requested);
        assert!(f.s.file_dialog.is_none());
        f.s.open_dialog_requested = false; // Keep current project.
        f.s.open(Some(f.path.clone()), false);
        assert_eq!(f.s.confirm_replace, Some(Some(f.path.clone())));
        assert_eq!(f.s.editor.project(), &before);
        assert_eq!(f.s.draft, draft);
        f.s.confirm_replace = None;
        f.s.open(None, false);
        assert_eq!(f.s.confirm_replace, Some(None));
        assert!(!f.s.request_close());
        assert!(!f.s.close_approved);
        f.s.close_requested = false;
        // Explicit discard authorization still cannot destroy work on a bad load.
        f.s.open(Some(f.dir.join("missing.qx.json")), true);
        assert_eq!(f.s.editor.project(), &before);
        assert_eq!(f.s.draft, draft);
        assert!(f.s.status.contains("Open failed"));
        f.s.confirm_replace = None;
        f.s.open(Some(f.path.clone()), true);
        assert!(!f.s.pending_edits());
        assert!(f.s.request_close());
    }
}

#[test]
fn save_dispatch_reads_latest_widget_draft_and_untitled_save_resolves_before_picker() {
    let mut f = Fixture::new();
    f.s.ui_action = Some(UiAction::Save);
    // Numeric fields consume native input after the command header and shortcuts.
    f.stage(true, true);
    f.s.run_ui_action();
    assert_eq!(f.s.draft_request, Some(DraftRequest::Save));
    assert!(f.s.has_drafts());
    f.s.path.clear();
    f.s.draft_request = None;
    f.s.save();
    assert_eq!(f.s.draft_request, Some(DraftRequest::Save));
    assert!(f.s.file_dialog.is_none());
}

#[cfg(windows)]
#[test]
fn locked_destination_failure_preserves_old_bytes_and_cleans_temporary_file() {
    use std::os::windows::fs::OpenOptionsExt;
    let mut f = Fixture::new();
    let bytes = std::fs::read(&f.path).unwrap();
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&f.path)
        .unwrap();
    f.stage(true, true);
    f.s.save();
    f.s.resolve_drafts(true);
    assert_eq!(f.s.save_state(), "Save failed");
    assert_eq!(std::fs::read_dir(&f.dir).unwrap().count(), 1);
    drop(locked);
    assert_eq!(std::fs::read(&f.path).unwrap(), bytes);
    f.s.save();
    assert_eq!(f.s.save_state(), "Saved");
    assert_ne!(std::fs::read(&f.path).unwrap(), bytes);
}

fn ui_frame(
    s: &mut Session,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let scene = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            s.ui(ctx, &scene, &mut Default::default());
        },
    );
    let texts = output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                Some((
                    t.galley.job.text.clone(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                ))
            } else {
                None
            }
        })
        .collect();
    output.textures_delta.clear();
    texts
}
fn ui_click_at(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, pos: egui::Pos2) {
    ui_frame(
        s,
        ctx,
        size,
        vec![
            egui::Event::PointerMoved(pos),
            super::m5_tests::pointer(pos, egui::PointerButton::Primary, true),
        ],
    );
    ui_frame(
        s,
        ctx,
        size,
        vec![super::m5_tests::pointer(
            pos,
            egui::PointerButton::Primary,
            false,
        )],
    );
}
fn ui_click(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, text: &str) {
    for _ in 0..3 {
        ui_frame(s, ctx, size, vec![]);
    }
    let texts = ui_frame(s, ctx, size, vec![]);
    let pos = texts
        .iter()
        .rev()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("Missing {text}: {texts:?}"))
        .1
        .center();
    ui_click_at(s, ctx, size, pos);
}
fn ctrl_key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::CTRL,
    }
}

#[test]
fn actual_save_modal_lists_both_drafts_cancel_retains_them_and_apply_saves_at_both_sizes() {
    for size in [egui::vec2(960., 640.), egui::vec2(1600., 1000.)] {
        let mut f = Fixture::new();
        let original = f.s.editor.project().clone();
        let ctx = egui::Context::default();
        f.stage(true, true);
        let intended = f.s.preview_project();
        ui_frame(&mut f.s, &ctx, size, vec![]);
        ui_click(&mut f.s, &ctx, size, "Save");
        let texts = ui_frame(&mut f.s, &ctx, size, vec![]);
        assert!(texts.iter().any(|(t, _)| t.contains("Section/graph 1")));
        assert!(texts.iter().any(|(t, _)| t.contains("Track construction")));
        assert_eq!(f.s.draft_request, Some(DraftRequest::Save));
        // Keyboard actions cannot bypass the active modal.
        ui_frame(&mut f.s, &ctx, size, vec![ctrl_key(egui::Key::N)]);
        assert!(f.s.confirm_replace.is_none());
        ui_click(&mut f.s, &ctx, size, "Keep editing / Cancel");
        assert!(f.s.has_drafts());
        assert_eq!(f.s.editor.project(), &original);
        assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), original);
        ui_click(&mut f.s, &ctx, size, "Save");
        ui_click(&mut f.s, &ctx, size, "Apply All & Save");
        assert!(!f.s.pending_edits());
        assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), intended);
        f.s.undo(false);
        assert_eq!(f.s.editor.project(), &original);
        assert!(!f.s.editor.can_undo());
    }
}

#[test]
fn actual_numeric_typing_and_ctrl_s_in_same_batch_save_the_latest_input() {
    let mut f = Fixture::new();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    for _ in 0..3 {
        ui_frame(&mut f.s, &ctx, size, vec![]);
    }
    let texts = ui_frame(&mut f.s, &ctx, size, vec![]);
    let label = texts.iter().find(|(t, _)| t == "Length (m)").unwrap().1;
    ui_click_at(
        &mut f.s,
        &ctx,
        size,
        egui::pos2(label.right() + 24., label.center().y),
    );
    ui_frame(
        &mut f.s,
        &ctx,
        size,
        vec![egui::Event::Text("27.125".into()), ctrl_key(egui::Key::S)],
    );
    assert_eq!(f.s.draft.extent, 27.125);
    assert_eq!(f.s.draft_request, Some(DraftRequest::Save));
    assert_eq!(
        persistence::load(&f.path, &f.s.catalog).unwrap().sections[0].extent,
        10.
    );
    ui_click(&mut f.s, &ctx, size, "Apply All & Save");
    assert_eq!(
        persistence::load(&f.path, &f.s.catalog).unwrap().sections[0].extent,
        27.125
    );
}

#[test]
fn actual_history_and_replacement_prompts_offer_safe_choices() {
    let mut f = Fixture::new();
    let ctx = egui::Context::default();
    let size = egui::vec2(1280., 900.);
    let original = f.s.editor.project().clone();
    f.s.draft.name = "Committed edit".into();
    assert!(f.s.apply_draft());
    f.stage(true, true);
    ui_frame(&mut f.s, &ctx, size, vec![]);
    ui_click(&mut f.s, &ctx, size, "Undo");
    ui_click(&mut f.s, &ctx, size, "Cancel drafts & continue history");
    assert_eq!(f.s.editor.project(), &original);
    assert!(!f.s.has_drafts());
    f.stage(true, true);
    ui_click(&mut f.s, &ctx, size, "Open");
    assert!(f.s.open_dialog_requested);
    assert!(f.s.file_dialog.is_none());
    ui_click(&mut f.s, &ctx, size, "Keep current project");
    assert!(f.s.has_drafts());
    ui_click(&mut f.s, &ctx, size, "New");
    ui_click(&mut f.s, &ctx, size, "Keep current project");
    assert!(f.s.has_drafts());
    assert!(!f.s.request_close());
    ui_click(&mut f.s, &ctx, size, "Keep working");
    assert!(!f.s.close_approved);
    assert!(f.s.has_drafts());
}

#[test]
fn confirmed_open_picker_cancel_or_failure_retains_edits_and_resets_authorization() {
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.stage(true, true);
    let draft = f.s.draft.clone();
    for path in [None, Some(f.dir.join("missing.qx.json"))] {
        f.s.open_dialog_confirmed = true;
        complete_dialog(&mut f.s, FileAction::Open, path);
        assert!(!f.s.open_dialog_confirmed);
        assert_eq!(f.s.editor.project(), &original);
        assert_eq!(f.s.draft, draft);
        assert!(f.s.has_drafts());
    }
    f.s.file_dialog(FileAction::Open);
    assert!(f.s.open_dialog_requested);
    assert!(f.s.file_dialog.is_none());
}

#[test]
fn failed_apply_all_preserves_redo_and_strict_downstream_connections() {
    let mut f = Fixture::new();
    let mut p = f.s.editor.project().clone();
    let mut second = qx_geometry::authoring::Section::straight(2, "Strict next section", 10.);
    second.incoming_boundary = qx_geometry::authoring::BoundaryPolicy::RequireContinuousRates;
    p.sections.push(second);
    persistence::save(&f.path, &p, &f.s.catalog).unwrap();
    f.s = Session::new(p.clone(), Some(&f.path), f.s.catalog.clone()).unwrap();
    let mut metadata = p.metadata.clone();
    metadata.name = "Redo must survive invalid Apply All".into();
    assert!(f.s.execute(vec![Command::SetMetadata(metadata)]));
    f.s.undo(false);
    assert!(f.s.editor.can_redo());
    f.stage(true, true);
    f.s.save();
    f.s.resolve_drafts(true);
    assert!(f.s.resolution_error.is_some());
    assert_eq!(f.s.editor.project(), &p);
    assert!(f.s.editor.can_redo());
    assert!(!f.s.editor.can_undo());
    assert!(f.s.has_drafts());
    assert_eq!(persistence::load(&f.path, &f.s.catalog).unwrap(), p);
}

#[test]
fn applying_drafts_before_redo_explicitly_creates_one_new_branch_of_history() {
    let mut f = Fixture::new();
    let original = f.s.editor.project().clone();
    f.s.draft.name = "Previous future".into();
    assert!(f.s.apply_draft());
    f.s.undo(false);
    assert!(f.s.editor.can_redo());
    f.stage(true, true);
    let intended = f.s.preview_project();
    f.s.undo(true);
    f.s.resolve_drafts(true);
    assert_eq!(f.s.editor.project(), &intended);
    assert!(!f.s.editor.can_redo());
    f.s.undo(false);
    assert_eq!(f.s.editor.project(), &original);
    assert!(!f.s.editor.can_undo());
}
