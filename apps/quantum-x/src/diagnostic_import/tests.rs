use super::*;
use qx_rendering::{DisplayOptions, EditorSurface};
fn inputs() -> Inputs {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/qx-geometry/tests/import-fixtures");
    Inputs {
        csv: root.join("terrain.csv"),
        xml: Some(root.join("terrain-supports.xml")),
        construction: "generic-dual".into(),
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
