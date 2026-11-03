//! Automated selection regression captures through the real Editor and GPU.
//! Test-only scene fixtures; never controls the owner's desktop.
use super::{
    coaster_properties,
    material_editor_scenario::wait,
    session::{Session, blank_project},
    workbench::Profile,
};
use qx_geometry::{authoring::Section, math::Vec3};
use qx_rendering::{DisplayMode, DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Capture<'a> {
    session: &'a mut Session,
    highlights: bool,
    imported: Vec<qx_rendering::asset::AssetInstance>,
}
impl EditorSurface for Capture<'_> {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let output = self.session.ui(ctx, scene, display);
        // Isolate material/selection pixels from optional technical overlays.
        // All widgets, docks and selection still come from the actual Editor.
        display.mode = DisplayMode::Rendered;
        display.grid = false;
        display.centerline = false;
        display.boundaries = false;
        display.inspection_marker = false;
        display.highlights = self.highlights;
        display.asset_instances = self.imported.clone();
        // Fixed bounds retain the identical camera across Track/Train layouts.
        display.train_bounds = Some((Vec3::new(0., -0.8, -0.9), Vec3::new(6., 1., 0.9)));
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
    scene: &InspectionScene,
    root: &Path,
    name: &str,
    highlights: bool,
    imported: &[qx_rendering::asset::AssetInstance],
) -> Result<serde_json::Value, Box<dyn Error>> {
    let profile = s.workbench.active;
    s.workbench = Default::default();
    s.workbench.active = profile;
    let report = qx_rendering::render_offscreen(
        scene,
        &mut Capture {
            session: s,
            highlights,
            imported: imported.to_vec(),
        },
        &root.join(format!("{name}.png")),
        2000,
        1600,
    )?;
    println!("{name}: {report}");
    Ok(serde_json::from_slice(&std::fs::read(
        root.join(format!("{name}.capture.json")),
    )?)?)
}
fn compare(
    root: &Path,
    a: &str,
    b: &str,
    allow_outline: bool,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let metadata = |name| -> Result<serde_json::Value, Box<dyn Error>> {
        Ok(serde_json::from_slice(&std::fs::read(
            root.join(format!("{name}.capture.json")),
        )?)?)
    };
    let ma = metadata(a)?;
    let mb = metadata(b)?;
    for key in ["viewport", "camera_target", "camera_distance"] {
        if ma[key] != mb[key] {
            return Err(format!("{a}/{b}: {key} differs").into());
        }
    }
    let viewport: [f32; 4] = serde_json::from_value(ma["viewport"].clone())?;
    let [x, y, w, h] = viewport.map(|x| x as u32);
    let a_image = image::open(root.join(format!("{a}.png")))?.into_rgba8();
    let b_image = image::open(root.join(format!("{b}.png")))?.into_rgba8();
    // Clear color from offscreen.rs, converted to the sRGB render target.
    let background = super::color_ui::srgb([0.018, 0.025, 0.035]);
    let mut changed = 0;
    let mut surface = 0;
    for yy in y..y + h {
        for xx in x..x + w {
            let a = a_image.get_pixel(xx, yy);
            let b = b_image.get_pixel(xx, yy);
            if a.0[..3] != background {
                surface += 1;
            }
            if a != b {
                changed += 1;
                if !allow_outline || a.0[..3] != background {
                    return Err(format!("{a:?}/{b:?}: shaded pixel changed at {xx},{yy}").into());
                }
            }
        }
    }
    if surface < 1000 || (allow_outline && changed < 100) {
        return Err(format!("Insufficient surface/selection evidence: {surface}/{changed}").into());
    }
    Ok(
        serde_json::json!({"baseline":a,"comparison":b,"surface_pixels_unchanged":surface,"changed_background_pixels":changed,"identical_camera_lighting":true}),
    )
}
pub(crate) fn run(root: PathBuf) -> Result<(), Box<dyn Error>> {
    if root.exists() {
        return Err("Choose a new selection evidence directory".into());
    }
    std::fs::create_dir_all(&root)?;
    let mut project = blank_project();
    project.metadata.name = "Selection/material regression · development fixture".into();
    project.sections = vec![
        Section::straight(1, "Selected section", 3.),
        Section::straight(2, "Unselected section", 3.),
    ];
    let mut s = Session::new(project.clone(), None, qx_style::Catalog::built_in()?)?;
    coaster_properties::open(&mut s);
    s.material_ui.appearance_tab = true;
    s.regenerate();
    let initial = wait(&mut s)?;
    let mut checks = vec![];
    for (finish, rgb, name) in [
        ("gloss-painted-steel", [235, 30, 65], "gloss-red"),
        ("gloss-painted-steel", [20, 235, 55], "gloss-green"),
        ("gloss-painted-steel", [225, 20, 240], "gloss-magenta"),
        ("gloss-painted-steel", [20, 220, 245], "gloss-cyan"),
        ("matte-paint", [235, 30, 65], "matte-red"),
        ("polished-steel", [235, 30, 65], "polished-red"),
        ("satin-steel", [36, 104, 255], "satin-blue"),
        ("track-bolt-metal", [235, 30, 65], "textured-red"),
    ] {
        for slot in &mut s.construction_draft.materials {
            let mut binding = qx_style::materials::Binding::new(finish);
            binding.tint = super::color_ui::linear(rgb);
            slot.library = Some(binding);
        }
        s.preview();
        let scene = wait(&mut s)?;
        if !scene.materials.warnings.is_empty() {
            return Err(format!("Material warnings: {:?}", scene.materials.warnings).into());
        }
        if scene.mesh.as_ref().unwrap().components != initial.mesh.as_ref().unwrap().components
            || scene.track.samples != initial.track.samples
            || s.editor.project() != &project
        {
            return Err("Selection/material preview changed authored data or geometry".into());
        }
        let plain = format!("{name}-plain");
        capture(&mut s, &scene, &root, &plain, false, &[])?;
        capture(&mut s, &scene, &root, name, true, &[])?;
        checks.push(compare(&root, &plain, name, true)?);
        if name == "gloss-red" {
            // Exercise real panel states, including the former tint trigger.
            for (tab, appearance, label) in [
                (1, false, "properties"),
                (0, false, "section"),
                (2, false, "reference"),
                (3, false, "project"),
            ] {
                s.track_tab = tab;
                s.material_ui.appearance_tab = appearance;
                let file = format!("gloss-red-{label}");
                capture(&mut s, &scene, &root, &file, true, &[])?;
                checks.push(compare(&root, name, &file, false)?);
            }
            // Train uses a different viewport size: compare highlight on/off
            // within that workspace rather than claim an identical Track camera.
            s.workbench.active = Profile::Train;
            capture(&mut s, &scene, &root, "train-plain", false, &[])?;
            capture(&mut s, &scene, &root, "train-selected", true, &[])?;
            checks.push(compare(&root, "train-plain", "train-selected", true)?);
            s.workbench.active = Profile::Track;
            s.track_tab = 1;
            s.material_ui.appearance_tab = true;
        }
        // Imported materials use the independent GLB shader. Place the same
        // material on an actual imported mesh overlapping the selected section:
        // both it and unselected track must be protected from the outline.
        if matches!(name, "gloss-red" | "polished-red" | "textured-red") {
            let mut model = qx_rendering::asset::import_glb(include_bytes!(
                "../../../../assets/development/train/development-body.glb"
            ))?;
            for i in 0..model.primitives.len() {
                qx_rendering::material_library::assign_imported(
                    &mut model,
                    i,
                    &scene.materials.slots["running"],
                )?;
            }
            let imported = [qx_rendering::asset::AssetInstance {
                model: Arc::new(model),
                component_id: "Imported GLB material/occlusion fixture".into(),
                transform: [
                    [1., 0., 0., 0.],
                    [0., 1., 0., 0.],
                    [0., 0., 1., 0.],
                    [1.5, 0., 0., 1.],
                ],
            }];
            let a = format!("{name}-glb-plain");
            let b = format!("{name}-glb-selected");
            capture(&mut s, &scene, &root, &a, false, &imported)?;
            capture(&mut s, &scene, &root, &b, true, &imported)?;
            checks.push(compare(&root, &a, &b, true)?);
        }
    }
    std::fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "actual_editor":true,"desktop_control_used":false,"source_geometry_unchanged":true,
            "checks":checks,"glb_scope":"Imported material/occlusion fixtures; production GLB track construction remains unsupported by the existing geometry generator."
        }))?,
    )?;
    println!("Selection evidence verified: {}", root.display());
    Ok(())
}
