use qx_document::{
    command::{Command, Editor},
    persistence,
    sample::foundation_project,
    train::{DevelopmentArrangement, Train, mechanism::*},
};
use qx_geometry::math::Vec3;
use qx_style::{Catalog, compatibility::Compatibility};
#[test]
fn optional_schema_five_mechanics_roundtrip_and_history_are_lossless() {
    let catalog = Catalog::built_in().unwrap();
    let mut editor = Editor::new(foundation_project(), catalog.clone(), 16).unwrap();
    let train = Train::development_consist(
        "multi-body".into(),
        4,
        DevelopmentArrangement::RotatingSeats,
    )
    .unwrap();
    editor
        .execute([Command::SetTrains(vec![train.clone()])])
        .unwrap();
    let json = persistence::to_json(editor.project(), &catalog).unwrap();
    assert_eq!(
        persistence::from_json(&json, &catalog).unwrap(),
        *editor.project()
    );
    assert!(editor.undo());
    assert!(editor.project().trains.is_empty());
    assert!(editor.redo());
    assert_eq!(editor.project().trains[0], train);
    let legacy = Train::development_fixture("legacy".into());
    let mut value = serde_json::to_value(&legacy).unwrap();
    value.as_object_mut().unwrap().remove("compatibility");
    for c in value["components"].as_array_mut().unwrap() {
        c.as_object_mut().unwrap().remove("center_of_mass_m");
    }
    for c in value["connections"].as_array_mut().unwrap() {
        c.as_object_mut().unwrap().remove("model");
    }
    assert_eq!(serde_json::from_value::<Train>(value).unwrap(), legacy);
}
#[test]
fn mounting_graphs_and_invalid_inertia_are_transactional() {
    let mut t =
        Train::development_consist("mechanism".into(), 1, DevelopmentArrangement::RotatingSeats)
            .unwrap();
    let baseline = t.clone();
    t.components[0].placement = Some(Placement::Mounted {
        parent: "seat-1".into(),
        frame: LocalFrame::default(),
    });
    assert!(t.validate().unwrap_err().to_string().contains("cyclic"));
    t = baseline.clone();
    t.components[3].center_of_mass_m = Vec3::new(f64::NAN, 0., 0.);
    assert!(t.validate().is_err());
    t = baseline.clone();
    t.components[0].inertia_kg_m2 = Vec3::new(1., 1., 3.);
    assert!(t.validate().is_err());
    t = baseline.clone();
    t.connections[0].model = JointModel::Hinge {
        frame_a: Default::default(),
        frame_b: Default::default(),
        limits: Some(AngleLimits {
            min_rad: 1.,
            max_rad: -1.,
        }),
        drive: RotationDrive::Free,
    };
    assert!(t.validate().is_err());
    t = baseline;
    t.components[3].placement = Some(Placement::Mounted {
        parent: "unknown".into(),
        frame: LocalFrame::default(),
    });
    assert!(t.validate().is_err());
}
#[test]
fn arbitrary_car_counts_roles_and_shared_running_gear_are_representable() {
    let mut t =
        Train::development_consist("mixed".into(), 5, DevelopmentArrangement::BelowTrack).unwrap();
    let shared = t.cars[0].bogie_ids[1].clone();
    t.cars[1].bogie_ids.push(shared);
    t.cars[2].bogie_ids.clear(); // Association count is not a physical topology constraint.
    t.components[0].mass_kg = 1234.;
    t.validate().unwrap();
    t.cars.reverse();
    assert!(t.validate().is_err());
    assert!(
        Train::development_consist("max".into(), 256, DevelopmentArrangement::RotatingSeats)
            .is_ok()
    );
}
#[test]
fn compatibility_is_an_extensible_declaration_not_a_validation_claim() {
    let mut style = Catalog::built_in().unwrap().styles.remove(0);
    let mut t = Train::development_fixture("metadata".into());
    assert_eq!(t.compatibility.assess(&style), Compatibility::Unspecified);
    t.compatibility.track_style_ids.push(style.id.clone());
    t.compatibility
        .required_track_features
        .push("custom:guide-fin".into());
    assert!(matches!(
        t.compatibility.assess(&style),
        Compatibility::Incompatible(_)
    ));
    style.vehicle_features.push("custom:guide-fin".into());
    assert_eq!(t.compatibility.assess(&style), Compatibility::Declared);
    t.compatibility
        .required_track_features
        .push("custom:guide-fin".into());
    assert!(t.validate().is_err());
}
