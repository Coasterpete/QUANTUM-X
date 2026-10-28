use qx_document::{
    SCHEMA_VERSION,
    command::{Command, Editor},
    persistence,
    sample::foundation_project,
};
use qx_style::{Catalog, construction::TrackConstruction};
#[test]
fn m6_schema_three_migrates_without_reauthoring_and_m7_roundtrips() {
    let catalog = Catalog::built_in().unwrap();
    let original = foundation_project();
    let mut value = serde_json::to_value(&original).unwrap();
    value["schema_version"] = 3.into();
    value.as_object_mut().unwrap().remove("trains");
    value.as_object_mut().unwrap().remove("construction");
    let migrated = persistence::from_json(&value.to_string(), &catalog).unwrap();
    assert_eq!(migrated, original);
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    let track = migrated.generate(&catalog, Default::default()).unwrap();
    let mut editor = Editor::new(migrated.clone(), catalog.clone(), 16).unwrap();
    editor
        .execute([Command::SetConstruction(TrackConstruction::generic_single())])
        .unwrap();
    assert_eq!(
        track,
        editor
            .project()
            .generate(&catalog, Default::default())
            .unwrap()
    );
    let reopened = persistence::from_json(
        &persistence::to_json(editor.project(), &catalog).unwrap(),
        &catalog,
    )
    .unwrap();
    assert_eq!(&reopened, editor.project());
    assert!(editor.undo());
    assert_eq!(editor.project(), &migrated);
    assert!(editor.redo());
    assert_eq!(editor.project(), &reopened);
    let mut invalid = reopened.construction.clone();
    invalid.repeated[0].spacing_m = 0.;
    assert!(editor.execute([Command::SetConstruction(invalid)]).is_err());
    assert_eq!(editor.project(), &reopened);
    let mut corrupt = serde_json::to_value(&reopened).unwrap();
    corrupt["construction"]["unknown"] = true.into();
    assert!(persistence::from_json(&corrupt.to_string(), &catalog).is_err());
    corrupt = serde_json::to_value(&reopened).unwrap();
    corrupt.as_object_mut().unwrap().remove("construction");
    assert!(persistence::from_json(&corrupt.to_string(), &catalog).is_err());
}
