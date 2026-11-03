use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    construction::{ConstructionAssets, MeshLimits, build_with_assets},
    solver::{GenerationOptions, generate},
};
use qx_rendering::construction_assets::Sources;
use qx_style::construction::TrackConstruction;
use std::path::PathBuf;

const SPINE: &str = "tracks/box-spined/100/Box-Spine-100.glb";
const TIE: &str = "tracks/box-spined/100/Box-Spine-100-crosstie.glb";
fn sources() -> Sources {
    Sources::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
}
/// Exercise the registered, owner-authorized provisional construction.
fn fixture() -> TrackConstruction {
    qx_style::library::ContentManifest::from_json(include_str!(
        "../../../assets/tracks/box-spined/100/style.json"
    ))
    .unwrap()
    .construction
    .unwrap()
}
#[test]
fn planned_sources_validate_without_activating_unresolved_styles() {
    let assets = sources();
    for number in [100, 200, 300] {
        let spine = format!("tracks/box-spined/{number}/Box-Spine-{number}.glb");
        let tie = format!("tracks/box-spined/{number}/Box-Spine-{number}-crosstie.glb");
        let profile = assets.section(&spine, [1.; 3]).unwrap();
        let hardware = assets.hardware(&tie, [1.; 3]).unwrap();
        profile.validate().unwrap();
        hardware.validate().unwrap();
        println!(
            "style {number}: {} section edges, {} tie vertices; source validation only, assembly/attachment conventions still require owner specification",
            profile.points().unwrap().len(),
            hardware.vertices.len()
        );
    }
}
#[test]
fn original_spine_section_and_crosstie_are_preserved() {
    let assets = sources();
    let source = assets.original(SPINE, [1.; 3]).unwrap();
    assert_eq!((source.vertices.len(), source.indices.len()), (56, 132));
    let profile = assets.section(SPINE, [1.; 3]).unwrap();
    let points = profile.points().unwrap();
    assert_eq!(points.len(), 8, "retain all original corner chamfers");
    let min = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let max = points
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!((min + 0.975).abs() < 2e-6 && (max + 0.236).abs() < 2e-6);
    let tie = assets.hardware(TIE, [1.; 3]).unwrap();
    assert_eq!((tie.vertices.len(), tie.indices.len()), (184, 540));
    assert!(
        (tie.bounds.min.y + 0.336).abs() < 2e-6,
        "honor the original node's 180 degree rotation"
    );
    assert!((tie.bounds.max.z - tie.bounds.min.z - 1.2).abs() < 2e-6);
    assert!(tie.vertices.iter().any(|v| v.uv != [0.; 2]));
    let mut modified = (*source).clone();
    modified.vertices[0].position_m.y += 0.01;
    assert!(
        qx_geometry::source_profile::extract(&modified).is_err(),
        "reject nonprismatic source details"
    );
}
#[test]
fn original_hardware_follows_straight_hill_curve_bank_loop_and_section_joins() {
    let assets = sources();
    let construction = fixture();
    let mut hill = Section::straight(1, "hill", 12.);
    hill.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: 0.,
    };
    let mut curve = hill.clone();
    curve.kind = SectionKind::Arc {
        radius_m: 12.,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    let mut loop_section = Section::straight(1, "loop", std::f64::consts::TAU * 8.);
    loop_section.kind = SectionKind::Arc {
        radius_m: 8.,
        plane_angle_rad: 0.,
    };
    for (name, sections, bank) in [
        ("straight", vec![Section::straight(1, "straight", 8.)], 0.),
        ("hill", vec![hill], 0.),
        ("curve", vec![curve.clone()], 0.),
        ("bank", vec![curve], 0.8),
        ("loop", vec![loop_section], 0.),
        (
            "join",
            vec![Section::straight(1, "A", 4.), Section::straight(2, "B", 4.)],
            0.,
        ),
    ] {
        let initial = InitialConditions {
            orientation: qx_geometry::math::Quaternion::axis_angle(
                qx_geometry::math::Vec3::X,
                bank,
            )
            .unwrap(),
            ..Default::default()
        };
        let track = generate(&initial, &sections, GenerationOptions::default()).unwrap();
        let before = track.clone();
        let mesh = build_with_assets(&track, &construction, MeshLimits::default(), &assets)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(track, before);
        for component in &mesh.components {
            component.mesh.validate().unwrap();
        }
        assert_eq!(
            mesh.components[3].mesh,
            assets.hardware(TIE, [1.; 3]).unwrap()
        );
        let original = &mesh.components[3].mesh;
        for placement in &mesh.components[3].placements {
            assert!(
                placement
                    .frame
                    .orientation()
                    .conjugate()
                    .compose(
                        track
                            .sample_at_distance(placement.station_m)
                            .unwrap()
                            .frame
                            .orientation()
                    )
                    .rotation_vector()
                    .unwrap()
                    .length()
                    < 1e-10
            );
            let a = original.vertices[0].position_m;
            let b = original.vertices[1].position_m;
            assert!(
                (placement.frame.local_to_world(a - b).length() - (a - b).length()).abs() < 1e-12
            );
        }
        for (i, sample) in track.samples.iter().enumerate() {
            for (rail, lateral) in mesh.components[..2].iter().zip([-0.55, 0.55]) {
                let vertex = &rail.mesh.vertices[i * 96];
                let local = sample
                    .frame
                    .world_to_local(vertex.position_m - sample.position_m);
                assert!((local.y - 0.065).abs() < 1e-9 && (local.z - lateral).abs() < 1e-9);
                assert_eq!(vertex.uv[1], sample.distance_m);
            }
        }
    }
}
