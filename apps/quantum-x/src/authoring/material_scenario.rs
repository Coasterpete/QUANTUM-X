//! Opt-in diagnostic captures through the existing editor renderer, no desktop input.
use super::session::scene;
use qx_geometry::{authoring::Section, math::Vec3};
use qx_rendering::{
    DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput,
    asset::{AssetInstance, Model},
};
use std::{error::Error, path::PathBuf, sync::Arc};

struct Preview {
    label: String,
    description: &'static str,
    assets: Vec<AssetInstance>,
    bounds: (Vec3, Vec3),
}
impl EditorSurface for Preview {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        _: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        *display = DisplayOptions {
            asset_instances: self.assets.clone(),
            mode: qx_rendering::DisplayMode::Rendered,
            grid: false,
            centerline: false,
            boundaries: false,
            highlights: false,
            inspection_marker: false,
            ..Default::default()
        };
        // Include the adjacent asset probes in the same camera fit.
        display.train_bounds = Some(self.bounds);
        let mut viewport = egui::Rect::NOTHING;
        ctx.root_ui(|ui| {
            ui.heading("M1 · TRACK MATERIAL DIAGNOSTIC");
            ui.label(&self.label);
            ui.label(self.description);
            viewport = ui.available_rect_before_wrap();
        });
        SurfaceOutput {
            viewport,
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
    fn open_path(&mut self, _: &std::path::Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
pub fn run(root: PathBuf) -> Result<(), Box<dyn Error>> {
    if root.exists() {
        return Err("Choose a new material evidence directory".into());
    }
    std::fs::create_dir_all(&root)?;
    let catalog = qx_style::Catalog::built_in()?;
    let library_root = super::library_ui::assets_root().join("materials");
    let library = qx_rendering::material_library::read_manifest(&library_root)?;
    let mut records = vec![];
    for definition in &library.materials {
        let mut project = qx_document::sample::foundation_project();
        project.metadata.name = format!("M1 diagnostic · {}", definition.id);
        project.sections = vec![Section::straight(1, "Material test only", 4.)];
        for slot in &mut project.construction.materials {
            slot.library = Some(qx_style::materials::Binding::new(&definition.id));
        }
        let captured = scene(&project, &catalog, Default::default())?;
        let material = captured
            .materials
            .slots
            .values()
            .next()
            .ok_or("Preview material failed to load; check warnings")?;
        let mut imported = qx_rendering::asset::import_glb(include_bytes!(
            "../../../../assets/development/train/development-body.glb"
        ))?;
        // Reuse only this fixture's unit-box primitive. Its train hierarchy is
        // deliberately excluded: track materials are previewed on a swatch cube.
        imported.nodes = vec![qx_rendering::asset::Node {
            primitive: 0,
            transform: glam_identity(),
        }];
        for i in 0..imported.primitives.len() {
            qx_rendering::material_library::assign_imported(&mut imported, i, material)?;
        }
        let imported = Arc::new(imported);
        let transform = |position: [f64; 3], scale: f64| {
            [
                [scale, 0., 0., 0.],
                [0., scale, 0., 0.],
                [0., 0., scale, 0.],
                [position[0], position[1], position[2], 1.],
            ]
        };
        let mut preview = Preview {
            label: format!("{} · {:?}", definition.name, captured.materials.warnings),
            description: "Existing generic rails/spine + GLB unit-box swatch + neutral matte train placeholder. Diagnostic only.",
            bounds: (Vec3::new(0., -0.7, -0.8), Vec3::new(4., 1.2, 3.9)),
            assets: vec![
                AssetInstance {
                    model: imported,
                    component_id: "GLB unit-box material swatch".into(),
                    transform: transform([2., 0.6, 2.], 1.2),
                },
                AssetInstance {
                    model: Arc::new(Model::development_box()),
                    component_id: "Neutral train placeholder".into(),
                    transform: transform([2., 0.5, 3.4], 1.),
                },
            ],
        };
        let file = format!("{}.png", definition.id);
        let report =
            qx_rendering::render_offscreen(&captured, &mut preview, &root.join(&file), 1440, 960)?;
        let mut cube = Model::development_box();
        cube.primitives[0].material = material.clone();
        preview.description =
            "Unit-cube material swatch only. This is neither track hardware nor a train model.";
        preview.assets = vec![AssetInstance {
            model: Arc::new(cube),
            component_id: "Material swatch cube · diagnostic only".into(),
            transform: transform([0.; 3], 1.),
        }];
        preview.bounds = (Vec3::new(-0.55, -0.55, -0.55), Vec3::new(0.55, 0.55, 0.55));
        let mut swatch = captured.clone();
        swatch.mesh = None;
        let closeup = format!("{}-closeup.png", definition.id);
        qx_rendering::render_offscreen(&swatch, &mut preview, &root.join(&closeup), 1000, 800)?;
        // Exercise real persistence and its existing construction undo transaction.
        let mut editor = qx_document::command::Editor::new(
            qx_document::sample::foundation_project(),
            catalog.clone(),
            8,
        )?;
        let before = editor.project().construction.clone();
        editor.execute([qx_document::command::Command::SetConstruction(
            project.construction.clone(),
        )])?;
        if !editor.undo() {
            return Err("Material undo unavailable".into());
        }
        if editor.project().construction != before {
            return Err("Material undo changed the original construction".into());
        }
        if !editor.redo() {
            return Err("Material redo unavailable".into());
        }
        let path = root.join(format!("{}.qx.json", definition.id));
        qx_document::persistence::save(&path, &project, &catalog)?;
        let reopened = qx_document::persistence::load(&path, &catalog)?;
        if reopened != project {
            return Err("Material assignment save/reopen mismatch".into());
        }
        records.push(serde_json::json!({"material":definition.id,"capture":file,"closeup":closeup,"warnings":captured.materials.warnings,"renderer":report,"save_reopen_undo_redo":true,
            "smoothness_map":definition.maps.smoothness,"roughness_map":definition.maps.roughness,"roughness_factor":definition.roughness,"scalar_map_color_space":"linear"}));
    }
    let probes = qx_rendering::verify_material_rendering(
        &scene(
            &qx_document::sample::foundation_project(),
            &catalog,
            Default::default(),
        )?,
        &root.join("probes"),
    )?;
    let mut surface = Preview {
        label: "Polished steel (roughness 0.07) and rough steel (0.85)".into(),
        description: "Identical metal/color, procedural sky reflections and sunlight. Material spheres are diagnostic swatches, not train assets. No scene reflections.",
        bounds: (Vec3::new(-1.6, -0.7, -0.7), Vec3::new(1.6, 0.7, 0.7)),
        assets: [(-1., 0.07), (1., 0.85)]
            .map(|(x, roughness)| qx_rendering::asset::AssetInstance {
                model: Arc::new(sphere(qx_rendering::asset::Material {
                    color: [0.65, 0.65, 0.65, 1.],
                    metallic: 1.,
                    roughness,
                    ..Default::default()
                })),
                component_id: format!("Material diagnostic sphere · roughness {roughness}"),
                transform: [
                    [1., 0., 0., 0.],
                    [0., 1., 0., 0.],
                    [0., 0., 1., 0.],
                    [x, 0., 0., 1.],
                ],
            })
            .to_vec(),
    };
    let mut reflection_scene = scene(
        &qx_document::sample::foundation_project(),
        &catalog,
        Default::default(),
    )?;
    reflection_scene.mesh = None;
    qx_rendering::render_offscreen(
        &reflection_scene,
        &mut surface,
        &root.join("polished-versus-rough.png"),
        1440,
        960,
    )?;
    std::fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"automated_offscreen":true,"manual_desktop_tested":false,"materials":records,"gpu_probes":probes}),
        )?,
    )?;
    println!("Material evidence: {}", root.display());
    Ok(())
}
fn sphere(material: qx_rendering::asset::Material) -> Model {
    let mut model = Model::development_box();
    let p = &mut model.primitives[0];
    p.material = material;
    p.vertices.clear();
    p.indices.clear();
    for row in 0..=48 {
        for col in 0..=96 {
            let theta = row as f32 * std::f32::consts::PI / 48.;
            let phi = col as f32 * std::f32::consts::TAU / 96.;
            let n = [
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ];
            p.vertices.push(qx_rendering::asset::Vertex {
                position: n.map(|v| v * 0.6),
                normal: n,
                uv: [col as f32 / 96., row as f32 / 48.],
                tangent: [0.; 4],
            });
        }
    }
    for row in 0..48 {
        for col in 0..96 {
            let a = row * 97 + col;
            let b = a + 97;
            p.indices.extend([a, a + 1, b + 1, a, b + 1, b]);
        }
    }
    p.bounds = [[-0.6; 3], [0.6; 3]];
    model
}
fn glam_identity() -> [[f64; 4]; 4] {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}
