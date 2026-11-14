//! M4K Simulator Mode: real egui passes over the live Session and the one
//! production playback worker. Synthetic straight-track fixtures only.
use super::{
    session::{Session, blank_project, scene},
    simulator::{self, NAVIGATION_BANK},
    train_playback::Snapshot,
    workbench::Profile,
};
use qx_document::{
    command::Command,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionId},
    math::{Quaternion, Vec3},
};
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(20);

fn project() -> qx_document::Project {
    let mut p = blank_project();
    p.sections[0] = Section::straight(1, "Synthetic simulator fixture", 100.);
    p.sections
        .push(Section::straight(2, "Second synthetic region", 40.));
    p.initial.orientation = Quaternion::axis_angle(Vec3::Z, -0.2).unwrap();
    p.trains.push(
        Train::development_consist("vehicles".into(), 3, DevelopmentArrangement::RotatingSeats)
            .unwrap(),
    );
    p.heartline.train_configuration_id = Some("vehicles".into());
    p
}

fn session(p: qx_document::Project) -> Session {
    let mut s = Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap();
    settle(&mut s);
    s
}

/// Waits until committed geometry is visible, as the native loop would.
fn settle(s: &mut Session) {
    let until = Instant::now() + TIMEOUT;
    while s.visible_revision != s.revision || s.visible_preview {
        let _ = s.poll();
        assert!(Instant::now() < until, "generation timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

struct Frame {
    output: SurfaceOutput,
    display: DisplayOptions,
    labels: Vec<(String, egui::Rect)>,
}

fn frame(
    s: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
    events: Vec<egui::Event>,
) -> Frame {
    let mut result = None;
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440., 960.),
            )),
            events,
            ..Default::default()
        },
        |ctx| result = Some(s.ui(ctx, scene, display)),
    );
    fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => result.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| labels(s, result)),
            _ => {}
        }
    }
    output.textures_delta.clear();
    let mut found = vec![];
    for c in &output.shapes {
        labels(&c.shape, &mut found);
    }
    Frame {
        output: result.unwrap(),
        display: display.clone(),
        labels: found,
    }
}

fn click(
    s: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
    label: &str,
) {
    // egui lays out new areas in a first, invisible sizing pass.
    frame(s, ctx, scene, display, vec![]);
    let f = frame(s, ctx, scene, display, vec![]);
    let p = f
        .labels
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| {
            panic!(
                "missing label {label}; have {:?}",
                f.labels.iter().map(|l| &l.0).collect::<Vec<_>>()
            )
        })
        .1
        .center();
    for pressed in [true, false] {
        frame(
            s,
            ctx,
            scene,
            display,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

/// Enters through the real provisional menu command.
fn enter(
    s: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) {
    click(s, ctx, scene, display, "Window / Layout");
    click(s, ctx, scene, display, "Simulator Mode (development)…");
}

/// Runs frames until `done`, failing on timeout.
fn until(
    s: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
    what: &str,
    done: impl Fn(&Session) -> bool,
) -> Frame {
    let end = Instant::now() + TIMEOUT;
    loop {
        let f = frame(s, ctx, scene, display, vec![]);
        if done(s) {
            return f;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn snapshot(s: &Session) -> Option<&Snapshot> {
    s.playback.snapshot.as_ref()
}

/// Everything the Editor must get back unchanged after a Simulator visit.
#[derive(Debug, PartialEq)]
struct EditorState {
    project: qx_document::Project,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
    path: String,
    selected: SectionId,
    draft: Section,
    profile: Profile,
    /// Structural: egui_tiles stores tiles in a hash map, so the serialized
    /// key order of an unchanged layout is not stable. Every tile, ID, share,
    /// active tab and child order is still compared exactly.
    layouts: serde_json::Value,
    ride_mode: String,
    train_station_m: Option<f64>,
    viewport_inspection: Option<(SectionId, f64)>,
    pending_edits: bool,
}
fn editor_state(s: &Session) -> EditorState {
    EditorState {
        project: s.editor.project().clone(),
        dirty: s.dirty(),
        can_undo: s.editor.can_undo(),
        can_redo: s.editor.can_redo(),
        path: s.path.clone(),
        selected: s.selected,
        draft: s.draft.clone(),
        profile: s.workbench.active,
        layouts: serde_json::to_value(&s.workbench.layouts).unwrap(),
        ride_mode: format!("{:?}", s.ride.mode),
        train_station_m: s.train_station_m,
        viewport_inspection: s.viewport_inspection,
        pending_edits: s.pending_edits(),
    }
}
/// Names each differing field instead of printing two whole projects.
fn assert_same(now: &EditorState, then: &EditorState) {
    let mut differs = vec![];
    macro_rules! check {
        ($($f:ident),*) => {$(if now.$f != then.$f { differs.push(stringify!($f)); })*};
    }
    check!(
        project,
        dirty,
        can_undo,
        can_redo,
        path,
        selected,
        draft,
        profile,
        layouts,
        ride_mode,
        train_station_m,
        viewport_inspection,
        pending_edits
    );
    assert!(differs.is_empty(), "editor state changed: {differs:?}");
}

/// The intermittent M4K `["layouts"]` failure was a hash-map key-order
/// difference between structurally equal layouts. The structural comparison
/// ignores only object key order; values and child order still count.
#[test]
fn layout_comparison_ignores_only_hash_map_key_order() {
    let v = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
    let base = r#"{"tiles":{"1":{"Pane":"Track"},"2":{"Tabs":{"children":[1,3],"active":1}}}}"#;
    let reordered =
        r#"{"tiles":{"2":{"Tabs":{"active":1,"children":[1,3]}},"1":{"Pane":"Track"}}}"#;
    assert_ne!(base, reordered);
    assert_eq!(v(base), v(reordered));
    for changed in [
        r#"{"tiles":{"1":{"Pane":"View"},"2":{"Tabs":{"children":[1,3],"active":1}}}}"#,
        r#"{"tiles":{"1":{"Pane":"Track"},"2":{"Tabs":{"children":[3,1],"active":1}}}}"#,
        r#"{"tiles":{"1":{"Pane":"Track"},"2":{"Tabs":{"children":[1,3],"active":3}}}}"#,
    ] {
        assert_ne!(v(base), v(changed), "{changed}");
    }
}

#[test]
fn editor_simulator_editor_round_trip_preserves_the_open_document() {
    let mut s = session(project());
    // An unsaved committed edit and a non-default editor context must survive.
    let mut metadata = s.editor.project().metadata.clone();
    metadata.description = "Unsaved edit preserved across Simulator Mode".into();
    assert!(s.execute(vec![Command::SetMetadata(metadata)]));
    s.select(SectionId(2));
    s.workbench.active = Profile::Train;
    s.train_station_m = Some(50.);
    s.viewport_inspection = Some((SectionId(2), 110.));
    settle(&mut s);
    let ctx = egui::Context::default();
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut display = DisplayOptions::default();
    let before = editor_state(&s);
    assert!(before.dirty && before.can_undo && !before.can_redo);

    // Enter through the real command-bar button; the worker seats the train.
    enter(&mut s, &ctx, &sc, &mut display);
    assert!(s.simulator.preparing() || s.simulator.active());
    let entered = until(&mut s, &ctx, &sc, &mut display, "entry", |s| {
        s.simulator.active()
    });
    let f = frame(&mut s, &ctx, &sc, &mut display, vec![]);
    assert_eq!(f.output.profile, NAVIGATION_BANK);
    assert!(
        f.labels.iter().any(|l| l.0 == "SIMULATOR MODE"),
        "mode indicator missing"
    );
    let initial = snapshot(&s).unwrap().clone();
    assert_eq!(initial.ticks, 0);
    assert!(!initial.running);
    // Simulated train bodies are presented; no editor selection/marker.
    assert_eq!(
        f.display.asset_instances.len(),
        initial.poses.components.len()
    );
    assert!(f.display.selected.is_none() && !f.display.inspection_marker);
    drop(entered);

    // Viewport picks and hover inspection cannot touch editor state.
    EditorSurface::select_section(&mut s, SectionId(1));
    EditorSurface::inspect_station(&mut s, Some((SectionId(1), 3.)));
    assert_eq!(s.selected, SectionId(2));
    assert_eq!(s.viewport_inspection, Some((SectionId(2), 110.)));

    // Play (real button), observe fixed steps, Pause, Reset to the exact start.
    click(&mut s, &ctx, &sc, &mut display, "Play");
    let f = until(&mut s, &ctx, &sc, &mut display, "running ticks", |s| {
        snapshot(s).is_some_and(|p| p.running && p.ticks >= 24)
    });
    assert!(f.labels.iter().any(|l| l.0 == "Pause"));
    assert!(f.labels.iter().any(|l| l.0.starts_with("Running · tick")));
    let moving = snapshot(&s).unwrap();
    assert!(moving.telemetry.elapsed_s > 0.);
    assert!(
        (moving.telemetry.elapsed_s - moving.ticks as f64 * qx_physics::FIXED_STEP_SECONDS).abs()
            < 1e-9,
        "telemetry time must be the fixed-step tick count"
    );
    assert_ne!(
        moving.poses, initial.poses,
        "gravity on a grade moves the train"
    );
    click(&mut s, &ctx, &sc, &mut display, "Pause");
    until(&mut s, &ctx, &sc, &mut display, "pause", |s| {
        snapshot(s).is_some_and(|p| !p.running)
    });
    click(&mut s, &ctx, &sc, &mut display, "Reset");
    until(&mut s, &ctx, &sc, &mut display, "reset", |s| {
        snapshot(s).is_some_and(|p| p.ticks == 0 && !p.running)
    });
    assert_eq!(snapshot(&s).unwrap().poses, initial.poses);
    assert_eq!(
        s.editor.project(),
        &before.project,
        "simulation never edits"
    );

    // Return through the real button; the Editor comes back exactly.
    click(&mut s, &ctx, &sc, &mut display, "Return to Editor");
    assert!(!s.simulator.active() && !s.simulator.preparing());
    assert!(!s.playback.active(), "simulation retired on return");
    let f = frame(&mut s, &ctx, &sc, &mut display, vec![]);
    assert_eq!(f.output.profile, Profile::Train.index());
    assert!(f.labels.iter().all(|l| l.0 != "SIMULATOR MODE"));
    assert!(s.simulator.error.is_none());
    assert_same(&editor_state(&s), &before);
    // History still works on the preserved stack.
    assert!(s.editor.undo());
    assert!(s.editor.redo());
    assert_eq!(s.editor.project(), &before.project);

    // Edit the track, regenerate, and re-enter from the new committed revision.
    s.draft = Section::straight(2, "Second synthetic region", 60.);
    s.apply_draft();
    assert_eq!(s.editor.project().sections[1].extent, 60., "{}", s.status);
    assert!(!s.has_drafts());
    settle(&mut s);
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let edited = editor_state(&s);
    enter(&mut s, &ctx, &sc, &mut display);
    until(&mut s, &ctx, &sc, &mut display, "re-entry", |s| {
        s.simulator.active()
    });
    assert_eq!(snapshot(&s).unwrap().ticks, 0);
    simulator::exit(&mut s);
    assert_same(&editor_state(&s), &edited);
}

#[test]
fn entry_fails_closed_with_actionable_errors() {
    // No associated train.
    let mut p = project();
    p.heartline.train_configuration_id = None;
    let mut s = session(p);
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let before = editor_state(&s);
    assert!(!simulator::request_entry(&mut s, &sc));
    assert!(s.simulator.error.as_deref().unwrap().contains("No train"));
    assert!(!s.simulator.active() && !s.playback.active());
    assert_same(&editor_state(&s), &before);

    // A pending draft must be applied or canceled first.
    let mut s = session(project());
    s.draft.name = "Unapplied region rename".into();
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    assert!(simulator::readiness(&s).unwrap_err().contains("drafts"));
    assert!(!simulator::request_entry(&mut s, &sc));
    assert_eq!(s.draft.name, "Unapplied region rename", "draft untouched");

    // Generation still running for the committed document.
    let mut s = session(project());
    s.revision += 1;
    assert!(simulator::readiness(&s).unwrap_err().contains("generation"));

    // A nominal rider-reference configuration is not a simulatable train.
    let mut p = project();
    p.heartline.train_configuration_id = Some("nominal-reference-point-only".into());
    let s = session(p);
    assert!(
        simulator::readiness(&s)
            .unwrap_err()
            .contains("not a saved train")
    );
}

#[test]
fn failed_seating_returns_to_the_editor_without_a_partial_simulator() {
    let mut s = session(project());
    // The consist cannot be seated with its body behind the open track start.
    s.train_station_m = Some(0.);
    let ctx = egui::Context::default();
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut display = DisplayOptions::default();
    let before = editor_state(&s);
    assert!(simulator::request_entry(&mut s, &sc));
    let f = until(&mut s, &ctx, &sc, &mut display, "failure", |s| {
        !s.simulator.preparing()
    });
    assert!(!s.simulator.active());
    assert!(!s.playback.active() && s.playback.snapshot.is_none());
    let error = s.simulator.error.clone().expect("failure is reported");
    assert!(error.starts_with("Simulator could not start"), "{error}");
    assert_ne!(f.output.profile, NAVIGATION_BANK);
    frame(&mut s, &ctx, &sc, &mut display, vec![]);
    let f = frame(&mut s, &ctx, &sc, &mut display, vec![]);
    assert!(f.labels.iter().any(|l| l.0 == "Simulator unavailable"));
    assert_same(&editor_state(&s), &before);
}

#[test]
fn document_change_during_preparation_cancels_entry() {
    let mut s = session(project());
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    assert!(simulator::request_entry(&mut s, &sc));
    assert!(s.simulator.preparing());
    let mut metadata = s.editor.project().metadata.clone();
    metadata.name = "Changed while preparing".into();
    assert!(s.execute(vec![Command::SetMetadata(metadata)]));
    simulator::poll(&mut s);
    assert!(!s.simulator.preparing() && !s.simulator.active());
    assert!(!s.playback.active());
    assert!(s.simulator.error.as_deref().unwrap().contains("changed"));
}

#[test]
fn closing_from_the_simulator_returns_to_editor_guards() {
    let mut s = session(project());
    let mut metadata = s.editor.project().metadata.clone();
    metadata.description = "unsaved".into();
    assert!(s.execute(vec![Command::SetMetadata(metadata)]));
    settle(&mut s);
    let ctx = egui::Context::default();
    let sc = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut display = DisplayOptions::default();
    assert!(simulator::request_entry(&mut s, &sc));
    until(&mut s, &ctx, &sc, &mut display, "entry", |s| {
        s.simulator.active()
    });
    // Unsaved edits: the close prompt is an Editor prompt, never skipped.
    assert!(!EditorSurface::request_close(&mut s));
    assert!(!s.simulator.active() && !s.playback.active());
    assert!(s.close_requested);
    frame(&mut s, &ctx, &sc, &mut display, vec![]);
    let f = frame(&mut s, &ctx, &sc, &mut display, vec![]);
    assert!(f.labels.iter().any(|l| l.0 == "Close with unsaved edits?"));
}
