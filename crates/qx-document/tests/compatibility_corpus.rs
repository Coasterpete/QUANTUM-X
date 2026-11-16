//! Stored inputs and separately stored schema-7 expectations, never generated in tests.
use qx_document::{Project, SCHEMA_VERSION, persistence};
use qx_geometry::{authoring::SectionKind, transition::Function};
use qx_style::Catalog;
use std::{collections::BTreeSet, fs, path::PathBuf};

fn corpus() -> (PathBuf, serde_json::Value) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/compatibility");
    let manifest =
        serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    (root, manifest)
}

fn assert_authored_information(legacy: &Project, migrated: &Project) {
    let mut preserved = legacy.clone();
    preserved.schema_version = SCHEMA_VERSION;
    // Only legacy Hermite representation changes. Match its independently
    // evaluated values/derivatives using the EXISTING transition_regions bounds.
    for (source, result) in preserved.sections.iter_mut().zip(&migrated.sections) {
        for (key, old) in source.channels().into_iter().zip(result.channels()) {
            assert_eq!(key.0, old.0);
            assert_eq!(key.1.transitions.len(), old.1.transitions.len());
            for (a, b) in key.1.transitions.iter().zip(&old.1.transitions) {
                assert_eq!(
                    (a.id, a.length, a.start, a.end),
                    (b.id, b.length, b.start, b.end)
                );
                if legacy.schema_version < 6 && matches!(a.function, Function::Hermite { .. }) {
                    assert!(matches!(b.function, Function::Nurbs { .. }));
                    for i in 0..=1000 {
                        let x = a.length * i as f64 / 1000.;
                        let a = a.evaluate(x).unwrap();
                        let b = b.evaluate(x).unwrap();
                        assert!((a.value - b.value).abs() < 2e-14);
                        assert!((a.derivative - b.derivative).abs() < 2e-12);
                        assert!((a.second_derivative - b.second_derivative).abs() < 2e-9);
                    }
                } else {
                    assert_eq!(a, b);
                }
            }
        }
        // Replace only the representation-changing fields after testing their
        // semantics; the full equality below protects every other authored field.
        for key in qx_document::editing::channel_keys(source) {
            let result_channel = result.clone().channel_mut(key).unwrap().clone();
            for (a, b) in source
                .channel_mut(key)
                .unwrap()
                .transitions
                .iter_mut()
                .zip(result_channel.transitions)
            {
                if legacy.schema_version < 6 && matches!(a.function, Function::Hermite { .. }) {
                    a.function = b.function;
                    a.amplitude = b.amplitude;
                }
            }
        }
    }
    assert_eq!(&preserved, migrated);
}

#[test]
fn immutable_schemas_one_through_seven_load_migrate_save_and_reload() {
    let c = Catalog::built_in().unwrap();
    let (root, manifest) = corpus();
    assert_eq!(manifest["format_version"], 1);
    let mut schemas = BTreeSet::new();
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["input"].as_str().unwrap();
        let path = root.join(name);
        let bytes = fs::read(&path).unwrap();
        let legacy: Project = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            u64::from(legacy.schema_version),
            case["schema"].as_u64().unwrap()
        );
        schemas.insert(legacy.schema_version);
        let expected: Project = serde_json::from_str(
            &fs::read_to_string(root.join(case["expected"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        expected.validate(&c).unwrap(); // Expected data is not fed through migration.
        let project = persistence::load(&path, &c).unwrap();
        assert_eq!(project, expected, "{name}: committed migration expectation");
        assert_authored_information(&legacy, &project);
        let generated = project.generate(&c, Default::default()).unwrap();
        // The pre-migration source can generate before Hermite conversion too.
        let source =
            qx_geometry::solver::generate(&legacy.initial, &legacy.sections, Default::default())
                .unwrap();
        assert_eq!(source.samples.len(), generated.samples.len());
        for (a, b) in source.samples.iter().zip(&generated.samples) {
            assert!(
                (a.position_m - b.position_m).length() < 2e-12,
                "{name}: migration geometry"
            );
        }
        let temporary = std::env::temp_dir().join(format!("qx-r03-{}-{name}", std::process::id()));
        assert!(!temporary.exists(), "unexpected occupied scratch path");
        persistence::save(&temporary, &project, &c).unwrap();
        assert_eq!(persistence::load(&temporary, &c).unwrap(), project);
        // Exercise replacement of an existing saved copy as well.
        persistence::save(&temporary, &project, &c).unwrap();
        assert_eq!(persistence::load(&temporary, &c).unwrap(), expected);
        fs::remove_file(temporary).unwrap();
        assert_eq!(
            fs::read(path).unwrap(),
            bytes,
            "historical input was modified"
        );
        println!(
            "COMPATIBILITY {name} schema={} sections={} trains={} supports={}",
            legacy.schema_version,
            project.sections.len(),
            project.trains.len(),
            project.supports.len()
        );
    }
    assert_eq!(schemas, (1..=7).collect());
}

#[test]
fn corpus_rejects_unknown_future_and_truncated_data() {
    let c = Catalog::built_in().unwrap();
    let (root, manifest) = corpus();
    for case in manifest["cases"].as_array().unwrap() {
        let json = fs::read_to_string(root.join(case["input"].as_str().unwrap())).unwrap();
        for len in [1, json.len() / 2] {
            assert!(persistence::from_json(&json[..len], &c).is_err());
        }
        let mut unknown: serde_json::Value = serde_json::from_str(&json).unwrap();
        unknown["unknown_r03_data"] = true.into();
        assert!(persistence::from_json(&unknown.to_string(), &c).is_err());
        unknown.as_object_mut().unwrap().remove("unknown_r03_data");
        unknown["schema_version"] = 8.into();
        assert!(
            persistence::from_json(&unknown.to_string(), &c)
                .unwrap_err()
                .to_string()
                .contains("unsupported project schema")
        );
    }
}

#[test]
fn e01_known_station_domain_defect_is_reproduced_without_rescaling() {
    let c = Catalog::built_in().unwrap();
    let (root, _) = corpus();
    let p = persistence::load(&root.join("schema7-e01-known-defect.qx.json"), &c).unwrap();
    assert!(matches!(p.sections[0].kind, SectionKind::Arc { .. }));
    let authored = p.generate(&c, Default::default()).unwrap();
    let reference = p.reference_geometry(&authored).unwrap();
    let physical = qx_geometry::supports::physical_attachment_track(&authored, &reference).unwrap();
    assert!((authored.length_m() - 40.).abs() < 1e-9);
    assert!((physical.length_m() - 36.).abs() < 1e-9);
    p.operations.validate_track(&authored).unwrap();
    assert_eq!(
        p.operations
            .validate_track(&physical)
            .unwrap_err()
            .to_string(),
        "device stations exceed current track; revise its start/end positions"
    );
    let reopened = persistence::from_json(&persistence::to_json(&p, &c).unwrap(), &c).unwrap();
    assert_eq!(reopened, p);
    assert_eq!(
        (
            p.operations.devices[0].start_m,
            p.operations.devices[0].end_m
        ),
        (38., 40.)
    );
}
