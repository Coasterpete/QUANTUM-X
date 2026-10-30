use std::{path::PathBuf, process::Command};
// These tests share the executable under test. Concurrent CreateProcess/pipe
// startup stalled on native Windows; serialize workflows without skipping checks.
static CLI_WORKFLOW: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn identity_and_library_cli_report_planned_content_and_reject_invalid_indexes() {
    let _workflow = CLI_WORKFLOW.lock().unwrap();
    let binary = env!("CARGO_BIN_EXE_quantum-x");
    let version = Command::new(binary).arg("--version").output().unwrap();
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(
        version.contains("QUANTUM Dynamics")
            && version.contains("QUANTUM-X")
            && version.contains("quantum-x")
    );
    let report = Command::new(binary).arg("--self-check").output().unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["product"], "QUANTUM Dynamics");
    assert_eq!(report["track_families"], 6);
    assert_eq!(report["train_families"], 14);
    let entries = report["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 28);
    assert!(
        entries
            .iter()
            .all(|e| e["status"] == "planned" && e["content_ready"] == false)
    );
    let root = temporary();
    std::fs::create_dir_all(root.join("catalog")).unwrap();
    std::fs::write(root.join("catalog/styles.json"), "{\"version\":999}").unwrap();
    let bad = Command::new(binary)
        .arg("library")
        .arg(&root)
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("QUANTUM Dynamics"));
    std::fs::remove_dir_all(root).unwrap();
}
fn temporary() -> PathBuf {
    std::env::temp_dir().join(format!(
        "qx-demo-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
#[test]
fn diagnostic_executable_exports_and_reloads_real_project() {
    let _workflow = CLI_WORKFLOW.lock().unwrap();
    let dir = temporary();
    let demo = dir.join("demo");
    let regenerated = dir.join("regenerated");
    let first = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("demo")
        .arg(&demo)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    for name in [
        "project.qx.json",
        "centerline.csv",
        "measurements.csv",
        "transitions.csv",
        "reference-points.csv",
        "reference-info.txt",
        "centerline.obj",
        "measurements.svg",
        "transitions.svg",
        "trajectory.svg",
        "summary.txt",
    ] {
        assert!(
            std::fs::metadata(demo.join(name)).unwrap().len() > 80,
            "{name}"
        );
    }
    let second = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("generate")
        .arg(demo.join("project.qx.json"))
        .arg(&regenerated)
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    for name in [
        "centerline.csv",
        "measurements.csv",
        "transitions.csv",
        "centerline.obj",
        "project.qx.json",
    ] {
        assert_eq!(
            std::fs::read(demo.join(name)).unwrap(),
            std::fs::read(regenerated.join(name)).unwrap()
        );
    }
    let csv = std::fs::read_to_string(demo.join("centerline.csv")).unwrap();
    let last: Vec<_> = csv
        .lines()
        .last()
        .unwrap()
        .split(',')
        .map(|s| s.trim_matches('"').parse::<f64>().unwrap())
        .collect();
    assert!((last[0] - 152.0).abs() < 1e-9);
    assert!(last[3].abs() > 1.0 && last[4].abs() > 1.0);
    let bad = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("generate")
        .arg(demo.join("project.qx.json"))
        .arg(dir.join("invalid"))
        .arg("NaN")
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(!dir.join("invalid").exists());
    // A refinement diagnostic must not prevent a valid requested-quality solve.
    let catalog = qx_style::Catalog::built_in().unwrap();
    let mut tiny = qx_document::sample::foundation_project();
    tiny.sections = vec![qx_geometry::authoring::Section::straight(
        1, "Tiny", 0.00001,
    )];
    qx_document::persistence::save(&dir.join("tiny.json"), &tiny, &catalog).unwrap();
    let minimum = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("generate")
        .arg(dir.join("tiny.json"))
        .arg(dir.join("tiny"))
        .arg("0.000001")
        .output()
        .unwrap();
    assert!(
        minimum.status.success(),
        "{}",
        String::from_utf8_lossy(&minimum.stderr)
    );
    assert!(
        std::fs::read_to_string(dir.join("tiny/summary.txt"))
            .unwrap()
            .contains("comparison unavailable")
    );
    // Test owns this uniquely named temporary directory.
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn heartline_cli_roundtrip_retains_nominal_offset_and_independent_reference_geometry() {
    let _workflow = CLI_WORKFLOW.lock().unwrap();
    let dir = temporary();
    let first = dir.join("heartline");
    let second = dir.join("reopened");
    let output = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .args(["heartline-demo"])
        .arg(&first)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("generate")
        .arg(first.join("project.qx.json"))
        .arg(&second)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in [
        "reference-points.csv",
        "reference-info.txt",
        "project.qx.json",
    ] {
        assert_eq!(
            std::fs::read(first.join(name)).unwrap(),
            std::fs::read(second.join(name)).unwrap()
        );
    }
    let catalog = qx_style::Catalog::built_in().unwrap();
    let project = qx_document::persistence::load(&first.join("project.qx.json"), &catalog).unwrap();
    let source = project
        .generate(&catalog, qx_geometry::solver::GenerationOptions::default())
        .unwrap();
    let reference = project.reference_geometry(&source).unwrap();
    assert!((source.length_m() - 20. * std::f64::consts::TAU).abs() < 1e-8);
    assert!((reference.length_m - 18. * std::f64::consts::TAU).abs() < 1e-6);
    assert!((reference.points[0].speed_mps - 18.).abs() < 1e-7);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mixed_examples_export_actual_splines_and_precise_cli_edit_regenerates() {
    let _workflow = CLI_WORKFLOW.lock().unwrap();
    let dir = temporary();
    let result = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("examples")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    for name in [
        "spline-comparison",
        "mixed-authoring",
        "numerical-reference",
    ] {
        let out = dir.join(name);
        let project = out.join("project.qx.json");
        let catalog = qx_style::Catalog::built_in().unwrap();
        let p = qx_document::persistence::load(&project, &catalog).unwrap();
        let t = p
            .generate(&catalog, qx_geometry::solver::GenerationOptions::default())
            .unwrap();
        let csv = std::fs::read_to_string(out.join("centerline.csv")).unwrap();
        assert_eq!(csv.lines().count(), t.samples.len() + 1);
        for file in [
            "sections.csv",
            "numerical.csv",
            "connections.csv",
            "warnings.csv",
        ] {
            assert!(std::fs::metadata(out.join(file)).unwrap().len() > 50);
        }
        let methods = std::fs::read_to_string(out.join("sections.csv")).unwrap();
        for method in ["bezier", "b_spline", "nurbs", "catmull_rom"] {
            assert!(methods.contains(method));
        }
        for svg in ["trajectory.svg", "measurements.svg", "transitions.svg"] {
            let text = std::fs::read_to_string(out.join(svg)).unwrap();
            assert!(text.contains("<svg"));
            assert!(!text.contains("NaN"));
        }
    }
    let original = dir.join("spline-comparison/project.qx.json");
    let edited = dir.join("edited.qx.json");
    let result = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("move-point")
        .arg(&original)
        .arg(&edited)
        .args(["101", "4", "22.125", "8.25", "4.5"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = Command::new(env!("CARGO_BIN_EXE_quantum-x"))
        .arg("generate")
        .arg(&edited)
        .arg(dir.join("edited"))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_ne!(
        std::fs::read(dir.join("spline-comparison/centerline.csv")).unwrap(),
        std::fs::read(dir.join("edited/centerline.csv")).unwrap()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
