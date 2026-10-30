use super::*;
use crate::{Catalog, DevelopmentStatus, compatibility::Compatibility};
use std::path::PathBuf;
fn temporary() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "qx-library-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn fixture(root: &std::path::Path) -> (Catalog, PathBuf) {
    let mut catalog = Catalog::built_in().unwrap();
    let style = catalog
        .styles
        .iter_mut()
        .find(|s| s.id == "hybrid-100")
        .unwrap();
    style.status = DevelopmentStatus::Available;
    let directory = root.join(style.directory.as_ref().unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    (catalog, directory)
}
fn manifest() -> ContentManifest {
    ContentManifest {
        version: 1,
        kind: LibraryKind::Track,
        style_id: "hybrid-100".into(),
        visuals: vec![],
        previews: PreviewImages {
            thumbnail: Some("thumb.png".into()),
            large: Some("preview.png".into()),
            references: vec![],
        },
        construction: Some(Default::default()),
        mechanical_definition: None,
    }
}
#[test]
fn authored_inventory_and_stable_ids_round_trip() {
    let c = Catalog::built_in().unwrap();
    assert_eq!(
        (
            c.families.len(),
            c.styles.len(),
            c.train_families.len(),
            c.train_styles.len()
        ),
        (6, 17, 14, 11)
    );
    for id in ["hybrid-100", "monorail-100", "monorail-200"] {
        assert!(c.styles.iter().any(|s| s.id == id));
    }
    for id in [
        "4d",
        "free-spin",
        "flying",
        "inverted",
        "motorcycle-suspended",
    ] {
        assert!(c.train_families.iter().any(|f| f.id == id));
    }
    assert_eq!(
        c.train_style("xtreme-spinning-floorless")
            .unwrap()
            .directory,
        "trains/spinning/xtreme-spinning-floorless"
    );
    assert_eq!(
        c.styles
            .iter()
            .find(|s| s.id == "hybrid-100")
            .unwrap()
            .directory
            .as_deref(),
        Some("tracks/hybrid/100(topper-track)")
    );
    assert_eq!(
        Catalog::from_json(&serde_json::to_string(&c).unwrap()).unwrap(),
        c
    );
    let root = temporary();
    let library = Library::from_catalog(c, &root).unwrap();
    assert!(library.entries.iter().all(|e| !e.content_ready));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn empty_and_missing_directories_never_install_content() {
    let root = temporary();
    let (catalog, dir) = fixture(&root);
    std::fs::write(dir.join(".gitkeep"), "").unwrap();
    let library = Library::from_catalog(catalog.clone(), &root).unwrap();
    assert!(
        !library
            .entry(LibraryKind::Track, "hybrid-100")
            .unwrap()
            .content_ready
    );
    assert!(
        !library
            .entry(LibraryKind::Track, "monorail-100")
            .unwrap()
            .content_ready
    );
    std::fs::write(
        dir.join("style.json"),
        serde_json::to_string(&manifest()).unwrap(),
    )
    .unwrap();
    assert!(
        !Library::from_catalog(catalog.clone(), &root)
            .unwrap()
            .entry(LibraryKind::Track, "hybrid-100")
            .unwrap()
            .content_ready
    );
    for file in ["thumb.png", "preview.png"] {
        std::fs::write(
            dir.join(file),
            b"test path resolution only, not a representative image",
        )
        .unwrap();
    }
    let library = Library::from_catalog(catalog.clone(), &root).unwrap();
    assert!(
        library
            .entry(LibraryKind::Track, "hybrid-100")
            .unwrap()
            .content_ready
    );
    assert!(
        library
            .asset(LibraryKind::Track, "hybrid-100", "preview.png")
            .unwrap()
            .is_file()
    );
    assert!(
        library
            .asset(LibraryKind::Track, "hybrid-100", ".gitkeep")
            .is_err()
    );
    let mut shuffled = catalog.clone();
    shuffled.styles.reverse();
    assert_eq!(
        serde_json::to_value(&library.entries).unwrap(),
        serde_json::to_value(&Library::from_catalog(shuffled, &root).unwrap().entries).unwrap()
    );
    std::fs::create_dir_all(root.join("catalog")).unwrap();
    std::fs::write(
        root.join("catalog/styles.json"),
        serde_json::to_string(&catalog).unwrap(),
    )
    .unwrap();
    assert_eq!(Library::open(&root).unwrap().catalog, catalog);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn rejects_invalid_manifests_ids_links_and_path_traversal() {
    let json = serde_json::to_value(manifest()).unwrap();
    for (key, value) in [
        ("version", serde_json::json!(99)),
        ("style_id", serde_json::json!("Bad ID")),
        ("unknown", serde_json::json!(true)),
        ("mechanical_definition", serde_json::json!("train.json")),
    ] {
        let mut invalid = json.clone();
        invalid[key] = value;
        assert!(ContentManifest::from_json(&invalid.to_string()).is_err());
    }
    let root = temporary();
    let (catalog, dir) = fixture(&root);
    let mut wrong = manifest();
    wrong.style_id = "monorail-100".into();
    std::fs::write(
        dir.join("style.json"),
        serde_json::to_string(&wrong).unwrap(),
    )
    .unwrap();
    assert!(
        !Library::from_catalog(catalog.clone(), &root)
            .unwrap()
            .entry(LibraryKind::Track, "hybrid-100")
            .unwrap()
            .content_ready
    );
    for path in [
        "../escape",
        "C:/escape",
        "/absolute",
        "a\\b",
        "a/./b",
        "",
        "a\nsecret",
        "NUL.png",
        "COM1.json",
        "file. ",
    ] {
        let mut invalid = manifest();
        invalid.previews.thumbnail = Some(path.into());
        assert!(invalid.validate().is_err(), "{path}");
        assert!(resolve_existing(&root, path).is_err());
    }
    let mut c = catalog.clone();
    c.train_styles[0]
        .compatibility
        .track_style_ids
        .push("missing".into());
    assert!(c.validate().is_err());
    let mut c = catalog.clone();
    c.train_styles[0].directory = "trains/other/style".into();
    assert!(c.validate().is_err());
    let mut c = catalog.clone();
    c.train_styles[0].id = "Invalid".into();
    assert!(c.validate().is_err());
    let mut c = catalog;
    c.styles[1].directory = c.styles[0].directory.clone();
    assert!(c.validate().is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[cfg(windows)]
#[test]
fn windows_junctions_cannot_escape_asset_root() {
    let root = temporary();
    let outside = temporary();
    let junction = root.join("escape");
    std::fs::write(outside.join("retained.txt"), "retain outside content").unwrap();
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:QX_TEST_LINK -Target $env:QX_TEST_TARGET | Out-Null"])
        .env("QX_TEST_LINK", &junction).env("QX_TEST_TARGET", &outside)
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(resolve_existing(&root, "escape/retained.txt").is_err());
    std::fs::remove_dir(&junction).unwrap();
    assert!(outside.join("retained.txt").is_file());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}
#[test]
fn compatibility_requires_declarations_and_numbers_imply_nothing() {
    let mut c = Catalog::built_in().unwrap();
    assert_eq!(
        c.compatibility("hybrid-100", "hybrid-100").unwrap(),
        Compatibility::Unspecified
    );
    assert!(c.compatibility("missing", "hybrid-100").is_err());
    c.train_styles
        .iter_mut()
        .find(|t| t.id == "hybrid-100")
        .unwrap()
        .compatibility
        .track_style_ids
        .push("monorail-200".into());
    assert_eq!(
        c.compatibility("monorail-200", "hybrid-100").unwrap(),
        Compatibility::Declared
    );
    assert!(matches!(
        c.compatibility("hybrid-100", "hybrid-100").unwrap(),
        Compatibility::Incompatible(_)
    ));
    c.styles
        .iter_mut()
        .find(|t| t.id == "monorail-200")
        .unwrap()
        .compatible_train_ids
        .push("inverted-100".into());
    c.validate().unwrap();
    assert!(matches!(
        c.compatibility("monorail-200", "hybrid-100").unwrap(),
        Compatibility::Incompatible(_)
    ));
}
#[test]
fn version_one_metadata_and_status_names_remain_readable() {
    let json = r#"{"version":1,"families":[{"id":"hybrid","name":"Hybrid","description":"Legacy"}],"train_ids":[],"styles":[{"id":"hybrid-100","family_id":"hybrid","number":100,"name":"Hybrid-100","description":"Legacy concept","compatible_train_ids":[],"track_asset":null,"thumbnail":null,"preview":null,"reference_images":[],"status":"concept"}]}"#;
    let c = Catalog::from_json(json).unwrap();
    assert_eq!(c.styles[0].status, DevelopmentStatus::Planned);
    assert!(c.train_families.is_empty());
}
#[cfg(unix)]
#[test]
fn links_cannot_escape_asset_root() {
    let root = temporary();
    let outside = temporary();
    std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
    assert!(resolve_existing(&root, "escape").is_err());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}
