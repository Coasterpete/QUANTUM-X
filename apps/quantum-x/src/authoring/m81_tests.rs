use super::{
    session::{Session, blank_project, scene},
    train_assets, train_preview,
};
use qx_document::{
    command::Command,
    persistence,
    train::{Asset, Train},
};
use qx_geometry::math::{Quaternion, Vec3};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/development/train/development-blender-body.glb")
}
fn session() -> Session {
    let mut p = blank_project();
    p.sections[0] = qx_geometry::authoring::Section::straight(1, "Train inspection fixture", 100.);
    p.trains.push(Train::development_fixture("train".into()));
    p.heartline.train_configuration_id = Some("train".into());
    Session::new(p, None, qx_style::Catalog::built_in().unwrap()).unwrap()
}
fn temp() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "qx-m81-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn editor_assignment_remove_undo_save_as_reopen_and_missing_asset() {
    let mut s = session();
    let original = s.editor.project().clone();
    let key = s.assets.import_now(&fixture(), &s.path).unwrap();
    let mut a = train_assets::identity(key.clone());
    a.pivot_m = Vec3::new(0.12, 0.23, 0.34);
    for id in ["car-1", "car-2"] {
        assert!(s.assign_train_asset("train", id, Some(a.clone())));
    }
    let assigned = s.editor.project().clone();
    let mut mechanical = assigned.clone();
    for c in &mut mechanical.trains[0].components {
        c.asset = None;
    }
    assert_eq!(mechanical, original);
    assert!(s.assign_train_asset("train", "car-1", None));
    s.undo(false);
    assert_eq!(*s.editor.project(), assigned);
    s.undo(true);
    assert!(s.editor.project().trains[0].components[0].asset.is_none());
    s.undo(false);
    let first = temp();
    let path = first.join("first.qx.json");
    s.save_to(&path);
    assert!(s.save_failure.is_none(), "{}", s.status);
    assert!(first.join(&key).exists());
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), assigned);
    let second = temp();
    let relocated = second.join("second.qx.json");
    s.save_to(&relocated);
    assert!(s.save_failure.is_none(), "{}", s.status);
    s.open(Some(relocated), true);
    s.assets.load_blocking(s.editor.project(), &s.path);
    assert!(s.assets.errors.is_empty());
    assert_eq!(s.editor.project(), &assigned);
    assert_eq!(s.assets.library.len(), 1);
    // Remove external fixture copies deliberately, preserving persisted references.
    std::fs::remove_file(second.join(&key)).unwrap();
    s.assets = Default::default();
    s.assets.load_blocking(s.editor.project(), &s.path);
    assert!(s.assets.errors.contains_key(&key));
    assert_eq!(s.editor.project(), &assigned);
    let capture = scene(s.editor.project(), &s.catalog, Default::default()).unwrap();
    let poses = qx_physics::consist::inspect(&assigned.trains[0], &capture.track, 40.).unwrap();
    let mut display = Default::default();
    train_preview::populate(
        &assigned.trains[0],
        &capture.track,
        &poses,
        &mut s.assets,
        &mut display,
    );
    assert_eq!(display.asset_instances.len(), 6);
    assert!(
        display
            .asset_instances
            .iter()
            .all(|a| Arc::ptr_eq(&a.model, &s.assets.fallback))
    );
    s.save();
    assert!(s.status.contains("Missing asset preserved"));
    std::fs::remove_dir_all(first).unwrap();
    std::fs::remove_dir_all(second).unwrap();
}
#[test]
fn coordinate_correction_uses_pivot_before_scale_then_rotation_then_translation() {
    let mut a = train_assets::identity("assets/body.glb".into());
    a.pivot_m = Vec3::new(1., 2., 3.);
    a.scale = Vec3::new(2., 3., 4.);
    a.translation_m = Vec3::new(5., 6., 7.);
    a.rotation = Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap();
    let world = Quaternion::axis_angle(Vec3::X, std::f64::consts::FRAC_PI_2).unwrap();
    let m = train_assets::transform(Vec3::new(10., 20., 30.), world, &a);
    let p = [2., 4., 6.];
    let actual: Vec<_> = (0..3)
        .map(|row| m[3][row] + (0..3).map(|col| m[col][row] * p[col]).sum::<f64>())
        .collect();
    // model-pivot=(1,2,3), scaled=(2,6,12), Rz=(-6,2,12),
    // translated=(-1,8,19), Rx=(-1,-19,8), world=(9,1,38).
    for (x, y) in actual.iter().zip([9., 1., 38.]) {
        assert!((x - y).abs() < 1e-12);
    }
}
#[test]
fn repeated_instances_follow_station_and_overlay_visibility_is_independent() {
    let mut s = session();
    let key = s.assets.import_now(&fixture(), &s.path).unwrap();
    for id in ["car-1", "car-2"] {
        assert!(s.assign_train_asset("train", id, Some(train_assets::identity(key.clone()))));
    }
    let p = s.editor.project().clone();
    let capture = scene(&p, &s.catalog, Default::default()).unwrap();
    let train = &p.trains[0];
    let mut a = qx_rendering::DisplayOptions::default();
    let first = qx_physics::consist::inspect(train, &capture.track, 30.).unwrap();
    train_preview::populate(train, &capture.track, &first, &mut s.assets, &mut a);
    assert!(Arc::ptr_eq(
        &a.asset_instances[0].model,
        &a.asset_instances[3].model
    ));
    assert!(a.train_bounds.unwrap().1.x - a.train_bounds.unwrap().0.x < 10.);
    let second = qx_physics::consist::inspect(train, &capture.track, 45.).unwrap();
    let mut b = Default::default();
    train_preview::populate(train, &capture.track, &second, &mut s.assets, &mut b);
    assert_eq!(a.asset_instances.len(), b.asset_instances.len());
    for (a, b) in a.asset_instances.iter().zip(&b.asset_instances) {
        assert!((b.transform[3][0] - a.transform[3][0] - 15.).abs() < 1e-9);
    }
    s.assets.wireframe = false;
    s.assets.wheels = false;
    s.assets.joints = false;
    s.assets.couplers = false;
    let mut clean = Default::default();
    train_preview::populate(train, &capture.track, &second, &mut s.assets, &mut clean);
    assert!(clean.overlay_lines.is_empty());
    assert_eq!(clean.asset_instances, b.asset_instances);
    s.assets.bogies = false;
    let mut bodies = Default::default();
    train_preview::populate(train, &capture.track, &second, &mut s.assets, &mut bodies);
    assert_eq!(bodies.asset_instances.len(), 2);
}
#[test]
fn old_schema_five_assets_default_to_zero_pivot_and_failed_assignment_is_transactional() {
    let mut s = session();
    let original = s.editor.project().clone();
    let missing = Asset {
        path: "assets/missing.glb".into(),
        ..train_assets::identity(String::new())
    };
    assert!(!s.assign_train_asset("train", "car-1", Some(missing.clone())));
    assert_eq!(s.editor.project(), &original);
    let mut trains = original.trains.clone();
    trains[0].components[0].asset = Some(missing);
    assert!(s.execute(vec![Command::SetTrains(trains)]));
    let mut value = serde_json::to_value(s.editor.project()).unwrap();
    value["trains"][0]["components"][0]["asset"]
        .as_object_mut()
        .unwrap()
        .remove("pivot_m");
    let loaded = persistence::from_json(&value.to_string(), &s.catalog).unwrap();
    assert_eq!(loaded, s.editor.project().clone());
}
#[test]
fn native_import_completion_and_failure_use_worker_without_changing_mechanics() {
    let mut s = session();
    let original = s.editor.project().clone();
    for (path, success) in [
        (fixture(), true),
        (fixture().with_file_name("does-not-exist.glb"), false),
    ] {
        let (tx, result) = std::sync::mpsc::channel();
        s.file_dialog = Some(super::files::Dialog {
            action: super::files::FileAction::ImportGlb,
            result,
        });
        tx.send(Some(path)).unwrap();
        s.poll_file_dialog(&egui::Context::default());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while s.assets.busy() && std::time::Instant::now() < deadline {
            s.assets.poll(s.editor.project(), &s.path);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!s.assets.busy());
        assert_eq!(s.assets.errors.contains_key("Import"), !success);
        assert_eq!(s.editor.project(), &original);
    }
    assert_eq!(s.assets.library.len(), 1);
    let key = s.assets.library.keys().next().unwrap().clone();
    let mut asset = train_assets::identity(key);
    asset.scale.x = 1e-200;
    assert!(!s.assign_train_asset("train", "car-1", Some(asset)));
    assert_eq!(s.editor.project(), &original);
}
#[test]
fn save_as_packages_assets_needed_by_undo_even_before_first_asset_load() {
    let first = temp();
    let second = temp();
    let path = first.join("first.qx.json");
    let mut s = session();
    let key = s.assets.import_now(&fixture(), &s.path).unwrap();
    assert!(s.assign_train_asset("train", "car-1", Some(train_assets::identity(key.clone()))));
    s.save_to(&path);
    assert!(s.save_failure.is_none());
    let p = persistence::load(&path, &s.catalog).unwrap();
    let mut fresh = Session::new(p, Some(&path), s.catalog.clone()).unwrap();
    assert!(fresh.assets.library.is_empty());
    assert!(fresh.assign_train_asset("train", "car-1", None));
    fresh.save_to(&second.join("second.qx.json"));
    assert!(fresh.save_failure.is_none(), "{}", fresh.status);
    assert!(second.join(&key).exists());
    fresh.undo(false);
    fresh
        .assets
        .load_blocking(fresh.editor.project(), &fresh.path);
    assert!(fresh.assets.errors.is_empty());
    assert!(fresh.assets.library.contains_key(&key));
    std::fs::remove_dir_all(first).unwrap();
    std::fs::remove_dir_all(second).unwrap();
}

fn ui_frame(
    s: &mut Session,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> (
    Vec<(String, egui::Rect)>,
    qx_rendering::SurfaceOutput,
    qx_rendering::DisplayOptions,
) {
    use qx_rendering::EditorSurface;
    let scene = scene(s.editor.project(), &s.catalog, Default::default()).unwrap();
    let display_id = egui::Id::unique("m81 headless display state");
    let mut display = ctx
        .data_mut(|d| d.get_temp::<qx_rendering::DisplayOptions>(display_id))
        .unwrap_or_default();
    let mut surface = None;
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
            surface = Some(s.ui(ctx, &scene, &mut display));
        },
    );
    fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    texts(s, out)
                }
            }
            _ => {}
        }
    }
    let mut labels = vec![];
    ctx.data_mut(|d| d.insert_temp(display_id, display.clone()));
    output.textures_delta.clear();
    for shape in output.shapes {
        texts(&shape.shape, &mut labels);
    }
    (labels, surface.unwrap(), display)
}
fn ui_click(s: &mut Session, ctx: &egui::Context, text: &str) -> qx_rendering::SurfaceOutput {
    let labels = ui_frame(s, ctx, vec![]).0;
    let center = labels
        .iter()
        .find(|(label, _)| label == text)
        .unwrap_or_else(|| panic!("Missing UI label {text}"))
        .1
        .center();
    ui_frame(
        s,
        ctx,
        vec![
            egui::Event::PointerMoved(center),
            egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    ui_frame(
        s,
        ctx,
        vec![egui::Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
    .1
}
#[test]
fn real_train_workspace_buttons_station_and_visibility_reach_display_and_history() {
    let mut s = session();
    s.workbench.active = super::workbench::Profile::Train;
    s.assets.chosen = s.assets.import_now(&fixture(), &s.path).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..4 {
        ui_frame(&mut s, &ctx, vec![]);
    }
    ui_click(&mut s, &ctx, "Assign / replace");
    assert!(s.editor.project().trains[0].components[0].asset.is_some());
    let bogie = fixture().with_file_name("development-bogie.glb");
    let replacement = s.assets.import_now(&bogie, &s.path).unwrap();
    s.assets.chosen = replacement.clone();
    ui_click(&mut s, &ctx, "Assign / replace");
    assert_eq!(s.assets.draft.as_ref().unwrap().path, replacement);
    ui_click(&mut s, &ctx, "Apply transform");
    assert_eq!(
        s.editor.project().trains[0].components[0]
            .asset
            .as_ref()
            .unwrap()
            .path,
        replacement
    );
    assert!(matches!(
        ui_click(&mut s, &ctx, "Fit Train · F").view_command,
        Some(qx_rendering::ViewCommand::FitTrain)
    ));
    assert!(matches!(
        ui_click(&mut s, &ctx, "Focus Selected Car").view_command,
        Some(qx_rendering::ViewCommand::FocusSelectedCar)
    ));
    ui_click(&mut s, &ctx, "50.000");
    ui_frame(
        &mut s,
        &ctx,
        vec![
            egui::Event::Text("42.125".into()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert_eq!(s.train_station_m, Some(42.125));
    ui_click(&mut s, &ctx, "View");
    ui_click(&mut s, &ctx, "Development tools");
    assert!(s.development);
    ui_click(&mut s, &ctx, "Engineering references");
    for label in [
        "Engineering wireframe / axes / guides",
        "Wheel-reference markers",
        "Joint-anchor markers",
        "Coupler markers",
    ] {
        ui_click(&mut s, &ctx, label);
    }
    let (_, _, display) = ui_frame(&mut s, &ctx, vec![]);
    assert!(display.overlay_lines.is_empty());
    assert_eq!(display.asset_instances.len(), 6);
    ui_click(&mut s, &ctx, "Remove assignment");
    assert!(s.editor.project().trains[0].components[0].asset.is_none());
    s.undo(false);
    assert!(s.editor.project().trains[0].components[0].asset.is_some());
}
#[test]
fn visual_correction_drafts_share_save_and_history_guards_and_apply_atomically() {
    let mut s = session();
    let key = s.assets.import_now(&fixture(), &s.path).unwrap();
    let a = train_assets::identity(key);
    assert!(s.assign_train_asset("train", "car-1", Some(a.clone())));
    s.assets.selected = Some(("train".into(), "car-1".into()));
    let mut draft = a;
    draft.translation_m.y = 0.123456789;
    draft.rotation.w = 2.;
    s.assets.draft = Some(draft);
    let before = s.editor.project().clone();
    assert!(s.has_drafts());
    s.undo(false);
    assert!(matches!(
        s.draft_request,
        Some(super::draft_workflow::DraftRequest::History { .. })
    ));
    assert!(s.assets.draft.is_some());
    assert_eq!(s.editor.project(), &before);
    let dir = temp();
    let path = dir.join("visual.qx.json");
    s.draft_request = None;
    s.save_to(&path);
    assert!(matches!(
        s.draft_request,
        Some(super::draft_workflow::DraftRequest::SaveTo(_))
    ));
    assert!(!path.exists());
    s.assets.draft.as_mut().unwrap().scale.x = 0.;
    s.resolve_drafts(true);
    assert_eq!(s.editor.project(), &before);
    assert!(!path.exists());
    assert!(s.resolution_error.is_some());
    s.assets.draft.as_mut().unwrap().scale.x = 1.;
    s.resolve_drafts(true);
    assert!(!s.has_drafts());
    assert!(s.save_failure.is_none(), "{}", s.status);
    let saved = persistence::load(&path, &s.catalog).unwrap();
    let asset = saved.trains[0].components[0].asset.as_ref().unwrap();
    assert_eq!(asset.translation_m.y, 0.123456789);
    assert_eq!(asset.rotation, Quaternion::IDENTITY);
    s.undo(false);
    assert_eq!(s.editor.project(), &before);
    s.undo(true);
    assert_eq!(s.editor.project(), &saved);
    std::fs::remove_dir_all(dir).unwrap();
}
