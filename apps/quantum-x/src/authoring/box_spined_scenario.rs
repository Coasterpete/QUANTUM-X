//! Actual Editor/GPU evidence with numerical fixtures; no desktop input.
use super::{
    coaster_properties,
    material_editor_scenario::{click, frame, wait},
    material_ui,
    session::{Session, blank_project, scene},
    workbench::Profile,
};
use qx_document::{
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    math::{Quaternion, Vec3},
    transition::{Channel, Unit},
};
use qx_rendering::{DisplayMode, DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use qx_style::{Catalog, materials::Binding};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

struct Capture<'a> {
    session: &'a mut Session,
    close: bool,
}
impl EditorSurface for Capture<'_> {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let output = self.session.ui(ctx, scene, display);
        display.mode = DisplayMode::Rendered;
        display.highlights = false;
        display.centerline = false;
        display.boundaries = false;
        display.inspection_marker = false;
        if self.close {
            display.train_bounds = Some((Vec3::new(0., -1.05, -0.75), Vec3::new(3., 0.3, 0.75)));
        }
        output
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        None
    }
    fn open_path(&mut self, _: &Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
fn capture(
    s: &mut Session,
    sc: &InspectionScene,
    root: &Path,
    name: &str,
    close: bool,
) -> Result<String, Box<dyn Error>> {
    s.workbench = Default::default();
    s.workbench.active = Profile::Track;
    let report = qx_rendering::render_offscreen(
        sc,
        &mut Capture { session: s, close },
        &root.join(format!("{name}.png")),
        2000,
        1600,
    )?;
    println!("{name}: {report}");
    Ok(report)
}
fn tint(
    c: &mut qx_style::construction::TrackConstruction,
    component: &str,
    finish: &str,
    color: [f32; 3],
) {
    let slot = c.material_slot(component).unwrap().to_owned();
    let m = c.materials.iter_mut().find(|m| m.id == slot).unwrap();
    let mut binding = Binding::new(finish);
    binding.tint = color;
    m.library = Some(binding);
}

/// Catalog images use the same scene and mesh renderer as the Editor.
struct Preview;
impl EditorSurface for Preview {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        _: &InspectionScene,
        d: &mut DisplayOptions,
    ) -> SurfaceOutput {
        d.mode = DisplayMode::Rendered;
        d.grid = false;
        d.centerline = false;
        d.boundaries = false;
        d.highlights = false;
        d.inspection_marker = false;
        SurfaceOutput {
            viewport: ctx.content_rect(),
            view_command: None,
            close: false,
            window_command: None,
            profile: 0,
            reset_navigation: false,
        }
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        None
    }
    fn open_path(&mut self, _: &Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
pub(crate) fn run(root: PathBuf) -> Result<(), Box<dyn Error>> {
    if root.exists() {
        return Err("Choose a new Box-Spined evidence directory".into());
    }
    std::fs::create_dir_all(&root)?;
    let catalog = Catalog::built_in()?;
    let mut original = blank_project();
    original.style = Default::default();
    original.metadata.name = "M2 · original Box-Spined source integration".into();
    original.sections = vec![Section::straight(1, "Numerical straight fixture", 6.)];
    original.trains.push(Train::development_consist(
        "gray-box-train".into(),
        1,
        DevelopmentArrangement::Conventional,
    )?);
    let mut s = Session::new(original.clone(), None, catalog.clone())?;
    coaster_properties::open(&mut s);
    s.regenerate();
    let initial = wait(&mut s)?;
    let ctx = egui::Context::default();
    for _ in 0..5 {
        frame(&mut s, &ctx, &initial, vec![]);
    }
    click(&mut s, &ctx, &initial, "Choose a family", 0)?;
    click(&mut s, &ctx, &initial, "Box Spined", 0)?;
    click(&mut s, &ctx, &initial, "Unassigned", 0)?;
    let style = catalog
        .styles
        .iter()
        .find(|s| s.id == "box-spined-100")
        .unwrap();
    click(
        &mut s,
        &ctx,
        &initial,
        &format!("{} - {}", style.name, style.status.label()),
        0,
    )?;
    let draft = wait(&mut s)?;
    if s.editor.project() != &original || draft.mesh.as_ref().unwrap().components.len() != 4 {
        return Err(
            "Style selection failed to preview original source construction transactionally".into(),
        );
    }
    click(&mut s, &ctx, &draft, "Cancel", 0)?;
    wait(&mut s)?;
    if s.construction_draft != original.construction {
        return Err("Style Cancel failed".into());
    }
    coaster_properties::select_style(
        &mut s,
        qx_style::StyleSelection {
            family_id: "box-spined".into(),
            style_id: "box-spined-100".into(),
        },
    )?;
    let draft = wait(&mut s)?;
    click(&mut s, &ctx, &draft, "Apply", 0)?;
    let applied = s.editor.project().clone();
    let applied_scene = wait(&mut s)?;
    if applied.style.style_id != "box-spined-100"
        || applied.sections != original.sections
        || applied.heartline != original.heartline
        || applied.trains != original.trains
        || applied.initial != original.initial
    {
        return Err("Style apply changed unrelated authoring data".into());
    }
    let mut reports = vec![capture(
        &mut s,
        &applied_scene,
        &root,
        "01-editor-style-selection",
        true,
    )?];
    coaster_properties::open(&mut s);
    s.material_ui.appearance_tab = true;
    tint(
        &mut s.construction_draft,
        "rail-left",
        "polished-steel",
        [0.7, 0.8, 0.95],
    );
    tint(
        &mut s.construction_draft,
        "spine",
        "gloss-painted-steel",
        [0.03, 0.3, 0.58],
    );
    s.refresh_view();
    let painted = wait(&mut s)?;
    if !material_ui::apply(&mut s) {
        return Err("Appearance Apply failed".into());
    }
    let painted_project = s.editor.project().clone();
    reports.push(capture(
        &mut s,
        &painted,
        &root,
        "02-independent-gloss-and-polished-rails",
        true,
    )?);
    tint(
        &mut s.construction_draft,
        "spine",
        "gloss-painted-steel",
        [0.7, 0.04, 0.02],
    );
    s.refresh_view();
    let red = wait(&mut s)?;
    reports.push(capture(
        &mut s,
        &red,
        &root,
        "03-independent-spine-recolor-draft",
        true,
    )?);
    material_ui::cancel(&mut s);
    let restored = wait(&mut s)?;
    if s.editor.project() != &painted_project || restored.mesh != painted.mesh {
        return Err("Appearance Cancel changed material/geometry".into());
    }
    s.material_ui.appearance_tab = false;
    frame(&mut s, &ctx, &restored, vec![]);
    if s.construction_draft != painted_project.construction {
        return Err("Leaving Track Appearance changed colors".into());
    }
    s.undo(false);
    wait(&mut s)?;
    if s.editor.project() != &applied {
        return Err("Appearance undo failed".into());
    }
    s.undo(false);
    wait(&mut s)?;
    if s.editor.project() != &original {
        return Err("Style undo failed".into());
    }
    s.undo(true);
    wait(&mut s)?;
    s.undo(true);
    wait(&mut s)?;
    if s.editor.project() != &painted_project {
        return Err("Style/appearance redo failed".into());
    }
    persistence::save(
        &root.join("box-spined-source.qx.json"),
        &painted_project,
        &catalog,
    )?;
    let reopened = persistence::load(&root.join("box-spined-source.qx.json"), &catalog)?;
    if reopened != painted_project {
        return Err("Box-Spined save/reopen failed".into());
    }
    let mut cases = vec![];
    let mut hill = Section::straight(1, "Smooth hill fixture", 16.);
    hill.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 16., -0.045),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 16., 0.),
    };
    let mut curve = Section::straight(1, "Horizontal curve fixture", 18.);
    curve.kind = SectionKind::Geometric {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 18., 0.),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 18., 0.075),
    };
    let mut loop_section =
        Section::straight(1, "Inverted loop fixture", std::f64::consts::TAU * 8.);
    loop_section.kind = SectionKind::Arc {
        radius_m: 8.,
        plane_angle_rad: 0.,
    };
    for (name, sections, pitch, bank) in [
        (
            "04-straight",
            vec![Section::straight(1, "Straight fixture", 8.)],
            0.,
            0.,
        ),
        ("05-smooth-hill", vec![hill], 0.36, 0.),
        ("06-horizontal-curve", vec![curve.clone()], 0., 0.),
        ("07-banked-curve", vec![curve], 0., 0.9),
        ("08-inverted-loop", vec![loop_section], 0., 0.),
        (
            "09-connected-sections",
            vec![
                Section::straight(1, "Section A", 3.),
                Section::straight(2, "Section B", 4.),
                Section::straight(3, "Section C", 3.),
            ],
            0.,
            0.,
        ),
    ] {
        let mut p = reopened.clone();
        p.metadata.name = format!("Box-Spined 100 · {name} · numerical fixture");
        p.sections = sections;
        p.initial.orientation =
            Quaternion::axis_angle(Vec3::Z, pitch)?.compose(Quaternion::axis_angle(Vec3::X, bank)?);
        let sc = scene(&p, &catalog, Default::default())?;
        let mut s = Session::new(p.clone(), None, catalog.clone())?;
        coaster_properties::open(&mut s);
        persistence::save(&root.join(format!("{name}.qx.json")), &p, &catalog)?;
        let report = capture(&mut s, &sc, &root, name, false)?;
        cases.push(serde_json::json!({"case":name,"samples":sc.track.samples.len(),"vertices":sc.mesh.as_ref().unwrap().stats.vertices,"triangles":sc.mesh.as_ref().unwrap().stats.triangles,"hardware_instances":sc.mesh.as_ref().unwrap().stats.instances,"report":report}));
    }
    let sources =
        qx_rendering::construction_assets::Sources::new(&super::library_ui::assets_root());
    let source = sources.original("tracks/box-spined/100/Box-Spine-100.glb", [1.; 3])?;
    let tie = sources.original("tracks/box-spined/100/Box-Spine-100-crosstie.glb", [1.; 3])?;
    let mut short = reopened.clone();
    short.sections = vec![Section::straight(1, "Registered source preview", 3.)];
    short.trains.clear();
    let preview = scene(&short, &catalog, Default::default())?;
    for (name, width, height) in [("preview.png", 1200, 800), ("thumbnail.png", 640, 480)] {
        qx_rendering::render_offscreen(&preview, &mut Preview, &root.join(name), width, height)?;
    }
    // Compare the unmodified source surfaces against the swept result in the
    // actual Editor. One source tie is shown at the source segment midpoint;
    // this is a comparison fixture, never an inferred production repeat rule.
    short.sections = vec![Section::straight(
        1,
        "Original 1.5 m source comparison",
        1.5,
    )];
    let mut source_scene = scene(&short, &catalog, Default::default())?;
    let mesh = std::sync::Arc::make_mut(source_scene.mesh.as_mut().unwrap());
    mesh.components[2].mesh = (*source).clone();
    mesh.components[2].bounds = source.bounds;
    mesh.components
        .retain(|c| c.role != qx_style::construction::Role::Hardware);
    let material = mesh.components[2].material.clone();
    let frame = qx_geometry::math::Frame::new(Quaternion::IDENTITY)?;
    let placement = qx_geometry::construction::Placement {
        station_m: 0.75,
        source_station_m: 0.75,
        position_m: Vec3::X * 0.75,
        frame,
    };
    let mut bounds = tie.bounds;
    bounds.min = bounds.min + Vec3::X * 0.75;
    bounds.max = bounds.max + Vec3::X * 0.75;
    mesh.components.push(qx_geometry::construction::Component {
        id: "original-crosstie-comparison".into(),
        role: qx_style::construction::Role::Hardware,
        material,
        visible: true,
        mesh: (*tie).clone(),
        placements: vec![placement],
        bounds,
    });
    let mut source_session = Session::new(short, None, catalog.clone())?;
    source_session.status =
        "Original GLB comparison fixture · one tie at source midpoint; not a placement definition"
            .into();
    reports.push(capture(
        &mut source_session,
        &source_scene,
        &root,
        "10-original-glb-surfaces-comparison",
        true,
    )?);
    std::fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"verified_visual_construction_with_owner_authorized_provisional_dimensions", "editor_reports":reports,"cases":cases,
            "source_spine":{"vertices":source.vertices.len(),"triangles":source.indices.len()/3},
            "source_crosstie":{"vertices":tie.vertices.len(),"triangles":tie.indices.len()/3},
            "crossties_installed":true,"rail_center_spacing_m":1.10,"rail_diameter_m":0.13,"cross_tie_spacing_m":1.50,"production_dimensions_finalized":false,
            "cancel_apply_undo_redo_save_reopen":true,"desktop_input":false
        }))?,
    )?;
    Ok(())
}
