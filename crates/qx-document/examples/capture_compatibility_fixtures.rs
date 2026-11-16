//! Reconstructs the schema 4–7 test variants; schemas 1–3 come from saved examples.
//! Refuses existing output directories. Never invoked by tests or CI.
use qx_document::{
    Project,
    operations::{AccelerationKey, DeviceKind, TrackDevice},
    persistence,
    sample::foundation_project,
    train::{Asset, DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    heartline::{AuthoredReference, OffsetProfile},
    math::{Quaternion, Vec3},
    supports::{TimberRecipe, timber},
    topology::LayoutMode,
    transition::{Channel, Function, Transition, Unit},
};
use qx_style::{Catalog, construction::TrackConstruction};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(output: &Path, name: &str, p: &Project, version: u32) {
    let mut v = serde_json::to_value(p).unwrap();
    v["schema_version"] = version.into();
    for (introduced, key) in [
        (3, "heartline"),
        (4, "construction"),
        (5, "trains"),
        (7, "operations"),
        (7, "supports"),
    ] {
        if version < introduced {
            v.as_object_mut().unwrap().remove(key);
        }
    }
    if name == "schema5-train" {
        // Early schema-5 assets lacked pivots; optional mass/mechanism fields
        // already have dedicated migration tests. Preserve that absent-field case.
        for c in v["trains"][0]["components"].as_array_mut().unwrap() {
            if let Some(asset) = c.get_mut("asset").and_then(Value::as_object_mut) {
                asset.remove("pivot_m");
            }
        }
    }
    fs::write(
        output.join(format!("{name}.qx.json")),
        format!("{}\n", serde_json::to_string_pretty(&v).unwrap()),
    )
    .unwrap();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a NEW output directory")?,
    );
    fs::create_dir(&output)?;
    for (name, bytes) in [
        (
            "schema1-foundation",
            include_str!("../../../examples/foundation.qx.json"),
        ),
        (
            "schema2-mixed",
            include_str!("../../../examples/mixed-authoring.qx.json"),
        ),
        (
            "schema3-heartline",
            include_str!("../../../examples/heartline-reference.qx.json"),
        ),
    ] {
        // Normalize checkout EOLs only; Git blob content is otherwise unchanged.
        fs::write(
            output.join(format!("{name}.qx.json")),
            bytes.replace("\r\n", "\n"),
        )?;
    }
    let c = Catalog::built_in()?;
    let mut p = foundation_project();
    p.metadata.project_id = "compatibility-reconstruction".into();
    p.metadata.description = "Frozen R0.3 compatibility input reconstructed from existing migration test patterns; not a historical user file.".into();
    p.initial.position_m = Vec3::new(0.123456789012345, 10.000000000000002, -0.234567890123456);
    p.sections = vec![Section::straight(101, "Precision authored line", 50.)];
    p.construction = TrackConstruction::generic_single();
    p.construction.materials[0].roughness = 0.375;
    p.heartline.rider_offset = OffsetProfile::Constant {
        offset_m: Vec3::new(0., 1.25, 0.125),
    };
    write(&output, "schema4-construction", &p, 4);
    let mut train = Train::development_consist(
        "compat-train".into(),
        3,
        DevelopmentArrangement::Conventional,
    )?;
    train.components[0].asset = Some(Asset {
        path: "assets/development/body.glb".into(),
        translation_m: Vec3::new(0.125, 0., 0.),
        rotation: Quaternion::IDENTITY,
        scale: Vec3::new(1., 1., 1.),
        pivot_m: Vec3::ZERO,
    });
    p.trains.push(train);
    p.heartline.train_configuration_id = Some("compat-train".into());
    write(&output, "schema5-train", &p, 5);
    let mut hermite = p.clone();
    let mut section =
        Section::straight(105, "Legacy Hermite with amplitude and explicit jump", 10.);
    section.kind = SectionKind::RiderLocal {
        pitch_rate: Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![
                Transition {
                    id: 41,
                    length: 3.577,
                    start: 0.,
                    end: 0.01,
                    amplitude: 0.004,
                    function: Function::Hermite {
                        start_slope: 0.001,
                        end_slope: 0.002,
                    },
                },
                Transition {
                    id: 42,
                    length: 6.423,
                    start: 0.013,
                    end: 0.03,
                    amplitude: 0.004,
                    function: Function::Hermite {
                        start_slope: 0.001,
                        end_slope: 0.002,
                    },
                },
            ],
        },
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    hermite.sections = vec![section];
    write(&output, "schema5-hermite", &hermite, 5);
    for (_, channel) in hermite.sections[0].channels() {
        channel.validate()?;
    }
    let mut nurbs = hermite.clone();
    if let SectionKind::RiderLocal { pitch_rate, .. } = &mut nurbs.sections[0].kind {
        for t in &mut pitch_rate.transitions {
            *t = t.to_nurbs()?;
        }
    }
    write(&output, "schema6-nurbs", &nurbs, 6);
    p.operations.layout = LayoutMode::Shuttle;
    p.operations.physical.initial_speed_mps = -3.125;
    p.operations.physical.drag_area_m2 = 1.5;
    let mut launch = TrackDevice::new(701, DeviceKind::Launch, 5., 25.);
    launch.maximum_force_n = 1234.5;
    launch.profile = vec![
        AccelerationKey {
            distance_m: 0.,
            acceleration_mps2: 1.,
        },
        AccelerationKey {
            distance_m: 20.,
            acceleration_mps2: 2.5,
        },
    ];
    let mut brake = TrackDevice::new(702, DeviceKind::Brake, 30., 45.);
    brake.acceleration_curve = Some(Channel::constant(Unit::MetresPerSecondSquared, 15., 1.25));
    p.operations.devices = vec![launch, brake];
    p.supports = vec![timber(
        71,
        TimberRecipe {
            end_m: 40.,
            foundation_elevation_m: 0.,
            ..Default::default()
        },
        &p.generate(&c, Default::default())?,
    )?];
    write(&output, "schema7-integrated", &p, 7);
    // Exact reproduction of audit E01. This records current acceptance/rejection,
    // and must not be mistaken for a valid simulation fixture or a fix.
    let mut e01 = foundation_project();
    let mut arc = Section::straight(1, "E01 station-domain known defect", 40.);
    arc.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: 0.,
    };
    e01.sections = vec![arc];
    e01.heartline.authored_reference = AuthoredReference::RiderReference;
    e01.heartline.rider_offset = OffsetProfile::Constant {
        offset_m: Vec3::new(0., -2., 0.),
    };
    e01.operations.devices = vec![TrackDevice::new(1, DeviceKind::Brake, 38., 40.)];
    write(&output, "schema7-e01-known-defect", &e01, 7);
    // Expected snapshots are captured separately and reviewed, never recomputed
    // by the compatibility test. This utility accepts only valid legacy inputs.
    for entry in fs::read_dir(&output)?.collect::<Result<Vec<_>, _>>()? {
        let path = entry.path();
        let migrated = persistence::from_json(&fs::read_to_string(&path)?, &c)?;
        let stem = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(".qx.json", ".expected.json");
        fs::write(
            output.join(stem),
            format!("{}\n", persistence::to_json(&migrated, &c)?),
        )?;
    }
    Ok(())
}
