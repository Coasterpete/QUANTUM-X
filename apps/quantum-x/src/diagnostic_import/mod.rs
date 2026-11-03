//! Replaceable development inspection, independent of all native editor state.
mod construction_map;
mod surface;
#[cfg(test)]
mod tests;
use qx_geometry::{
    construction,
    diagnostic_import::{self as import, AlignmentReport, ImportedSupports, ImportedTrack},
};
use qx_rendering::{InspectionScene, OverlayLine};
use qx_style::{
    construction::TrackConstruction,
    library::{Library, LibraryKind},
};
use std::{error::Error, ffi::OsString, path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone)]
struct Inputs {
    csv: PathBuf,
    xml: Option<PathBuf>,
    construction: String,
    construction_map: Option<PathBuf>,
}
struct Loaded {
    scene: InspectionScene,
    track: ImportedTrack,
    supports: Option<ImportedSupports>,
    alignment: Option<AlignmentReport>,
    report: serde_json::Value,
    markers: Vec<OverlayLine>,
    alignment_lines: Vec<OverlayLine>,
    materials: Vec<String>,
    construction_report: Option<serde_json::Value>,
    boundary_markers: Vec<OverlayLine>,
    focus_station_m: Option<f64>,
}
fn load(inputs: &Inputs) -> Result<Loaded, String> {
    let result = || -> Result<Loaded, Box<dyn Error>> {
        let start = Instant::now();
        let track = import::parse_csv(&import::read_text(&inputs.csv)?)?;
        let supports = inputs
            .xml
            .as_ref()
            .map(|p| import::read_text(p).and_then(|text| import::parse_support_xml(&text)))
            .transpose()?;
        let alignment = supports
            .as_ref()
            .map(|s| import::alignment(&track, s))
            .transpose()?;
        let root = super::authoring::library_ui::assets_root();
        let definition = match inputs.construction.as_str() {
            "generic-dual" => TrackConstruction::generic_dual(),
            "box-spined-100" => {
                let library = Library::open(&root)?;
                library
                    .entry(LibraryKind::Track, "box-spined-100")
                    .and_then(|e| e.manifest.as_ref())
                    .and_then(|m| m.construction.clone())
                    .ok_or("box-spined-100 construction is unavailable")?
            }
            _ => return Err("construction must be generic-dual or box-spined-100".into()),
        };
        let generated = track.construction_track();
        let mapped = inputs
            .construction_map
            .as_ref()
            .map(|path| construction_map::build(path, &generated, &definition, &root))
            .transpose()?;
        let (mut mesh, construction_report, boundary_markers, focus_station_m) =
            if let Some(mapped) = mapped {
                (
                    mapped.mesh,
                    Some(mapped.report),
                    mapped.markers,
                    Some(mapped.focus_station_m),
                )
            } else {
                (
                    construction::build_with_assets(
                        &generated,
                        &definition,
                        Default::default(),
                        &qx_rendering::construction_assets::Sources::new(&root),
                    )?,
                    None,
                    vec![],
                    None,
                )
            };
        if let Some(s) = &supports {
            s.append_mesh(&mut mesh)?;
        }
        let material_library =
            qx_rendering::material_library::read_manifest(&root.join("materials"));
        let available = material_library
            .as_ref()
            .map(|l| l.materials.iter().map(|m| m.id.clone()).collect::<Vec<_>>())
            .unwrap_or_default();
        let materials =
            qx_rendering::material_library::load_bindings(&root.join("materials"), &mesh.materials);
        let mut markers = vec![];
        if let Some(s) = &supports {
            for n in s
                .nodes
                .values()
                .filter(|n| n.kind == "footer" || n.kind == "rail_connector")
            {
                let color = if n.kind == "footer" {
                    [1., 0.72, 0.28]
                } else {
                    [0.4, 1., 0.65]
                };
                for axis in [
                    qx_geometry::math::Vec3::X,
                    qx_geometry::math::Vec3::Y,
                    qx_geometry::math::Vec3::Z,
                ] {
                    markers.push(OverlayLine {
                        start_m: n.position_m - axis * 0.5,
                        end_m: n.position_m + axis * 0.5,
                        color,
                    });
                }
            }
        }
        let alignment_lines = alignment
            .as_ref()
            .map(|r| {
                r.residuals
                    .iter()
                    .map(|r| OverlayLine {
                        start_m: r.connector_position_m,
                        end_m: r.csv_nearest_position_m,
                        color: if r.track_index == 0 {
                            [1., 0.85, 0.3]
                        } else {
                            [1., 0.35, 0.35]
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        let support_stats=supports.as_ref().map(|s|serde_json::json!({
            "nodes":s.nodes.len(),"free_nodes":s.nodes.values().filter(|n|n.kind=="free").count(),
            "beam_nodes":s.nodes.values().filter(|n|n.kind=="beam_node").count(),
            "beams":s.beams.len(),"rendered_pipe_beams":s.beams.iter().filter(|b|b.diameter_m.is_some()).count(),
            "rail_connectors":s.connectors.len(),"footers":s.footers.len(),
            "bounds_m":s.bounds.map(|b|serde_json::json!({"min":b.min,"max":b.max})),
            "unsupported":s.unsupported,
            "beam_profile_note":"type 1 pipe, size1 outer diameter; closed 16-sided diagnostic solids; flange and connector meshes omitted",
            "foundation_note":"exact exported world positions retained; dimensions unavailable, markers only",
            "prefabs":s.prefabs.len(),"atomized_prefabs":s.prefabs.iter().filter(|p|p.atomized).count(),
        }));
        let report = serde_json::json!({
            "diagnostic_only":true,"coordinate_transform":"identity world XYZ metres; T=Front, N=Up, L=-Left; no support alignment transform",
            "format":format!("{:?}",track.format),"samples":track.samples.len(),
            "track_bounds_m":{"min":track.bounds.min,"max":track.bounds.max},
            "polyline_length_m":track.polyline_length_m,"station_extent_m":generated.length_m(),
            "endpoint_gap_m":track.endpoint_gap_m,"topology":"as exported; no added closing segment",
            "max_frame_axis_correction":track.max_frame_correction,
            "supports":support_stats,"alignment":alignment,
            "reference_limitation":"exported spline reference offset unspecified; attachment distances are not certified connection errors",
            "construction":inputs.construction,"construction_note":"existing QUANTUM construction, not an imported NL2 track-family mesh",
            "construction_segments":construction_report,
            "materials":mesh.materials,"available_materials":available,"material_warnings":materials.warnings,
            "material_library_error":material_library.err(),
            "mesh":{"components":mesh.components.len(),"vertices":mesh.stats.vertices,"triangles":mesh.stats.triangles,"rendered_triangles":mesh.stats.rendered_triangles,"instances":mesh.stats.instances,"bytes":mesh.stats.bytes,"bounds_m":{"min":mesh.bounds.min,"max":mesh.bounds.max}},
            "import_and_mesh_ms":start.elapsed().as_secs_f64()*1000.,
        });
        let mut scene = InspectionScene::empty();
        scene.name = "Read-only CSV/XML diagnostic — no Regions / no simulation".into();
        scene.track = generated;
        scene.mesh = Some(Arc::new(mesh));
        scene.materials = Arc::new(materials);
        scene.mesh_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(Loaded {
            scene,
            track,
            supports,
            alignment,
            report,
            markers,
            alignment_lines,
            materials: available,
            construction_report,
            boundary_markers,
            focus_station_m,
        })
    };
    result().map_err(|e| format!("Diagnostic import: {e}"))
}
pub fn run(args: &[OsString]) -> Result<(), Box<dyn Error>> {
    if args.len() < 2 {
        return Err("usage: diagnostic-import <track.csv> <supports.xml|-> [--render <new-directory>] [--construction <generic-dual|box-spined-100> | --construction-map <map.json>]".into());
    }
    let mut inputs = Inputs {
        csv: args[0].clone().into(),
        xml: if args[1] == "-" {
            None
        } else {
            Some(args[1].clone().into())
        },
        construction: "generic-dual".into(),
        construction_map: None,
    };
    let mut output = None;
    let (options, remainder) = args[2..].as_chunks::<2>();
    for pair in options {
        match pair[0].to_str() {
            Some("--render") if output.is_none() => output = Some(PathBuf::from(&pair[1])),
            Some("--construction") => {
                inputs.construction = pair[1].to_str().ok_or("construction must be UTF-8")?.into()
            }
            Some("--construction-map") if inputs.construction_map.is_none() => {
                inputs.construction_map = Some(PathBuf::from(&pair[1]));
            }
            _ => return Err("unknown or duplicate diagnostic import option".into()),
        }
    }
    if !remainder.is_empty() {
        return Err("diagnostic option requires a value".into());
    }
    if inputs.construction_map.is_some() {
        // The sidecar's explicit style/variant IDs are authoritative. The M1.3
        // diagnostic registry contains only the verified Box-Spined-100 pair.
        if args[2..].iter().any(|a| a == "--construction") {
            return Err("choose --construction OR --construction-map, not both".into());
        }
        inputs.construction = "box-spined-100".into();
    }
    let loaded = load(&inputs)?;
    println!("{}", serde_json::to_string_pretty(&loaded.report)?);
    let scene = loaded.scene.clone();
    let mut surface = surface::DiagnosticSurface::new(inputs.clone(), loaded);
    if let Some(output) = output {
        // Never overwrite a source file, an existing evidence directory or a project.
        std::fs::create_dir(&output)?;
        surface.comparison_camera = inputs.construction_map.is_some();
        let mut report = surface.loaded.as_ref().unwrap().report.clone();
        let overview = qx_rendering::render_offscreen(
            &scene,
            &mut surface,
            &output.join("overview.png"),
            1800,
            1200,
        )?;
        surface.show_alignment = true;
        let alignment = qx_rendering::render_offscreen(
            &scene,
            &mut surface,
            &output.join("alignment.png"),
            1800,
            1200,
        )?;
        // Focus near an actual connector, with no modifications to source geometry.
        surface.closeup = true;
        surface.structural_detail = inputs.construction_map.is_some();
        let detail = qx_rendering::render_offscreen(
            &scene,
            &mut surface,
            &output.join("detail.png"),
            1600,
            1100,
        )?;
        report["offscreen"] =
            serde_json::json!({"overview":overview,"alignment":alignment,"detail":detail});
        if inputs.construction_map.is_some() {
            let mut baseline_inputs = inputs.clone();
            baseline_inputs.construction_map = None;
            let baseline = load(&baseline_inputs)?;
            let baseline_scene = baseline.scene.clone();
            let mut before = surface::DiagnosticSurface::new(baseline_inputs, baseline);
            before.comparison_camera = true;
            let baseline_overview = qx_rendering::render_offscreen(
                &baseline_scene,
                &mut before,
                &output.join("before-overview.png"),
                1800,
                1200,
            )?;
            let boundary = surface
                .loaded
                .as_ref()
                .and_then(|l| l.focus_station_m)
                .unwrap_or(scene.track.length_m() * 0.5);
            let double_station = surface
                .loaded
                .as_ref()
                .and_then(|l| l.construction_report.as_ref())
                .and_then(|r| r["map"]["segments"].as_array())
                .and_then(|a| a.iter().find(|s| s["variant_id"] == "diagnostic-double"))
                .and_then(|s| Some((s["start_m"].as_f64()? + s["end_m"].as_f64()?) * 0.5))
                .unwrap_or(boundary);
            let mut comparisons = vec![];
            for (label, station) in [("boundary", boundary), ("double", double_station)] {
                before.closeup = true;
                before.structural_detail = true;
                before.evidence_station_m = Some(station);
                surface.closeup = true;
                surface.evidence_station_m = Some(station);
                surface.show_alignment = false;
                let a = qx_rendering::render_offscreen(
                    &baseline_scene,
                    &mut before,
                    &output.join(format!("before-{label}.png")),
                    1600,
                    1100,
                )?;
                let b = qx_rendering::render_offscreen(
                    &scene,
                    &mut surface,
                    &output.join(format!("after-{label}.png")),
                    1600,
                    1100,
                )?;
                comparisons.push(
                    serde_json::json!({"view":label,"station_m":station,"before":a,"after":b}),
                );
            }
            report["comparison"] = serde_json::json!({"baseline_overview":baseline_overview,"views":comparisons,"camera":"identical source bounds and source pose in before/after","baseline":before.loaded.as_ref().unwrap().report});
        }
        std::fs::write(
            output.join("report.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!("Evidence: {}", output.display());
        Ok(())
    } else {
        qx_rendering::edit(scene, surface).map_err(Into::into)
    }
}
