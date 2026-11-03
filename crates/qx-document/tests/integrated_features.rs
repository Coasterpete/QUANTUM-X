use qx_document::{
    command::{Command, Editor},
    operations::*,
    persistence,
    sample::foundation_project,
};
use qx_geometry::{authoring::Section, math::Vec3, supports::*, topology::LayoutMode};
use qx_style::Catalog;
#[test]
fn acceleration_curves_convert_legacy_keys_and_validate_between_samples() {
    use qx_geometry::transition::{Function, Transition};
    let mut d = TrackDevice::new(1, DeviceKind::Launch, 0., 20.);
    d.profile = vec![
        AccelerationKey {
            distance_m: 0.,
            acceleration_mps2: 1.,
        },
        AccelerationKey {
            distance_m: 7.,
            acceleration_mps2: 3.,
        },
        AccelerationKey {
            distance_m: 20.,
            acceleration_mps2: 2.,
        },
    ];
    let curve = d.editable_curve(20.);
    let original = d.clone();
    d.profile.clear();
    d.acceleration_curve = Some(curve);
    d.validate().unwrap();
    for i in 0..=200 {
        let x = i as f64 / 10.;
        assert!((d.command(x) - original.command(x)).abs() < 1e-12);
    }
    let c = d.acceleration_curve.as_mut().unwrap();
    c.transitions = vec![Transition {
        id: 1,
        length: 20.,
        start: 0.,
        end: 0.,
        amplitude: -1.,
        function: Function::Linear {},
    }];
    assert!(d.validate().is_err());
    let c = d.acceleration_curve.as_mut().unwrap();
    c.transitions[0].amplitude = 1.;
    c.transitions[0].function = Function::Hermite {
        start_slope: -1.,
        end_slope: 0.,
    };
    assert!(d.validate().is_err());
    let c = d.acceleration_curve.as_mut().unwrap();
    c.transitions[0] = Transition {
        id: 1,
        length: 20.,
        start: 1.,
        end: 3.,
        amplitude: 0.,
        function: Function::Cosine {},
    };
    d.validate().unwrap();
    d.end_m = 40.;
    assert!(d.length(100., false).is_err());
    d.fit_profile(100., false).unwrap();
    assert_eq!(d.length(100., false).unwrap(), 40.);
    assert!((d.command(20.) - 2.).abs() < 1e-12);
}
#[test]
fn schema_seven_roundtrip_atomic_history_and_invalid_inputs_preserve_every_system() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = foundation_project();
    p.sections = vec![Section::straight(1, "track", 50.)];
    p.initial.position_m.y = 10.;
    let t = p.generate(&catalog, Default::default()).unwrap();
    let structure = timber(
        1,
        TimberRecipe {
            end_m: 40.,
            foundation_elevation_m: 0.,
            ..Default::default()
        },
        &t,
    )
    .unwrap();
    let mut op = Operations {
        layout: LayoutMode::Shuttle,
        ..Default::default()
    };
    op.devices
        .push(TrackDevice::new(1, DeviceKind::Launch, 5., 25.));
    op.physical.initial_speed_mps = 3.;
    op.physical.drag_area_m2 = 1.5;
    let mut e = Editor::new(p.clone(), catalog.clone(), 16).unwrap();
    e.execute([
        Command::SetOperations(op),
        Command::SetSupports(vec![structure]),
        Command::ValidateConnections {
            options: Default::default(),
        },
    ])
    .unwrap();
    let edited = e.project().clone();
    let path =
        std::env::temp_dir().join(format!("quantum-integrated-{}.qx.json", std::process::id()));
    persistence::save(&path, &edited, &catalog).unwrap();
    persistence::save(&path, &edited, &catalog).unwrap();
    assert_eq!(persistence::load(&path, &catalog).unwrap(), edited);
    std::fs::remove_file(path).unwrap();
    assert!(e.undo());
    assert_eq!(e.project(), &p);
    assert!(e.redo());
    assert_eq!(e.project(), &edited);
    let mut invalid = edited.operations.clone();
    invalid.devices[0].end_m = 100.;
    assert!(
        e.execute([
            Command::SetOperations(invalid),
            Command::ValidateConnections {
                options: Default::default()
            }
        ])
        .is_err()
    );
    assert_eq!(e.project(), &edited);
    assert!(
        e.execute([Command::RemoveSection {
            id: edited.sections[0].id
        }])
        .is_err()
    );
    assert_eq!(e.project(), &edited);
    let mut json = serde_json::to_value(&edited).unwrap();
    json["operations"]["devices"][0]["kind"] = "lift".into();
    assert!(persistence::from_json(&json.to_string(), &catalog).is_err());
    let mut json = serde_json::to_value(&edited).unwrap();
    json["supports"][0]["members"][0]["end_node"] = 999999.into();
    assert!(persistence::from_json(&json.to_string(), &catalog).is_err());
    let mut json = serde_json::to_value(&edited).unwrap();
    json["operations"]["unknown"] = true.into();
    assert!(persistence::from_json(&json.to_string(), &catalog).is_err());
}
#[test]
fn schema_six_migration_preserves_authored_track_train_materials_and_precise_supports() {
    let c = Catalog::built_in().unwrap();
    let p = foundation_project();
    let mut old = serde_json::to_value(&p).unwrap();
    old["schema_version"] = 6.into();
    old.as_object_mut().unwrap().remove("operations");
    old.as_object_mut().unwrap().remove("supports");
    let migrated = persistence::from_json(&old.to_string(), &c).unwrap();
    assert_eq!(migrated, p);
    assert_eq!(
        migrated.generate(&c, Default::default()).unwrap(),
        p.generate(&c, Default::default()).unwrap()
    );
    // Author precise editable construction independently of a procedural recipe.
    let mut structure = Structure::empty(1);
    structure.nodes = vec![
        Node {
            id: 1,
            name: "foundation".into(),
            position: NodePosition::World {
                position_m: Vec3::new(1.234567890123, 0., 2.),
                foundation: true,
            },
        },
        Node {
            id: 2,
            name: "mount".into(),
            position: NodePosition::Track {
                attachment: Attachment {
                    section_id: p.sections[0].id,
                    fraction: 0.5,
                    offset_m: Vec3::new(0., -0.5, 1.),
                },
            },
        },
    ];
    structure.members.push(Member {
        id: 1,
        start_node: 1,
        end_node: 2,
        profile: MemberProfile::Circular {
            diameter_m: 0.123456789,
        },
        roll_rad: 0.1234567890123,
    });
    let mut p = migrated;
    p.supports.push(structure);
    let re = persistence::from_json(&persistence::to_json(&p, &c).unwrap(), &c).unwrap();
    assert_eq!(re, p);
}
