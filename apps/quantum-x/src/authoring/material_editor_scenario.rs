//! Real Editor evidence with in-memory egui events and GPU captures; no desktop.
use super::{
    coaster_properties,
    session::{Session, blank_project},
    workbench::Profile,
};
use qx_document::{
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::authoring::Section;
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(super) fn wait(session: &mut Session) -> Result<InspectionScene, String> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(scene) = session.poll() {
            return Ok(scene);
        }
        if let Some(error) = &session.draft_error {
            return Err(error.clone());
        }
        if Instant::now() > until {
            return Err(format!("Scene worker timeout: {}", session.status));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
pub(super) fn frame(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    fn labels(shape: &egui::Shape, items: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(t) => items.push((t.galley.text().into(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => {
                for shape in v {
                    labels(shape, items);
                }
            }
            _ => {}
        }
    }
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2000., 1600.),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            session.ui(ctx, scene, &mut DisplayOptions::default());
        },
    );
    let mut items = vec![];
    for shape in &output.shapes {
        labels(&shape.shape, &mut items);
    }
    output.textures_delta.clear();
    items
}
pub(super) fn click(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    text: &str,
    index: usize,
) -> Result<(), String> {
    let mut items = frame(session, ctx, scene, vec![]);
    let choosing_material = session.material_ui.choosing.is_some()
        && session
            .material_catalog
            .as_ref()
            .is_ok_and(|library| library.materials.iter().any(|m| m.name == text));
    // Exercise scrolling inside the actual gallery, including short Track docks.
    for _ in 0..24 {
        let target = items
            .iter()
            .filter(|(s, _)| s == text)
            .filter(|(_, r)| {
                !choosing_material
                    || session
                        .material_ui
                        .chooser_rect
                        .is_some_and(|g| g.contains(r.center()))
            })
            .nth(if choosing_material { 0 } else { index });
        if target.is_some() {
            break;
        }
        let Some(rect) = session.material_ui.chooser_rect.filter(|r| r.is_positive()) else {
            break;
        };
        items = frame(
            session,
            ctx,
            scene,
            vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    source: egui::MouseWheelSource::Unknown,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0., -60.),
                    modifiers: Default::default(),
                },
            ],
        );
    }
    let position = items
        .iter()
        .filter(|(s, _)| s == text)
        .filter(|(_, r)| {
            !choosing_material
                || session
                    .material_ui
                    .chooser_rect
                    .is_some_and(|g| g.contains(r.center()))
        })
        .nth(if choosing_material { 0 } else { index })
        .ok_or_else(|| {
            format!(
                "Editor control missing: {text} [{index}]; visible: {:?}",
                items.iter().map(|(s, _)| s).collect::<Vec<_>>()
            )
        })?
        .1
        .center();
    for pressed in [true, false] {
        frame(
            session,
            ctx,
            scene,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    Ok(())
}
fn capture(
    session: &mut Session,
    scene: &InspectionScene,
    output: &Path,
    file: &str,
) -> Result<String, String> {
    // Every capture creates a fresh egui context: do not reuse another context's
    // dock geometry cache. The fixture always uses the actual default layout.
    let profile = session.workbench.active;
    session.workbench = Default::default();
    session.workbench.active = profile;
    // The ordinary Session draws the complete Editor. No substitute surface or
    // image composited from a separately rendered viewport is involved.
    qx_rendering::render_offscreen(scene, session, &output.join(file), 2000, 1800)
}
pub(crate) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("Choose a new material Editor evidence directory".into());
    }
    std::fs::create_dir_all(&output)?;
    let catalog = qx_style::Catalog::built_in()?;
    let mut project = blank_project();
    project.metadata.name = "M1.1 · procedural material verification fixture".into();
    project.sections = vec![Section::straight(
        1,
        "Short straight · verification only",
        6.,
    )];
    project.trains.push(Train::development_consist(
        "gray-box-train".into(),
        1,
        DevelopmentArrangement::Conventional,
    )?);
    project.heartline.train_configuration_id = Some("gray-box-train".into());
    let mut s = Session::new(project.clone(), None, catalog.clone())?;
    coaster_properties::open(&mut s);
    s.material_ui.appearance_tab = true;
    s.regenerate();
    let ctx = egui::Context::default();
    let initial = wait(&mut s)?;
    for _ in 0..4 {
        frame(&mut s, &ctx, &initial, vec![]);
    }
    let default = "Default finish";
    let baseline = capture(&mut s, &initial, &output, "01-construction-default.png")?;
    click(&mut s, &ctx, &initial, default, 0)?;
    click(&mut s, &ctx, &initial, "Track bolt metal", 0)?;
    let rails = wait(&mut s)?;
    if !rails.materials.slots.contains_key("running") || s.editor.project() != &project {
        return Err(
            "Named selector did not produce an uncommitted live rail material preview".into(),
        );
    }
    let rail_report = capture(
        &mut s,
        &rails,
        &output,
        "02-registered-procedural-rails-draft.png",
    )?;
    click(&mut s, &ctx, &rails, default, 0)?;
    click(&mut s, &ctx, &rails, "Track pine", 0)?;
    let mixed = wait(&mut s)?;
    if !mixed.materials.slots.contains_key("structure") {
        return Err("Spine binding did not reach the live renderer".into());
    }
    let mixed_report = capture(&mut s, &mixed, &output, "03-independent-spine-draft.png")?;
    click(&mut s, &ctx, &mixed, "Apply appearance", 0)?;
    let applied = s.editor.project().clone();
    if applied.sections != project.sections
        || applied.initial != project.initial
        || applied.heartline != project.heartline
        || applied.trains != project.trains
    {
        return Err("Appearance edited geometry or train data".into());
    }
    click(&mut s, &ctx, &mixed, "Track bolt metal", 0)?;
    click(&mut s, &ctx, &mixed, "Track pine", 1)?;
    let changed = wait(&mut s)?;
    capture(
        &mut s,
        &changed,
        &output,
        "04-live-rail-material-change.png",
    )?;
    click(&mut s, &ctx, &changed, "Cancel appearance", 0)?;
    let restored = wait(&mut s)?;
    if s.editor.project() != &applied || s.construction_draft != applied.construction {
        return Err("Appearance Cancel failed".into());
    }
    s.material_ui.inspected = Some("track-pine".into());
    s.material_ui.scroll_to_inspection = true;
    capture(&mut s, &restored, &output, "05-smoothness-inspection.png")?;
    s.undo(false);
    let undone = wait(&mut s)?;
    if s.editor.project() != &project || undone.mesh != initial.mesh {
        return Err("Appearance undo failed".into());
    }
    s.undo(true);
    let redone = wait(&mut s)?;
    if s.editor.project() != &applied {
        return Err("Appearance redo failed".into());
    }
    let saved = output.join("material-assignments.qx.json");
    s.save_to(&saved);
    if let Some(error) = &s.save_failure {
        return Err(error.clone().into());
    }
    let reopened = persistence::load(&saved, &catalog)?;
    if reopened != applied {
        return Err("Appearance save/reopen failed".into());
    }
    s.workbench.active = Profile::Train;
    s.train_station_m = Some(3.);
    let train_report = capture(
        &mut s,
        &redone,
        &output,
        "06-neutral-train-placeholders.png",
    )?;
    // Color-only changes use the same finish, maps and existing draft/history.
    s.workbench.active = Profile::Track;
    let mut colors = vec![];
    for (label, rgb) in [
        ("07-gloss-blue", [36, 104, 255]),
        ("08-gloss-red", [235, 30, 65]),
    ] {
        let mut binding = qx_style::materials::Binding::new("gloss-painted-steel");
        binding.tint = super::color_ui::linear(rgb);
        s.construction_draft
            .assign_material("running", Some(binding))?;
        s.preview();
        let colored = wait(&mut s)?;
        if colored.track.samples != initial.track.samples
            || colored.mesh.as_ref().unwrap().components
                != initial.mesh.as_ref().unwrap().components
        {
            return Err("Color changed geometry".into());
        }
        colors.push(colored.clone());
        capture(&mut s, &colored, &output, &format!("{label}.png"))?;
    }
    for (slot, rgb) in [("structure", [138, 32, 240]), ("hardware", [40, 230, 90])] {
        let mut binding = qx_style::materials::Binding::new("gloss-painted-steel");
        binding.tint = super::color_ui::linear(rgb);
        s.construction_draft.assign_material(slot, Some(binding))?;
    }
    s.preview();
    let independent = wait(&mut s)?;
    capture(&mut s, &independent, &output, "09-component-colors.png")?;
    for _ in 0..4 {
        frame(&mut s, &ctx, &independent, vec![]);
    }
    s.workbench = Default::default();
    let mut point = egui::Pos2::ZERO;
    let mut input = |ctx: &egui::Context, pass| {
        if pass == 3 {
            point = ctx
                .data(|d| {
                    d.get_temp::<egui::Rect>(egui::Id::unique((
                        "appearance color button",
                        &"running".to_owned(),
                    )))
                })
                .expect("Rendered rail swatch")
                .center();
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]
        } else if pass == 4 {
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]
        } else {
            vec![]
        }
    };
    qx_rendering::render_offscreen_with_input(
        &independent,
        &mut s,
        &output.join("10-native-egui-color-picker.png"),
        2000,
        1800,
        &mut input,
    )?;
    let picker: serde_json::Value = serde_json::from_slice(&std::fs::read(
        output.join("10-native-egui-color-picker.capture.json"),
    )?)?;
    if picker["popup_open"] != true {
        return Err("Color picker was not open in its capture".into());
    }
    if !super::material_ui::apply(&mut s) {
        return Err("Color apply failed".into());
    }
    s.save_to(&output.join("component-colors.qx.json"));
    if persistence::load(&output.join("component-colors.qx.json"), &catalog)? != *s.editor.project()
    {
        return Err("Color persistence failed".into());
    }
    for (finish, file) in [
        ("polished-steel", "11-polished-track.png"),
        ("satin-steel", "12-satin-track.png"),
    ] {
        let mut binding = qx_style::materials::Binding::new(finish);
        binding.tint = [0.7; 3];
        s.construction_draft
            .assign_material("running", Some(binding))?;
        s.preview();
        let scene = wait(&mut s)?;
        capture(&mut s, &scene, &output, file)?;
    }
    super::material_ui::cancel(&mut s);
    wait(&mut s)?;
    // Restore previous state for the original M1.1 regression evidence below.
    s.undo(false);
    wait(&mut s)?;
    let model = &s.assets.fallback;
    for primitive in &model.primitives {
        let m = &primitive.material;
        if m.color[0] != m.color[1]
            || m.color[1] != m.color[2]
            || m.metallic != 0.
            || m.roughness < 0.9
            || m.textures().iter().any(|t| t.is_some())
        {
            return Err("Train fallback is not untextured neutral matte gray".into());
        }
    }
    for next in [&rails, &mixed, &changed, &restored, &redone] {
        if next.track.samples != initial.track.samples || next.reference != initial.reference {
            return Err("Appearance altered numerical/reference data".into());
        }
        for (a, b) in initial
            .mesh
            .as_ref()
            .unwrap()
            .components
            .iter()
            .zip(&next.mesh.as_ref().unwrap().components)
        {
            if a != b {
                return Err("Appearance altered procedural mesh/UVs/placements".into());
            }
        }
    }
    let records = serde_json::json!({
        "actual_editor_surface":true,"automated_offscreen":true,"manual_desktop_tested":false,"development_tools_enabled":s.development,
        "named_ui_selections_exercised":true,"apply_cancel_undo_redo_save_reopen":true,"geometry_uv_reference_train_data_unchanged":true,
        "train_placeholder":{"base_color":model.primitives[0].material.color,"metallic":0,"roughness":model.primitives[0].material.roughness,"textures":0},
        "construction":applied.construction,"schema_version":applied.schema_version,
        "smoothness":s.material_catalog.as_ref().ok().unwrap().materials.iter().map(|d|serde_json::json!({"id":d.id,"smoothness":d.maps.smoothness,"roughness":d.maps.roughness,"roughness_factor":d.roughness,"normal_convention":d.normal_convention})).collect::<Vec<_>>(),
        "renderers":[baseline,rail_report,mixed_report,train_report]
    });
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    println!("Real Editor material evidence: {}", output.display());
    Ok(())
}
