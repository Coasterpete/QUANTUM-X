//! Reproducible offscreen M7 evidence. This does not interact with Windows UI.
use super::{
    scenario,
    session::{Session, scene},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    construction::TrackMesh,
};
use qx_rendering::{DisplayMode, DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use qx_style::construction::{Role, TrackConstruction};
use std::{
    error::Error,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};
pub(super) struct Capture<'a> {
    pub session: &'a mut Session,
    pub display: DisplayOptions,
}
impl EditorSurface for Capture<'_> {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let out = self.session.ui(ctx, scene, display);
        let selected = display.selected;
        *display = self.display.clone();
        display.selected = selected;
        out
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        None
    }
    fn open_path(&mut self, _: &Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
pub(super) fn export_obj(mesh: &TrackMesh, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    let mut base = 1;
    writeln!(
        out,
        "# M7 provisional engine validation meshes; SI metres, world X/Y/Z"
    )?;
    for c in &mesh.components {
        let count = if c.role == Role::Hardware {
            c.placements.len()
        } else {
            1
        };
        if !c.visible {
            continue;
        }
        for i in 0..count {
            writeln!(out, "g {}-{i}", c.id)?;
            for v in &c.mesh.vertices {
                let p = if c.role == Role::Hardware {
                    c.placements[i].position_m + c.placements[i].frame.local_to_world(v.position_m)
                } else {
                    v.position_m
                };
                writeln!(out, "v {:.12} {:.12} {:.12}", p.x, p.y, p.z)?;
            }
            for t in c.mesh.indices.as_chunks::<3>().0.iter() {
                writeln!(out, "f {} {} {}", base + t[0], base + t[1], base + t[2])?;
            }
            base += c.mesh.vertices.len() as u32;
        }
    }
    Ok(())
}
pub(super) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&output)?;
    let built = scenario::build()?;
    let original = built.editor.project().clone();
    let catalog = qx_style::Catalog::built_in()?;
    let mut records = vec![];
    for case in [
        "a-centerline",
        "b-dual-rails",
        "c-dual-spine",
        "d-dual-hardware",
        "e-single-beam",
        "f-inverted-loop",
        "g-edited-force-roll",
        "h-technical",
        "h-wireframe",
        "i-dual-detail",
        "j-single-detail",
        "k-banked-dual",
        "k-banked-single",
        "f-inverted-single",
    ] {
        let mut p = original.clone();
        p.metadata.name = format!("M7 validation / {case}");
        p.construction = if [
            "e-single-beam",
            "j-single-detail",
            "k-banked-single",
            "f-inverted-single",
        ]
        .contains(&case)
        {
            TrackConstruction::generic_single()
        } else {
            TrackConstruction::generic_dual()
        };
        if case == "b-dual-rails" {
            p.construction
                .continuous
                .iter_mut()
                .filter(|c| c.role == Role::Structure)
                .for_each(|c| c.visible = false);
        }
        if ["b-dual-rails", "c-dual-spine"].contains(&case) {
            p.construction
                .repeated
                .iter_mut()
                .for_each(|c| c.visible = false);
        }
        if ["i-dual-detail", "j-single-detail"].contains(&case) {
            p.sections = vec![Section::straight(1, "10 m construction detail", 10.)];
        }
        if ["k-banked-dual", "k-banked-single"].contains(&case) {
            p.sections[1] = qx_document::region_edit::endpoint(
                &p.sections[1],
                qx_geometry::authoring::ChannelKey::Roll,
                0,
                true,
                0.04,
            )?;
        }
        if ["f-inverted-loop", "f-inverted-single"].contains(&case) {
            let mut section =
                Section::straight(1, "Synthetic vertical loop", std::f64::consts::TAU * 10.);
            section.kind = SectionKind::Arc {
                radius_m: 10.,
                plane_angle_rad: 0.,
            };
            p.sections = vec![section];
        }
        if case == "g-edited-force-roll" {
            for (key, value) in [
                (qx_geometry::authoring::ChannelKey::Normal, 2.3),
                (qx_geometry::authoring::ChannelKey::Roll, 0.018),
            ] {
                p.sections[1] =
                    qx_document::region_edit::endpoint(&p.sections[1], key, 0, true, value)?;
            }
        }
        qx_document::persistence::save(&output.join(format!("{case}.qx.json")), &p, &catalog)?;
        let capture_scene = scene(&p, &catalog, Default::default())?;
        let mesh = capture_scene.mesh.as_ref().unwrap();
        export_obj(mesh, &output.join(format!("{case}.obj")))?;
        let mode = match case {
            "a-centerline" | "h-technical" => DisplayMode::Technical,
            "h-wireframe" => DisplayMode::Wireframe,
            _ => DisplayMode::Rendered,
        };
        let visible_triangles: usize = if mode == DisplayMode::Technical {
            0
        } else {
            mesh.components
                .iter()
                .filter(|c| c.visible)
                .map(|c| {
                    c.mesh.indices.len() / 3
                        * if c.role == Role::Hardware {
                            c.placements.len()
                        } else {
                            1
                        }
                })
                .sum()
        };
        let mut renders = vec![];
        for (width, height) in [(1440, 960), (960, 640)] {
            let mut session = Session::new(p.clone(), None, catalog.clone())?;
            let mut capture = Capture {
                session: &mut session,
                display: DisplayOptions {
                    mode,
                    centerline: case == "a-centerline" || case == "h-technical",
                    frames: case.starts_with("f-inverted") || case.starts_with("k-banked"),
                    grid: true,
                    boundaries: case != "a-centerline",
                    ..Default::default()
                },
            };
            capture.session.select(p.sections.last().unwrap().id);
            capture.session.select_channel(if p.sections.len() > 1 {
                qx_geometry::authoring::ChannelKey::Normal
            } else {
                qx_geometry::authoring::ChannelKey::Roll
            });
            let file = format!("{case}-{width}.png");
            let result = qx_rendering::render_offscreen(
                &capture_scene,
                &mut capture,
                &output.join(&file),
                width,
                height,
            )?;
            println!("{case} {width}: {result}");
            renders
                .push(serde_json::json!({"file":file,"width":width,"height":height,"gpu":result}));
        }
        let components:Vec<_>=mesh.components.iter().map(|c|serde_json::json!({"id":c.id,"role":format!("{:?}",c.role),"visible":c.visible,"material":c.material,"vertices":c.mesh.vertices.len(),"triangles":c.mesh.indices.len()/3,"stations":c.placements.iter().map(|p|p.station_m).collect::<Vec<_>>(),"placement_frames":c.placements.iter().map(|p|[p.frame.normal().x,p.frame.normal().y,p.frame.normal().z]).collect::<Vec<_>>()})).collect();
        records.push(serde_json::json!({"case":case,"mode":mode.label(),"visible_triangles":visible_triangles,"samples":capture_scene.track.samples.len(),"length_m":capture_scene.track.length_m(),"solver_ms":capture_scene.solver_ms,"mesh_ms":capture_scene.mesh_ms,"vertices":mesh.stats.vertices,"triangles":mesh.stats.triangles,"rendered_triangles":mesh.stats.rendered_triangles,"cpu_payload_bytes":mesh.stats.bytes,"components":components,"captures":renders}));
    }
    let live_edit = super::m7_live::run(&original, &catalog, &output)?;
    // Measured CPU work, independently from rendering/driver initialization.
    let mut benchmarks = vec![];
    for length in [90., 1000.] {
        for config in [
            TrackConstruction::generic_dual(),
            TrackConstruction::generic_single(),
        ] {
            let mut p = original.clone();
            p.sections = vec![Section::straight(1, "Benchmark straight", length)];
            let solver = Instant::now();
            let track = p.generate(&catalog, Default::default())?;
            let solver_ms = solver.elapsed().as_secs_f64() * 1000.;
            let start = Instant::now();
            let mesh = qx_geometry::construction::build(&track, &config, Default::default())?;
            benchmarks.push(serde_json::json!({"configuration":config.id,"length_m":length,"samples":track.samples.len(),"solver_ms":solver_ms,"mesh_ms":start.elapsed().as_secs_f64()*1000.,"vertices":mesh.stats.vertices,"stored_triangles":mesh.stats.triangles,"drawn_triangles":mesh.stats.rendered_triangles,"cpu_payload_bytes":mesh.stats.bytes}));
        }
    }
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_string_pretty(
            &serde_json::json!({"build":if cfg!(debug_assertions){"debug"}else{"release"},"automated_offscreen":true,"manual_desktop_tested":false,"fixtures":records,"live_edit":live_edit,"benchmarks":benchmarks}),
        )?,
    )?;
    println!("M7 evidence: {}", output.display());
    Ok(())
}
