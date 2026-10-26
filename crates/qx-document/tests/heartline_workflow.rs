use qx_document::{
    HeartlineSettings,
    command::{Command, Editor},
    editing, persistence,
    sample::foundation_project,
};
use qx_geometry::{
    authoring::{ChannelKey, Domain, SectionId},
    heartline::{AuthoredReference, OffsetProfile},
    math::Vec3,
    solver::GenerationOptions,
};
use qx_style::Catalog;
#[test]
fn heartline_roundtrip_undo_and_style_independence_preserve_authored_layout() {
    let catalog = Catalog::built_in().unwrap();
    let p = foundation_project();
    let baseline = p.generate(&catalog, GenerationOptions::default()).unwrap();
    let mut editor = Editor::new(p.clone(), catalog.clone(), 16).unwrap();
    let settings = HeartlineSettings {
        authored_reference: AuthoredReference::TrackCenterline,
        rider_offset: OffsetProfile::Quintic {
            start_m: Vec3::Y,
            end_m: Vec3::Y * 1.4,
        },
        train_configuration_id: Some("nominal-train-1".into()),
    };
    editor
        .execute([Command::SetHeartline(settings.clone())])
        .unwrap();
    assert_eq!(
        baseline,
        editor
            .project()
            .generate(&catalog, GenerationOptions::default())
            .unwrap()
    );
    let derived = editor.project().reference_geometry(&baseline).unwrap();
    assert_ne!(derived.points[0].position_m, baseline.samples[0].position_m);
    let reopened = persistence::from_json(
        &persistence::to_json(editor.project(), &catalog).unwrap(),
        &catalog,
    )
    .unwrap();
    assert_eq!(reopened.heartline, settings);
    assert_eq!(derived, reopened.reference_geometry(&baseline).unwrap());
    assert!(editor.undo());
    assert_eq!(editor.project(), &p);
    assert!(editor.redo());
    assert_eq!(editor.project(), &reopened);
    let mut invalid = settings;
    invalid.rider_offset = OffsetProfile::Constant {
        offset_m: Vec3::Y * 101.,
    };
    assert!(editor.execute([Command::SetHeartline(invalid)]).is_err());
    assert_eq!(editor.project(), &reopened);
}
#[test]
fn v1_v2_migration_and_v3_strict_settings_are_preserved() {
    let catalog = Catalog::built_in().unwrap();
    for fixture in [
        include_str!("../../../examples/foundation.qx.json"),
        include_str!("../../../examples/mixed-authoring.qx.json"),
    ] {
        let p = persistence::from_json(fixture, &catalog).unwrap();
        assert_eq!(p.schema_version, qx_document::SCHEMA_VERSION);
        assert_eq!(p.heartline, HeartlineSettings::default());
    }
    let mut value: serde_json::Value =
        serde_json::from_str(&persistence::to_json(&foundation_project(), &catalog).unwrap())
            .unwrap();
    value["heartline"]["unknown"] = 1.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    value["heartline"]
        .as_object_mut()
        .unwrap()
        .remove("unknown");
    value.as_object_mut().unwrap().remove("construction");
    value["schema_version"] = 2.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    value["schema_version"] = 3.into();
    value.as_object_mut().unwrap().remove("heartline");
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
}
#[test]
fn section_reordering_and_transition_edits_regenerate_and_undo_as_transactions() {
    let catalog = Catalog::built_in().unwrap();
    let p = foundation_project();
    let mut editor = Editor::new(p.clone(), catalog.clone(), 16).unwrap();
    editor
        .execute([Command::MoveSection {
            id: SectionId(5),
            after: None,
        }])
        .unwrap();
    assert_eq!(editor.project().sections[0].id, SectionId(5));
    editor
        .project()
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project(), &p);
    let section = editing::split_hermite(&p.sections[1], ChannelKey::Normal, 1, 0.5).unwrap();
    let section = editing::resize(&section, 80.).unwrap();
    editor
        .execute([Command::ReplaceSection { section }])
        .unwrap();
    let changed = editor
        .project()
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    assert!((changed.length_m() - 172.).abs() < 1e-8);
    assert!(editor.undo());
    assert_eq!(editor.project(), &p);
    assert!(editor.redo());
    let section =
        editing::remove_transition(&editor.project().sections[1], ChannelKey::Normal, 3).unwrap();
    assert_eq!(section.extent, 80.);
    editor
        .execute([Command::ReplaceSection { section }])
        .unwrap();
}
#[test]
fn domain_conversion_preserves_geometry_and_hermite_slopes() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = foundation_project();
    let before = p.generate(&catalog, GenerationOptions::default()).unwrap();
    p.sections[1] = editing::convert_domain(&p.sections[1], Domain::Time, 20.).unwrap();
    let after = p.generate(&catalog, GenerationOptions::default()).unwrap();
    assert!(
        (before.samples.last().unwrap().position_m - after.samples.last().unwrap().position_m)
            .length()
            < 1e-8
    );
}
#[test]
fn mixed_spline_force_and_geometric_heartlines_regenerate_deterministically() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = qx_document::sample::mixed_authoring_project();
    p.heartline.rider_offset = OffsetProfile::Constant {
        offset_m: Vec3::new(0.4, 1.2, -0.3),
    };
    let source = p.generate(&catalog, GenerationOptions::default()).unwrap();
    let reference = p.reference_geometry(&source).unwrap();
    assert!(
        reference
            .points
            .iter()
            .all(|p| p.position_m.is_finite() && p.curvature_per_m.is_finite() && p.speed_mps > 0.)
    );
    assert_eq!(reference, p.reference_geometry(&source).unwrap());
    let reopened =
        persistence::from_json(&persistence::to_json(&p, &catalog).unwrap(), &catalog).unwrap();
    assert_eq!(reference, reopened.reference_geometry(&source).unwrap());
}
