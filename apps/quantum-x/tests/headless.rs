use std::{path::PathBuf, process::Command};
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
