use super::{
    session::{Session, blank_project, scene},
    train_playback::{Action, Snapshot},
    workbench::Profile,
};
use qx_document::{
    command::Command,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::Section,
    math::{Quaternion, Vec3},
};
use qx_rendering::EditorSurface;
use std::time::{Duration, Instant};

fn session() -> Session {
    let mut p = blank_project();
    p.sections[0] = Section::straight(1, "Synthetic playback test", 100.);
    p.initial.orientation = Quaternion::axis_angle(Vec3::Z, -0.2).unwrap();
    p.trains.push(
        Train::development_consist("vehicles".into(), 3, DevelopmentArrangement::RotatingSeats)
            .unwrap(),
    );
    p.heartline.train_configuration_id = Some("vehicles".into());
    let mut s = Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap();
    s.workbench.active = Profile::Train;
    s
}
fn wait(s: &mut Session, predicate: impl Fn(&Snapshot) -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        s.playback.sync(s.editor.project(), s.options);
        assert!(s.playback.error.is_none(), "{:?}", s.playback.error);
        if s.playback.snapshot.as_ref().is_some_and(&predicate) {
            return;
        }
        assert!(Instant::now() < until, "playback worker timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn frame(
    s: &mut Session,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect)>, qx_rendering::DisplayOptions) {
    let scene = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut display = Default::default();
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
            s.ui(ctx, &scene, &mut display);
        },
    );
    fn labels(shape: &egui::Shape, result: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => result.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => {
                for s in v {
                    labels(s, result)
                }
            }
            _ => {}
        }
    }
    output.textures_delta.clear();
    let mut result = vec![];
    for s in output.shapes {
        labels(&s.shape, &mut result);
    }
    (result, display)
}
fn click(s: &mut Session, ctx: &egui::Context, label: &str) {
    let labels = frame(s, ctx, vec![]).0;
    let p = labels
        .iter()
        .find(|(s, _)| s == label)
        .unwrap_or_else(|| panic!("missing label {label}"))
        .1
        .center();
    for pressed in [true, false] {
        frame(
            s,
            ctx,
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
#[test]
fn actual_playback_buttons_drive_worker_and_render_the_same_physical_snapshots() {
    let mut s = session();
    let ctx = egui::Context::default();
    let original = s.editor.project().clone();
    for _ in 0..4 {
        frame(&mut s, &ctx, vec![]);
    }
    click(&mut s, &ctx, "Initialize playback");
    wait(&mut s, |p| p.ticks == 0);
    let first = s.playback.snapshot.as_ref().unwrap().poses.clone();
    click(&mut s, &ctx, "Single step");
    wait(&mut s, |p| p.ticks == 1);
    click(&mut s, &ctx, "Play");
    wait(&mut s, |p| p.ticks >= 5);
    click(&mut s, &ctx, "Pause");
    wait(&mut s, |p| !p.running);
    let paused = s.playback.snapshot.as_ref().unwrap().ticks;
    std::thread::sleep(Duration::from_millis(20));
    let display = frame(&mut s, &ctx, vec![]).1;
    assert_eq!(s.playback.snapshot.as_ref().unwrap().ticks, paused);
    let p = s.playback.snapshot.as_ref().unwrap();
    assert_ne!(p.poses, first);
    assert_eq!(display.asset_instances.len(), 12);
    for (instance, pose) in display.asset_instances.iter().zip(&p.poses.components) {
        let t = instance.transform[3];
        assert!((Vec3::new(t[0], t[1], t[2]) - pose.pose.position_m).length() < 1e-9);
    }
    click(&mut s, &ctx, "Reset simulation");
    wait(&mut s, |p| p.ticks == 0 && !p.running);
    assert_eq!(s.playback.snapshot.as_ref().unwrap().poses, first);
    click(&mut s, &ctx, "Return to inspection");
    assert!(!s.playback.active());
    assert_eq!(s.editor.project(), &original);
}
#[test]
fn visual_assignment_preserves_simulation_but_mechanical_edits_and_undo_invalidate_it() {
    let mut s = session();
    let track = scene(s.editor.project(), &s.catalog, s.options)
        .unwrap()
        .track;
    s.playback
        .initialize(s.editor.project(), s.options, &track, 50.);
    wait(&mut s, |p| p.ticks == 0);
    s.playback.action(Action::Step);
    wait(&mut s, |p| p.ticks == 1);
    let before = s.playback.snapshot.as_ref().unwrap().poses.clone();
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/development/train/development-blender-body.glb");
    let asset = s.assets.import_now(&file, &s.path).unwrap();
    assert!(s.assign_train_asset(
        "vehicles",
        "seat-1",
        Some(super::train_assets::identity(asset))
    ));
    s.playback.sync(s.editor.project(), s.options);
    assert!(s.playback.active());
    assert_eq!(s.playback.snapshot.as_ref().unwrap().ticks, 1);
    assert_eq!(s.playback.snapshot.as_ref().unwrap().poses, before);
    s.undo(false);
    s.playback.sync(s.editor.project(), s.options);
    assert!(s.playback.active());
    let mut trains = s.editor.project().trains.clone();
    trains[0].components[0].mass_kg += 1.;
    assert!(s.execute(vec![Command::SetTrains(trains)]));
    s.playback.sync(s.editor.project(), s.options);
    assert!(!s.playback.active());
    assert!(s.playback.snapshot.is_none());
    s.playback
        .initialize(s.editor.project(), s.options, &track, 50.);
    wait(&mut s, |p| p.ticks == 0);
    s.undo(false);
    s.playback.sync(s.editor.project(), s.options);
    assert!(!s.playback.active());
}
#[test]
fn fixture_controls_accept_more_than_two_cars_and_below_track_arrangement() {
    let mut s = session();
    let ctx = egui::Context::default();
    s.playback.fixture_count = 4;
    s.playback.arrangement = DevelopmentArrangement::BelowTrack;
    for _ in 0..4 {
        frame(&mut s, &ctx, vec![]);
    }
    click(&mut s, &ctx, "Add configurable development consist");
    let new = s.editor.project().trains.last().unwrap();
    assert_eq!(new.cars.len(), 4);
    assert!(new.components.iter().filter(|c|c.kind==qx_document::train::ComponentKind::CarBody).all(|c|matches!(&c.placement,Some(qx_document::train::mechanism::Placement::Track {frame,..}) if frame.translation_m.y<0.)));
    s.undo(false);
    assert_eq!(s.editor.project().trains.len(), 1);
}
