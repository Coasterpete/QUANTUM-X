use qx_document::{
    command::{Command, Editor},
    library::load_train_definition,
    persistence, sample,
    train::Train,
};
use qx_style::{
    Catalog, DevelopmentStatus,
    library::{ContentManifest, Library, LibraryKind, PreviewImages},
};
use std::path::PathBuf;

fn temporary() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "qx-library-document-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn real_definition_loading_compatibility_persistence_and_history() {
    let root = temporary();
    let mut catalog = Catalog::built_in().unwrap();
    let style = catalog
        .train_styles
        .iter_mut()
        .find(|s| s.id == "motorcycle-suspended-100")
        .unwrap();
    style.status = DevelopmentStatus::Available;
    style.compatibility.track_style_ids = vec!["monorail-100".into()];
    let dir = root.join(&style.directory);
    std::fs::create_dir_all(&dir).unwrap();
    let definition = Train::development_fixture("synthetic-library-test".into());
    std::fs::write(
        dir.join("mechanics.json"),
        serde_json::to_string(&definition).unwrap(),
    )
    .unwrap();
    let manifest = ContentManifest {
        version: 1,
        kind: LibraryKind::Train,
        style_id: style.id.clone(),
        visuals: vec![],
        previews: PreviewImages {
            thumbnail: Some("thumbnail.png".into()),
            large: Some("preview.png".into()),
            references: vec![],
        },
        construction: None,
        mechanical_definition: Some("mechanics.json".into()),
    };
    std::fs::write(
        dir.join("style.json"),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    for name in ["thumbnail.png", "preview.png"] {
        std::fs::write(dir.join(name), b"path resolution test only").unwrap();
    }
    let library = Library::from_catalog(catalog.clone(), &root).unwrap();
    let resolved = load_train_definition(&library, "motorcycle-suspended-100").unwrap();
    assert!(resolved.visual_sources.is_empty());
    assert_eq!(resolved.definition.components, definition.components);
    assert_eq!(resolved.definition.connections, definition.connections);
    assert_eq!(
        resolved.definition.compatibility.train_style_id.as_deref(),
        Some("motorcycle-suspended-100")
    );
    let mut editor = Editor::new(sample::foundation_project(), catalog.clone(), 8).unwrap();
    editor
        .execute([Command::SetTrains(vec![resolved.definition.clone()])])
        .unwrap();
    let json = persistence::to_json(editor.project(), &catalog).unwrap();
    assert_eq!(
        persistence::from_json(&json, &catalog).unwrap(),
        *editor.project()
    );
    assert!(editor.undo());
    assert!(editor.project().trains.is_empty());
    assert!(editor.redo());
    assert_eq!(editor.project().trains[0], resolved.definition);
    // Domain validation rejects malformed mechanics even when all paths exist.
    let mut invalid = definition.clone();
    invalid.components[0].mass_kg = -1.;
    std::fs::write(
        dir.join("mechanics.json"),
        serde_json::to_string(&invalid).unwrap(),
    )
    .unwrap();
    assert!(load_train_definition(&library, "motorcycle-suspended-100").is_err());
    let mut invalid = definition;
    invalid.components[0].asset = Some(qx_document::train::Asset {
        path: "undeclared.glb".into(),
        translation_m: Default::default(),
        rotation: qx_geometry::math::Quaternion::IDENTITY,
        scale: qx_geometry::math::Vec3::new(1., 1., 1.),
        pivot_m: Default::default(),
    });
    std::fs::write(
        dir.join("mechanics.json"),
        serde_json::to_string(&invalid).unwrap(),
    )
    .unwrap();
    assert!(load_train_definition(&library, "motorcycle-suspended-100").is_err());
    std::fs::remove_file(dir.join("mechanics.json")).unwrap();
    assert!(load_train_definition(&library, "motorcycle-suspended-100").is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn existing_project_fixtures_load_with_the_expanded_catalog() {
    let catalog = Catalog::built_in().unwrap();
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut count = 0;
    for file in std::fs::read_dir(examples).unwrap() {
        let file = file.unwrap().path();
        if file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".qx.json")
        {
            let p = persistence::load(&file, &catalog).unwrap();
            let reopened =
                persistence::from_json(&persistence::to_json(&p, &catalog).unwrap(), &catalog)
                    .unwrap();
            assert_eq!(p, reopened);
            count += 1;
        }
    }
    assert!(count >= 4);
}
