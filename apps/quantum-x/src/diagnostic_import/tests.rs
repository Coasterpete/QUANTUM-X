use super::*;
use qx_rendering::{DisplayOptions, EditorSurface};
fn inputs() -> Inputs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/qx-geometry/tests/import-fixtures");
    Inputs {
        csv: root.join("terrain.csv"),
        xml: Some(root.join("terrain-supports.xml")),
        construction: "generic-dual".into(),
        construction_map: None,
    }
}
fn construction_inputs() -> Inputs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/qx-geometry/tests/import-fixtures");
    Inputs {
        csv: root.join("construction.csv"),
        xml: Some(root.join("construction-supports.xml")),
        construction: "box-spined-100".into(),
        construction_map: Some(root.join("construction-map.json")),
    }
}
fn compare_construction(inputs: Inputs) {
    let csv = std::fs::read(&inputs.csv).unwrap();
    let xml = std::fs::read(inputs.xml.as_ref().unwrap()).unwrap();
    let mut baseline_inputs = inputs.clone();
    baseline_inputs.construction_map = None;
    let before = load(&baseline_inputs).unwrap();
    let after = load(&inputs).unwrap();
    assert_eq!(before.track, after.track);
    assert_eq!(before.scene.track, after.scene.track);
    assert_eq!(before.supports, after.supports);
    assert_eq!(
        serde_json::to_value(&before.alignment).unwrap(),
        serde_json::to_value(&after.alignment).unwrap()
    );
    let a = before.scene.mesh.as_ref().unwrap();
    let b = after.scene.mesh.as_ref().unwrap();
    let keep = |m: &construction::TrackMesh| {
        m.components
            .iter()
            .filter(|c| {
                c.role == qx_style::construction::Role::Running || c.id.starts_with("support/")
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keep(a),
        keep(b),
        "rails and imported support meshes must be identical in every attribute"
    );
    assert_eq!(
        a.components
            .iter()
            .filter(|c| c.role == qx_style::construction::Role::Running)
            .count(),
        2
    );
    assert!(after.construction_report.is_some());
    assert!(!after.boundary_markers.is_empty());
    for c in &b.components {
        c.mesh.validate().unwrap();
    }
    let again = load(&inputs).unwrap();
    assert_eq!(after.scene.mesh, again.scene.mesh);
    assert_eq!(std::fs::read(&inputs.csv).unwrap(), csv);
    assert_eq!(std::fs::read(inputs.xml.as_ref().unwrap()).unwrap(), xml);
    println!(
        "M1.3 comparison: track samples={}, position/frame/rail/support attribute deltas=0; baseline vertices={}, bytes={}, total import+mesh ms={}; segmented vertices={}, bytes={}, total import+mesh ms={}; repeat ms={}",
        after.track.samples.len(),
        a.stats.vertices,
        a.stats.bytes,
        before.report["import_and_mesh_ms"],
        b.stats.vertices,
        b.stats.bytes,
        after.report["import_and_mesh_ms"],
        again.report["import_and_mesh_ms"]
    );
}
#[test]
fn segmented_source_assets_imported_supports_and_geometry_invariants() {
    compare_construction(construction_inputs());
}

#[test]
fn optional_private_segmented_brobdingnagian() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/nl2/BROBDINGNAGIAN");
    let csv = root.join("BROBDINGNAGIAN.csv");
    let xml = root.join("BROBDINGNAGIAN_supports.xml");
    if !csv.is_file() || !xml.is_file() {
        eprintln!("SKIP: private BROBDINGNAGIAN fixture unavailable");
        return;
    }
    let track = import::parse_csv(&import::read_text(&csv).unwrap()).unwrap();
    let cuts = [
        0.,
        250.25,
        700.75,
        1250.25,
        1900.75,
        2400.25,
        track.construction_track().length_m(),
    ];
    let segments=cuts.windows(2).enumerate().map(|(i,w)|serde_json::json!({"id":format!("local-{i}"),"start_m":w[0],"end_m":w[1],"style_id":"box-spined-100","variant_id":if i%2==0 {"source-single"} else {"diagnostic-double"}})).collect::<Vec<_>>();
    let map = std::env::temp_dir().join(format!(
        "qx-m13-map-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&map)
        .unwrap();
    std::io::Write::write_all(
        &mut file,
        &serde_json::to_vec(&serde_json::json!({"version":1,"segments":segments})).unwrap(),
    )
    .unwrap();
    drop(file);
    compare_construction(Inputs {
        csv,
        xml: Some(xml),
        construction: "box-spined-100".into(),
        construction_map: Some(map.clone()),
    });
    std::fs::remove_file(map).unwrap();
}

#[test]
fn offscreen_segmented_comparisons_when_requested() {
    let Some(root) = std::env::var_os("QX_SEGMENT_RENDER_TEST_DIR") else {
        eprintln!("SKIP: set QX_SEGMENT_RENDER_TEST_DIR for offscreen segmented evidence");
        return;
    };
    let inputs = construction_inputs();
    let root = PathBuf::from(root);
    run(&[
        inputs.csv.into_os_string(),
        inputs.xml.unwrap().into_os_string(),
        "--construction-map".into(),
        inputs.construction_map.unwrap().into_os_string(),
        "--render".into(),
        root.clone().into_os_string(),
    ])
    .unwrap();
    for label in ["boundary", "double"] {
        let before = image::open(root.join(format!("before-{label}.png")))
            .unwrap()
            .to_rgb8();
        let after = image::open(root.join(format!("after-{label}.png")))
            .unwrap()
            .to_rgb8();
        let changes = before
            .enumerate_pixels()
            .filter(|(x, y, p)| {
                *x > 400 && *y > 40 && *y < 1050 && p.0 != after.get_pixel(*x, *y).0
            })
            .count();
        assert!(
            changes > 100,
            "actual structural geometry should change viewport pixels; {label}: {changes}"
        );
    }
}
#[test]
fn repeated_import_cleanup_and_native_document_preservation() {
    let catalog = qx_style::Catalog::built_in().unwrap();
    let project = qx_document::sample::foundation_project();
    let bytes = qx_document::persistence::to_json(&project, &catalog).unwrap();
    let source_csv = std::fs::read(inputs().csv).unwrap();
    let source_xml = std::fs::read(inputs().xml.unwrap()).unwrap();
    let first = load(&inputs()).unwrap();
    let original_mesh = first.scene.mesh.clone().unwrap();
    let mut surface = surface::DiagnosticSurface::new(inputs(), first);
    // Execute actual Clear logic and verify scene resource ownership is released.
    surface.clear();
    let empty = surface.poll_scene().unwrap();
    assert!(surface.loaded.is_none());
    assert!(empty.mesh.is_none());
    assert!(empty.track.samples.is_empty());
    assert_eq!(Arc::strong_count(&original_mesh), 1);
    for _ in 0..3 {
        let l = load(&inputs()).unwrap();
        assert_eq!(l.scene.mesh.as_deref().unwrap(), original_mesh.as_ref());
        assert!(l.scene.track.sections.is_empty());
        assert!(l.scene.track.graphs.is_empty());
    }
    assert_eq!(
        qx_document::persistence::to_json(&project, &catalog).unwrap(),
        bytes
    );
    assert_eq!(std::fs::read(inputs().csv).unwrap(), source_csv);
    assert_eq!(std::fs::read(inputs().xml.unwrap()).unwrap(), source_xml);
}
#[test]
fn actual_native_export_is_accepted_without_mutating_project() {
    let catalog = qx_style::Catalog::built_in().unwrap();
    let project = qx_document::sample::foundation_project();
    let generated = project.generate(&catalog, Default::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("qx-m12-native-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    super::super::export::write_all(
        &dir,
        &project,
        &generated,
        0.1,
        &super::super::export::Refinement::Unavailable("import compatibility test".into()),
    )
    .unwrap();
    let parsed =
        import::parse_csv(&import::read_text(&dir.join("centerline.csv")).unwrap()).unwrap();
    assert_eq!(parsed.samples.len(), generated.samples.len());
    for (a, b) in parsed.samples.iter().zip(&generated.samples) {
        assert_eq!(a.position_m, b.position_m);
        assert_eq!(a.distance_m, b.distance_m);
        assert!((a.frame.normal() - b.frame.normal()).length() < 1e-13);
    }
    // Known task-owned directory, verified inside the OS temp root before cleanup.
    let resolved = dir.canonicalize().unwrap();
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert!(resolved.starts_with(temp));
    std::fs::remove_dir_all(resolved).unwrap();
}
#[test]
fn offscreen_import_uses_existing_renderer_when_requested() {
    let Some(root) = std::env::var_os("QX_IMPORT_RENDER_TEST_DIR") else {
        eprintln!("SKIP: set QX_IMPORT_RENDER_TEST_DIR to run GPU import evidence");
        return;
    };
    let root = PathBuf::from(root);
    std::fs::create_dir(&root).unwrap();
    let loaded = load(&inputs()).unwrap();
    let scene = loaded.scene.clone();
    let mut surface = surface::DiagnosticSurface::new(inputs(), loaded);
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    let mut output = ctx.run_pass(Default::default(), |ctx| {
        surface.ui(ctx, &scene, &mut display);
    });
    // This pass inspects UI state only. No GPU consumer owns its font delta.
    output.textures_delta.clear();
    assert!(!display.overlay_lines.is_empty());
    assert!(!display.grid);
    let report = qx_rendering::render_offscreen(
        &scene,
        &mut surface,
        &root.join("synthetic-import.png"),
        1000,
        800,
    )
    .unwrap();
    let image = image::open(root.join("synthetic-import.png"))
        .unwrap()
        .to_rgb8();
    assert_eq!(image.dimensions(), (1000, 800));
    // Inspect viewport pixels, excluding the developer panel. A blank render fails.
    let pixels = image
        .enumerate_pixels()
        .filter(|(x, y, _)| *x > 370 && *y > 40 && *y < 760)
        .map(|(_, _, p)| p.0)
        .collect::<Vec<_>>();
    let distinct = pixels
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert!(
        distinct > 150,
        "imported geometry should produce a nonblank shaded viewport; colors={distinct}"
    );
    std::fs::write(root.join("render.txt"), report).unwrap();
}
