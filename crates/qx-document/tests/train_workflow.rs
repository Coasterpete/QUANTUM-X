use qx_document::{
    command::{Command, Editor},
    persistence,
    sample::foundation_project,
    train::{ComponentKind, Train},
};
use qx_style::Catalog;

#[test]
fn trains_roundtrip_validate_and_undo_atomically() {
    let catalog = Catalog::built_in().unwrap();
    let baseline = foundation_project();
    let mut editor = Editor::new(baseline.clone(), catalog.clone(), 8).unwrap();
    let mut train = Train::development_fixture("train-a".into());
    train
        .components
        .iter_mut()
        .find(|c| c.id == "car-2")
        .unwrap()
        .dimensions_m
        .x = 3.5;
    train.cars[1].bogie_ids.push("bogie-1-2".into());
    train.validate().unwrap();
    editor
        .execute([Command::SetTrains(vec![train.clone()])])
        .unwrap();
    let json = persistence::to_json(editor.project(), &catalog).unwrap();
    assert_eq!(
        persistence::from_json(&json, &catalog).unwrap(),
        *editor.project()
    );
    assert!(editor.undo());
    assert_eq!(editor.project(), &baseline);
    assert!(editor.redo());
    assert_eq!(editor.project().trains, vec![train.clone()]);
    let mut invalid = train.clone();
    invalid.components[1].id = invalid.components[0].id.clone();
    assert!(editor.execute([Command::SetTrains(vec![invalid])]).is_err());
    let mut invalid = train;
    invalid.connections[0].component_b = "missing".into();
    assert!(editor.execute([Command::SetTrains(vec![invalid])]).is_err());
    assert_eq!(
        editor.project().trains[0]
            .components
            .iter()
            .filter(|c| c.kind == ComponentKind::CarBody)
            .count(),
        2
    );
}
#[test]
fn old_version_and_asset_paths_are_checked() {
    use qx_document::train::Asset;
    use qx_geometry::math::{Quaternion, Vec3};
    let catalog = Catalog::built_in().unwrap();
    let p = foundation_project();
    let mut old = serde_json::to_value(&p).unwrap();
    old["schema_version"] = 4.into();
    old.as_object_mut().unwrap().remove("trains");
    assert_eq!(
        persistence::from_json(&old.to_string(), &catalog).unwrap(),
        p
    );
    let asset = |path: &str| Asset {
        pivot_m: Vec3::ZERO,
        path: path.into(),
        translation_m: Vec3::ZERO,
        rotation: Quaternion::IDENTITY,
        scale: Vec3::new(1., 1., 1.),
    };
    assert!(asset("assets/body.glb").validate().is_ok());
    for path in [
        "../body.glb",
        "C:/body.glb",
        "/body.glb",
        "assets\\body.glb",
    ] {
        assert!(asset(path).validate().is_err(), "{path}");
    }
}
