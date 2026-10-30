use super::{
    files::{Dialog, FileAction},
    project_library::Projects,
    recent::Recent,
    startup::{App, Destination, Launch, LibraryTab, Page, Picker, launch},
};
use qx_document::{command::Command, persistence};
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::mpsc,
};

struct Fixture {
    root: PathBuf,
    app: App,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "qx-startup-{}-{}",
            std::process::id(),
            super::session::new_project_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut app = App::welcome().unwrap();
        app.recent = Recent::load_path(Some(root.join("recent.json")));
        app.refresh_recents();
        Self { root, app }
    }
    fn saved(&self, name: &str) -> PathBuf {
        let path = self.root.join(format!("{name}.qx.json"));
        let mut p = super::session::blank_project();
        p.metadata.name = name.into();
        persistence::save(&path, &p, &self.app.catalog).unwrap();
        path
    }
    fn frame(&mut self) {
        let scene = self.app.poll_scene().unwrap_or_else(InspectionScene::empty);
        let ctx = egui::Context::default();
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280., 900.),
                )),
                ..Default::default()
            },
            |ctx| {
                self.app.ui(ctx, &scene, &mut DisplayOptions::default());
            },
        );
        output.textures_delta.clear();
    }
    fn picker(&mut self, path: Option<PathBuf>, relocate: Option<usize>) {
        let (send, result) = mpsc::channel();
        send.send(path).unwrap();
        self.app.picker = Some(Picker { result, relocate });
        self.app.poll_picker();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.app.recent.poll(true);
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn startup_routes_public_developer_explicit_file_and_diagnostics() {
    for exe in ["QUANTUM Dynamics.exe", "quantum-x.exe"] {
        let route = |args: &[&str]| {
            launch(
                Path::new(exe),
                &args.iter().map(OsString::from).collect::<Vec<_>>(),
            )
        };
        assert_eq!(route(&["edit"]), Launch::Editor(None));
        assert_eq!(
            route(&["edit", "Coaster.qx.json"]),
            Launch::Editor(Some("Coaster.qx.json".into()))
        );
        assert_eq!(
            route(&["Coaster.QX.JSON"]),
            Launch::Editor(Some("Coaster.QX.JSON".into()))
        );
        for args in [
            &["--version"][..],
            &["--help"],
            &["--self-check"],
            &["physics-diagnostic"],
            &["demo", "output"],
            &["edit", "a", "b"],
        ] {
            assert_eq!(route(args), Launch::Command);
        }
    }
    assert_eq!(
        launch(Path::new("C:/Portable/QUANTUM Dynamics.exe"), &[]),
        Launch::Welcome
    );
    assert_eq!(launch(Path::new("quantum-x.exe"), &[]), Launch::Command);
}
#[test]
fn welcome_new_cancel_and_creation_are_explicit_and_transactional() {
    let mut f = Fixture::new();
    assert_eq!(f.app.page, Page::Welcome);
    assert!(f.app.session.is_none());
    f.frame();
    assert!(f.app.session.is_none());
    f.app.begin_new();
    assert!(f.app.session.is_none());
    f.app.cancel_setup();
    assert_eq!(f.app.page, Page::Welcome);
    assert!(f.app.session.is_none());
    f.app.begin_new();
    f.app.create();
    assert!(f.app.session.is_none());
    assert!(f.app.status.contains("Enter a project name"));
    f.app.setup.name = "Owner project".into();
    f.app.setup.name = "x".repeat(1025);
    f.app.create();
    assert!(f.app.session.is_none());
    f.app.setup.name = "Owner project".into();
    f.app.create();
    assert_eq!(f.app.page, Page::Editor);
    let s = f.app.session.as_ref().unwrap();
    assert!(s.path.is_empty() && s.dirty());
    assert_eq!(s.editor.project().metadata.name, "Owner project");
    assert_eq!(s.editor.project().sections.len(), 1);
    assert_eq!(s.editor.project().sections[0].extent, 10.);
    assert_eq!(s.editor.project().initial.reference_speed_mps, 20.);
    assert!(s.editor.project().style.is_unassigned());
    assert_eq!(s.editor.project().metadata.author, "");
    assert_eq!(
        f.app.window_title(),
        "QUANTUM Dynamics — Editor — Owner project"
    );
    assert_eq!(std::fs::read_dir(&f.root).unwrap().count(), 0);
    f.frame();
}
#[test]
fn welcome_open_cancel_corrupt_missing_and_success_have_no_unwanted_document() {
    let mut f = Fixture::new();
    f.picker(None, None);
    assert!(f.app.session.is_none());
    assert_eq!(f.app.page, Page::Welcome);
    f.picker(Some(f.root.join("missing.qx.json")), None);
    assert!(f.app.session.is_none());
    let bad = f.root.join("bad.qx.json");
    std::fs::write(&bad, "{}").unwrap();
    f.picker(Some(bad), None);
    assert!(f.app.session.is_none());
    let path = f.saved("Valid");
    f.picker(Some(path.clone()), None);
    f.frame();
    assert_eq!(f.app.page, Page::Editor);
    assert!(!f.app.session.as_ref().unwrap().pending_edits());
    assert_eq!(f.app.recent.entries.len(), 1);
    assert_eq!(f.app.recent.entries[0].path, path.canonicalize().unwrap());
}
#[test]
fn editor_new_cancel_failure_and_welcome_preserve_drafts_until_authorized() {
    let mut f = Fixture::new();
    let path = f.saved("Existing");
    f.app.load_project(path, None);
    f.app.poll_scene();
    let s = f.app.session.as_mut().unwrap();
    s.draft.name = "Owner draft".into();
    let original = s.editor.project().clone();
    let draft = s.draft.clone();
    s.request_startup(Destination::New);
    assert_eq!(s.startup_request, Some(Destination::New));
    s.startup_request = None;
    f.frame();
    assert_eq!(f.app.page, Page::Editor);
    let s = f.app.session.as_mut().unwrap();
    s.request_startup(Destination::New);
    s.resolve_startup(false);
    f.frame();
    assert_eq!(f.app.page, Page::New);
    f.app.setup.name = " ".into();
    f.app.create();
    assert_eq!(f.app.session.as_ref().unwrap().editor.project(), &original);
    f.app.cancel_setup();
    assert_eq!(f.app.page, Page::Editor);
    let s = f.app.session.as_mut().unwrap();
    assert_eq!(s.draft, draft);
    s.request_startup(Destination::Welcome);
    assert_eq!(s.startup_request, Some(Destination::Welcome));
    s.resolve_startup(false);
    f.frame();
    assert_eq!(f.app.page, Page::Welcome);
    assert!(f.app.session.is_none());
    let scene = f.app.poll_scene().unwrap();
    assert!(scene.track.samples.is_empty());
    f.frame();
}
#[test]
fn return_welcome_save_resolves_drafts_and_failed_save_or_cancel_cannot_leave() {
    let mut f = Fixture::new();
    let path = f.saved("Save before leaving");
    f.app.load_project(path.clone(), None);
    let s = f.app.session.as_mut().unwrap();
    s.metadata_draft.author = "Owner draft".into();
    s.request_startup(Destination::Welcome);
    s.resolve_startup(true);
    assert!(s.draft_request.is_some());
    assert!(s.startup_approved.is_none());
    s.resolve_drafts(true);
    assert_eq!(s.startup_approved, Some(Destination::Welcome));
    assert_eq!(
        persistence::load(&path, &s.catalog)
            .unwrap()
            .metadata
            .author,
        "Owner draft"
    );
    f.frame();
    assert!(f.app.session.is_none());
    f.app.load_project(path, None);
    let s = f.app.session.as_mut().unwrap();
    let mut m = s.editor.project().metadata.clone();
    m.author = "Unsaved".into();
    assert!(s.execute(vec![Command::SetMetadata(m)]));
    s.path = f.root.to_string_lossy().into_owned(); // Destination is a directory: failure is recoverable.
    s.request_startup(Destination::Welcome);
    s.resolve_startup(true);
    assert!(s.save_failure.is_some());
    assert!(s.startup_approved.is_none());
    s.path.clear();
    let (send, result) = mpsc::channel();
    send.send(None).unwrap();
    s.file_dialog = Some(Dialog {
        action: FileAction::SaveAs,
        result,
    });
    s.poll_file_dialog(&egui::Context::default());
    assert!(!s.startup_after_save);
    assert!(s.startup_approved.is_none());
    assert!(s.pending_edits());
}
#[test]
fn recent_recovery_cancel_failure_relocation_remove_and_restart() {
    let mut f = Fixture::new();
    let path = f.saved("Moved");
    f.app.recent.remember(&path, "Moved");
    f.app.recent.poll(true);
    let recorded = f.app.recent.entries[0].path.clone();
    let moved = f.root.join("relocated.qx.json");
    std::fs::rename(&path, &moved).unwrap();
    f.app.refresh_recents();
    assert!(f.app.missing[0]);
    f.app.load_project(path.clone(), None);
    assert!(f.app.session.is_none());
    f.picker(None, Some(0));
    assert_eq!(f.app.recent.entries[0].path, recorded);
    f.picker(Some(f.root.join("bad.qx.json")), Some(0));
    assert_eq!(f.app.recent.entries.len(), 1);
    f.picker(Some(moved.clone()), Some(0));
    assert_eq!(f.app.recent.entries.len(), 1);
    f.app.recent.poll(true);
    let mut reopened = Recent::load_path(Some(f.root.join("recent.json")));
    assert_eq!(reopened.entries[0].path, moved.canonicalize().unwrap());
    reopened.remove(0);
    reopened.poll(true);
    assert!(
        Recent::load_path(Some(f.root.join("recent.json")))
            .entries
            .is_empty()
    );
}
#[test]
fn corrupt_recents_are_preserved_and_history_is_bounded_deduplicated() {
    let mut f = Fixture::new();
    let corrupt = f.root.join("corrupt.json");
    std::fs::write(&corrupt, b"{bad original}").unwrap();
    let mut store = Recent::load_path(Some(corrupt.clone()));
    assert!(store.error.is_some());
    store.remember(&f.root.join("test.qx.json"), "Test");
    store.poll(true);
    assert_eq!(std::fs::read(corrupt).unwrap(), b"{bad original}");
    for i in 0..30 {
        f.app
            .recent
            .remember(&f.root.join(format!("{i}.qx.json")), "Project");
    }
    assert_eq!(f.app.recent.entries.len(), 24);
    let last = f.app.recent.entries[0].path.clone();
    f.app.recent.remember(&last, "Again");
    assert_eq!(f.app.recent.entries.len(), 24);
    f.app.recent.poll(true);
}
#[test]
fn library_empty_states_samples_and_template_copy_do_not_invent_content() {
    let mut f = Fixture::new();
    let empty = Projects::discover(&f.root);
    assert!(empty.samples.is_empty() && empty.templates.is_empty());
    std::fs::create_dir_all(f.root.join("projects/samples")).unwrap();
    std::fs::create_dir_all(f.root.join("projects/templates")).unwrap();
    let source = f.saved("Installed template");
    let template = f.root.join("projects/templates/installed.qx.json");
    std::fs::copy(&source, &template).unwrap();
    std::fs::copy(&source, f.root.join("projects/samples/existing.qx.json")).unwrap();
    let library = Projects::discover(&f.root);
    assert_eq!(library.samples.len(), 1);
    assert_eq!(library.templates.len(), 1);
    let original = persistence::load(&template, &f.app.catalog).unwrap();
    let bytes = std::fs::read(&template).unwrap();
    f.app.use_template(template.clone());
    assert_eq!(f.app.page, Page::New);
    assert!(f.app.session.is_none());
    f.app.setup.name = "Owner copy".into();
    f.app.create();
    let s = f.app.session.as_ref().unwrap();
    assert!(s.path.is_empty());
    assert_ne!(
        s.editor.project().metadata.project_id,
        original.metadata.project_id
    );
    assert_eq!(s.editor.project().sections, original.sections);
    assert_eq!(std::fs::read(&template).unwrap(), bytes);
}

fn frame(
    app: &mut App,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    frame_at(app, ctx, events, egui::vec2(1280., 900.))
}
fn frame_at(
    app: &mut App,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    physical_size: egui::Vec2,
) -> Vec<(String, egui::Rect)> {
    fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => result.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => {
                for s in v {
                    labels(s, result);
                }
            }
            _ => {}
        }
    }
    let scene = app.poll_scene().unwrap_or_else(InspectionScene::empty);
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                physical_size / ctx.pixels_per_point(),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            app.ui(ctx, &scene, &mut DisplayOptions::default());
        },
    );
    let mut result = vec![];
    for s in &output.shapes {
        labels(&s.shape, &mut result);
    }
    output.textures_delta.clear();
    result
}
fn click(app: &mut App, ctx: &egui::Context, label: &str) {
    let mut labels = vec![];
    for _ in 0..3 {
        labels = frame(app, ctx, vec![]);
    }
    let pos = labels
        .iter()
        .find(|(s, _)| s == label)
        .unwrap_or_else(|| panic!("missing {label}: {labels:?}"))
        .1
        .center();
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}
#[test]
fn actual_welcome_buttons_setup_cancel_library_tabs_and_empty_states_work() {
    let mut f = Fixture::new();
    f.app.projects = Projects::default();
    let ctx = egui::Context::default();
    click(&mut f.app, &ctx, "New Project…");
    assert_eq!(f.app.page, Page::New);
    let labels = frame(&mut f.app, &ctx, vec![]);
    for forbidden in [
        "Coaster family",
        "Family / numbered style",
        "Description",
        "Author (optional)",
        "Design reference speed (m/s)",
    ] {
        assert!(!labels.iter().any(|(t, _)| t == forbidden));
    }
    assert!(f.app.session.is_none());
    click(&mut f.app, &ctx, "Cancel");
    assert_eq!(f.app.page, Page::Welcome);
    click(&mut f.app, &ctx, "LIBRARY");
    assert_eq!(f.app.page, Page::Library);
    click(&mut f.app, &ctx, "Templates");
    assert_eq!(f.app.library_tab, LibraryTab::Templates);
    assert!(
        frame(&mut f.app, &ctx, vec![])
            .iter()
            .any(|(t, _)| t == "No templates installed.")
    );
    click(&mut f.app, &ctx, "Sample Projects");
    assert!(
        frame(&mut f.app, &ctx, vec![])
            .iter()
            .any(|(t, _)| t == "No sample projects installed.")
    );
    click(&mut f.app, &ctx, "Welcome");
    assert_eq!(f.app.page, Page::Welcome);
    assert!(f.app.session.is_none());
    click(&mut f.app, &ctx, "New Project…");
    f.app.setup.name = "UI created".into();
    click(&mut f.app, &ctx, "Create Project");
    assert_eq!(f.app.page, Page::Editor);
}

#[test]
fn owner_navigation_is_functional_and_setup_never_creates_a_document() {
    let mut f = Fixture::new();
    let ctx = egui::Context::default();
    let original = f.app.window_preferences();
    click(&mut f.app, &ctx, "EDITOR");
    assert_eq!(f.app.page, Page::Projects);
    assert!(f.app.session.is_none());
    let labels = frame(&mut f.app, &ctx, vec![]);
    assert!(labels.iter().any(|(t, _)| t == "New Project…"));
    assert!(labels.iter().any(|(t, _)| t == "Open Project…"));
    for (label, page) in [
        ("INFO", Page::Info),
        ("NEWS", Page::News),
        ("SETUP", Page::Configuration),
    ] {
        click(&mut f.app, &ctx, "Welcome");
        click(&mut f.app, &ctx, label);
        assert_eq!(f.app.page, page);
        assert!(f.app.session.is_none());
    }
    f.app.configuration_draft.ui_scale = 1.5;
    click(&mut f.app, &ctx, "Cancel");
    assert_eq!(f.app.window_preferences(), original);
    click(&mut f.app, &ctx, "SETUP");
    f.app.configuration_draft.ui_scale = 1.25;
    click(&mut f.app, &ctx, "Apply configuration");
    assert_eq!(f.app.window_preferences().ui_scale, 1.25);
    assert!(f.app.session.is_none());
    click(&mut f.app, &ctx, "Welcome");
    click(&mut f.app, &ctx, "EDITOR");
    click(&mut f.app, &ctx, "New Project…");
    f.app.setup.name = "Named by owner".into();
    click(&mut f.app, &ctx, "Create Project");
    assert_eq!(f.app.page, Page::Editor);
    assert!(
        f.app
            .session
            .as_ref()
            .unwrap()
            .editor
            .project()
            .style
            .is_unassigned()
    );
    assert_eq!(f.app.window_preferences().ui_scale, 1.25);
}

#[test]
fn welcome_destinations_remain_visible_at_small_windows_and_large_ui_scale() {
    let mut f = Fixture::new();
    for (size, scale) in [
        (egui::vec2(960., 640.), 1.),
        (egui::vec2(960., 640.), 2.),
        (egui::vec2(1440., 960.), 1.5),
    ] {
        let ctx = egui::Context::default();
        let mut preferences = f.app.window_preferences();
        preferences.ui_scale = scale;
        f.app.window_changed(preferences);
        let mut text = vec![];
        for _ in 0..6 {
            text = frame_at(&mut f.app, &ctx, vec![], size);
        }
        for label in ["LIBRARY", "EDITOR", "INFO", "NEWS", "SETUP"] {
            let rect = text.iter().find(|(s, _)| s == label).unwrap().1;
            assert!(rect.min.x >= 24. && rect.max.x <= size.x / scale - 24.);
            assert!(
                rect.min.y >= 24. && rect.max.y <= size.y / scale - 24.,
                "{label} at {size:?} scale {scale}: {rect:?}"
            );
        }
        assert!(f.app.session.is_none());
    }
}

#[test]
fn editor_properties_apply_cancel_save_history_and_pending_geometry_are_integrated() {
    let mut f = Fixture::new();
    f.app.begin_new();
    f.app.setup.name = "Unassigned coaster".into();
    f.app.create();
    let original = f.app.session.as_ref().unwrap().editor.project().clone();
    let ctx = egui::Context::default();
    // Command is available from Train as well as Track, even after floating Track.
    let s = f.app.session.as_mut().unwrap();
    s.workbench.layouts[0].float(super::workbench::Pane::Track);
    s.workbench.active = super::workbench::Profile::Train;
    click(&mut f.app, &ctx, "Edit");
    click(&mut f.app, &ctx, "Coaster Properties");
    let s = f.app.session.as_mut().unwrap();
    assert_eq!(s.workbench.active, super::workbench::Profile::Track);
    assert!(s.workbench.layouts[0].floating.is_empty());
    assert_eq!(s.track_tab, 1);
    s.metadata_draft.name = "Owner coaster".into();
    s.metadata_draft.description = "Owner description".into();
    s.style_draft = qx_style::StyleSelection {
        family_id: "monorail".into(),
        style_id: "monorail-200".into(),
    };
    s.draft.name = "Unapplied geometry".into();
    let geometry = s.draft.clone();
    click(&mut f.app, &ctx, "Apply");
    let s = f.app.session.as_mut().unwrap();
    let applied = s.editor.project().clone();
    assert_eq!(applied.metadata.description, "Owner description");
    assert_eq!(applied.style.style_id, "monorail-200");
    assert_eq!(applied.sections, original.sections);
    assert_eq!(applied.construction, original.construction);
    assert_eq!(applied.trains, original.trains);
    assert_eq!(s.draft, geometry);
    s.metadata_draft.description = "Canceled description".into();
    s.style_draft = Default::default();
    click(&mut f.app, &ctx, "Cancel");
    let s = f.app.session.as_mut().unwrap();
    assert_eq!(s.editor.project(), &applied);
    assert_eq!(s.metadata_draft, applied.metadata);
    assert_eq!(s.style_draft, applied.style);
    assert_eq!(s.draft, geometry);
    s.cancel_section();
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    assert_eq!(s.style_draft, original.style);
    s.undo(true);
    assert_eq!(s.editor.project(), &applied);
    let path = f.root.join("properties.qx.json");
    s.save_to(&path);
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), applied);
    f.app.load_project(path, None);
    assert_eq!(f.app.session.as_ref().unwrap().editor.project(), &applied);
}

#[test]
fn deferred_style_roundtrip_and_invalid_properties_preserve_history_and_drafts() {
    let mut f = Fixture::new();
    f.app.begin_new();
    f.app.setup.name = "No style needed".into();
    f.app.create();
    let path = f.root.join("unassigned.qx.json");
    let s = f.app.session.as_mut().unwrap();
    s.save_to(&path);
    let original = s.editor.project().clone();
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), original);
    s.metadata_draft.description = "Pending properties".into();
    s.style_draft = qx_style::StyleSelection {
        family_id: "hybrid".into(),
        style_id: "hybrid-100".into(),
    };
    s.save_to(&path);
    assert!(s.draft_request.is_some());
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), original);
    s.resolve_drafts(true);
    let assigned = s.editor.project().clone();
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), assigned);
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    s.style_draft.family_id = "invalid".into();
    assert!(!super::coaster_properties::apply(s));
    assert_eq!(s.editor.project(), &original);
    assert!(s.editor.can_redo());
    assert_eq!(s.style_draft.family_id, "invalid");
    super::coaster_properties::cancel(s);
    s.undo(true);
    assert_eq!(s.editor.project(), &assigned);
    // A style-only draft participates in the leave/close guards and Apply All.
    s.style_draft = Default::default();
    assert!(s.has_drafts() && s.pending_edits());
    assert!(s.draft_names().iter().any(|n| n.contains("numbered style")));
    s.request_startup(Destination::Welcome);
    s.resolve_startup(true);
    s.resolve_drafts(true);
    assert_eq!(s.startup_approved, Some(Destination::Welcome));
    assert!(
        persistence::load(&path, &s.catalog)
            .unwrap()
            .style
            .is_unassigned()
    );
}

#[test]
fn actual_file_return_to_welcome_resolves_save_drafts_and_cancel_without_losing_work() {
    let mut f = Fixture::new();
    let path = f.saved("Return through File");
    f.app.load_project(path.clone(), None);
    f.app.session.as_mut().unwrap().draft.name = "Owner's latest section".into();
    let ctx = egui::Context::default();
    click(&mut f.app, &ctx, "File");
    click(&mut f.app, &ctx, "Return to Welcome Screen…");
    click(&mut f.app, &ctx, "Keep current project");
    assert_eq!(f.app.page, Page::Editor);
    assert!(f.app.session.as_ref().unwrap().has_drafts());
    click(&mut f.app, &ctx, "File");
    click(&mut f.app, &ctx, "Return to Welcome Screen…");
    click(&mut f.app, &ctx, "Save & continue");
    click(&mut f.app, &ctx, "Keep editing / Cancel");
    let s = f.app.session.as_ref().unwrap();
    assert!(s.has_drafts());
    assert!(s.startup_request.is_none() && !s.startup_after_save);
    click(&mut f.app, &ctx, "File");
    click(&mut f.app, &ctx, "Return to Welcome Screen…");
    click(&mut f.app, &ctx, "Save & continue");
    click(&mut f.app, &ctx, "Apply All & Save");
    assert_eq!(f.app.page, Page::Welcome);
    assert!(f.app.session.is_none());
    assert_eq!(
        persistence::load(&path, &f.app.catalog).unwrap().sections[0].name,
        "Owner's latest section"
    );
}

#[test]
fn window_close_dialog_disconnect_and_setup_replacement_are_recoverable() {
    let mut f = Fixture::new();
    assert!(f.app.request_close());
    let (send, result) = mpsc::channel();
    f.app.picker = Some(Picker {
        result,
        relocate: None,
    });
    assert!(!f.app.request_close());
    drop(send);
    f.app.poll_picker();
    assert!(f.app.picker.is_none() && f.app.session.is_none());
    let path = f.saved("Existing");
    f.app.load_project(path, None);
    let s = f.app.session.as_mut().unwrap();
    s.metadata_draft.author = "Unapplied owner".into();
    let old = s.editor.project().metadata.project_id.clone();
    s.request_startup(Destination::New);
    s.resolve_startup(false);
    f.frame();
    assert_eq!(f.app.page, Page::New);
    assert!(!f.app.request_close());
    assert_eq!(f.app.page, Page::Editor);
    let s = f.app.session.as_mut().unwrap();
    assert!(s.close_requested && s.has_drafts());
    s.close_requested = false;
    s.request_startup(Destination::New);
    s.resolve_startup(false);
    f.frame();
    f.app.setup.name = "Intentional replacement".into();
    f.app.create();
    let s = f.app.session.as_ref().unwrap();
    assert_ne!(s.editor.project().metadata.project_id, old);
    assert_eq!(s.editor.project().metadata.name, "Intentional replacement");
    assert!(s.path.is_empty() && s.dirty());
}

#[test]
fn template_copy_resolves_and_packages_its_relative_train_assets() {
    let mut f = Fixture::new();
    let mut p = super::session::blank_project();
    let mut train = qx_document::train::Train::development_fixture("train".into());
    train.components[0].asset = Some(super::train_assets::identity("assets/body.glb".into()));
    p.trains.push(train);
    std::fs::create_dir_all(f.root.join("assets")).unwrap();
    let body = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/development/train/development-body.glb");
    std::fs::copy(&body, f.root.join("assets/body.glb")).unwrap();
    let template = f.root.join("with-train.qx.json");
    persistence::save(&template, &p, &f.app.catalog).unwrap();
    f.app.use_template(template);
    f.app.create();
    let s = f.app.session.as_mut().unwrap();
    let start = std::time::Instant::now();
    while !s.assets.library.contains_key("assets/body.glb") {
        s.assets.poll(s.editor.project(), &s.path);
        assert!(s.assets.errors.is_empty(), "{:?}", s.assets.errors);
        assert!(start.elapsed().as_secs() < 10, "asset load timed out");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let destination = f.root.join("copy");
    std::fs::create_dir(&destination).unwrap();
    s.save_to(&destination.join("new.qx.json"));
    assert!(s.save_failure.is_none(), "{}", s.status);
    assert_eq!(
        std::fs::read(destination.join("assets/body.glb")).unwrap(),
        std::fs::read(body).unwrap()
    );
}
