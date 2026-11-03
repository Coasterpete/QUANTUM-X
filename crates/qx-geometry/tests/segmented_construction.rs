use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    construction::{self, ConstructionAssets, Mesh, MeshLimits, segmented},
    math::{Quaternion, Vec3},
    solver::{GeneratedTrack, GenerationOptions, generate},
    supports::Attachment,
};
use qx_style::{
    construction::{Profile, Role, TrackConstruction},
    construction_segments::*,
};

struct NoAssets;
impl ConstructionAssets for NoAssets {
    fn section(&self, _: &str, _: [f64; 3]) -> qx_geometry::Result<Profile> {
        panic!("synthetic test must not resolve assets")
    }
    fn hardware(&self, _: &str, _: [f64; 3]) -> qx_geometry::Result<Mesh> {
        panic!("synthetic test must not resolve assets")
    }
}
fn track() -> GeneratedTrack {
    generate(
        &InitialConditions::default(),
        &[Section::straight(1, "A", 4.), Section::straight(2, "B", 6.)],
        GenerationOptions {
            max_step_m: 0.5,
            ..Default::default()
        },
    )
    .unwrap()
}
fn variants() -> Vec<ConstructionVariant> {
    // Entirely synthetic redistributable definition; does not test source assets.
    let mut base = TrackConstruction::generic_dual();
    base.id = "box-spined-100".into();
    box_spined_diagnostic_variants(&base).unwrap()
}
fn map(length: f64, cuts: &[f64]) -> ConstructionMap {
    let mut stations = vec![0.];
    stations.extend(cuts);
    stations.push(length);
    ConstructionMap {
        version: 1,
        segments: stations
            .windows(2)
            .enumerate()
            .map(|(i, p)| ConstructionSegment {
                id: format!("segment-{i}"),
                start_m: p[0],
                end_m: p[1],
                style_id: "box-spined-100".into(),
                variant_id: if i % 2 == 0 {
                    "source-single"
                } else {
                    "diagnostic-double"
                }
                .into(),
            })
            .collect(),
    }
}
fn build(
    t: &GeneratedTrack,
    m: &ConstructionMap,
    v: &[ConstructionVariant],
) -> segmented::SegmentedMesh {
    segmented::build(
        t,
        &m.validate(t.length_m(), v).unwrap(),
        Default::default(),
        &NoAssets,
    )
    .unwrap()
}
fn rails(mesh: &construction::TrackMesh) -> Vec<&construction::Component> {
    mesh.components
        .iter()
        .filter(|c| c.role == Role::Running)
        .collect()
}

#[test]
fn full_coverage_matches_existing_engine_and_preserves_authoring() {
    let t = track();
    let before = t.clone();
    let v = variants();
    let m = map(t.length_m(), &[]);
    let out = build(&t, &m, &v);
    let baseline = construction::build(&t, &v[0].definition, Default::default()).unwrap();
    assert_eq!(t, before);
    assert_eq!(rails(&out.mesh), rails(&baseline));
    for (a, b) in out.mesh.components.iter().zip(&baseline.components) {
        assert_eq!(a.mesh, b.mesh);
        assert_eq!(a.placements, b.placements);
        assert_eq!(a.bounds, b.bounds);
    }
    assert_eq!(out.mesh.stats, baseline.stats);
    assert!(out.boundaries.is_empty());
    // A map must preserve the original outer cap policy, even when disabled.
    let mut v = v;
    for variant in &mut v {
        for c in &mut variant.definition.continuous {
            if c.role != Role::Running {
                c.cap_ends = false;
            }
        }
    }
    let open = build(&t, &m, &v);
    let baseline = construction::build(&t, &v[0].definition, Default::default()).unwrap();
    assert_eq!(open.mesh.stats, baseline.stats);
    for (a, b) in open.mesh.components.iter().zip(&baseline.components) {
        assert_eq!(a.mesh, b.mesh);
    }
    let cut = build(&t, &map(t.length_m(), &[4.25]), &v);
    for c in cut
        .mesh
        .components
        .iter()
        .filter(|c| c.role == Role::Structure)
    {
        assert!(
            c.mesh
                .vertices
                .iter()
                .any(|p| p.station_m == 4.25 && p.normal.x.abs() > 0.99)
        );
        assert!(
            !c.mesh
                .vertices
                .iter()
                .any(|p| (p.station_m == 0. || p.station_m == 10.) && p.normal.x.abs() > 0.99)
        );
    }
}
#[test]
fn exact_boundary_ownership_sorting_and_final_station() {
    let v = variants();
    let mut m = map(10., &[4.25]);
    m.segments.reverse();
    let valid = m.validate(10., &v).unwrap();
    for (s, id) in [
        (0., "segment-0"),
        (4.25 - 1e-10, "segment-0"),
        (4.25, "segment-1"),
        (10., "segment-1"),
    ] {
        assert_eq!(valid.at_station(s).unwrap().assignment.id, id);
    }
    for s in [-1., 10.0001, f64::NAN, f64::INFINITY] {
        assert!(valid.at_station(s).is_err());
    }
}
#[test]
fn double_spine_boundary_between_samples_retains_exact_rail_meshes() {
    let t = track();
    let v = variants();
    let m = map(10., &[4.25]);
    let out = build(&t, &m, &v);
    let baseline = construction::build(&t, &v[0].definition, Default::default()).unwrap();
    assert_eq!(rails(&out.mesh), rails(&baseline));
    assert_eq!(
        out.mesh
            .components
            .iter()
            .filter(|c| c.role == Role::Structure)
            .count(),
        3
    );
    assert_eq!(out.boundaries.len(), 1);
    assert_eq!(out.boundaries[0].station_m, 4.25);
    assert!(
        out.boundaries[0]
            .message
            .contains("explicit transition asset")
    );
    for c in &out.mesh.components {
        c.mesh.validate().unwrap();
        if c.role == Role::Structure {
            let (start, end) = if c.id.starts_with("segment-0::") {
                (0., 4.25)
            } else {
                (4.25, 10.)
            };
            assert_eq!(
                c.mesh
                    .vertices
                    .iter()
                    .map(|p| p.station_m)
                    .fold(f64::INFINITY, f64::min),
                start
            );
            assert_eq!(
                c.mesh
                    .vertices
                    .iter()
                    .map(|p| p.station_m)
                    .fold(f64::NEG_INFINITY, f64::max),
                end
            );
            assert!(
                c.mesh
                    .vertices
                    .iter()
                    .any(|p| p.station_m == 4.25 && p.normal.x.abs() > 0.99)
            );
        }
    }
    // No changes to station samples/Region lengths or design-time state.
    assert_eq!(t, track());
}
#[test]
fn boundaries_near_start_and_end_do_not_make_zero_length_geometry() {
    let t = track();
    let v = variants();
    for cuts in [vec![0.0001], vec![9.9999], vec![0.0001, 9.9999]] {
        let out = build(&t, &map(10., &cuts), &v);
        for c in &out.mesh.components {
            c.mesh.validate().unwrap();
        }
        assert_eq!(rails(&out.mesh).len(), 2);
    }
}
#[test]
fn invalid_intervals_and_missing_definitions_report_actionable_errors() {
    let v = variants();
    for (start, end, expected) in [
        (3.9, 10., "overlap"),
        (4.1, 10., "gap"),
        (4., 9.9, "final coverage"),
        (4., 4., "invalid ID or interval"),
        (4., 11., "invalid ID or interval"),
        (f64::NAN, 10., "invalid ID or interval"),
    ] {
        let mut m = map(10., &[4.]);
        m.segments[1].start_m = start;
        m.segments[1].end_m = end;
        assert!(
            m.validate(10., &v)
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
    }
    let mut m = map(10., &[]);
    m.segments[0].variant_id = "unknown".into();
    assert!(
        m.validate(10., &v)
            .unwrap_err()
            .to_string()
            .contains("not registered")
    );
    m.segments[0].variant_id = "source-single".into();
    m.segments[0].style_id = "truss-100".into();
    assert!(m.validate(10., &v).is_err());
    assert!(map(10., &[4., 4.]).validate(10., &v).is_err());
    assert!(
        ConstructionMap {
            version: 1,
            segments: vec![]
        }
        .validate(10., &v)
        .is_err()
    );
}
#[test]
fn incompatible_gauge_rail_count_mount_profile_and_style_are_rejected() {
    for case in 0..5 {
        let mut v = variants();
        let mut m = map(10., &[4.]);
        match case {
            0 => v[1].definition.continuous[0].offset_m[2] -= 0.1,
            1 => v[1].definition = TrackConstruction::generic_single(),
            2 => v[1].definition.continuous[0].offset_m[1] += 0.1,
            3 => {
                v[1].definition.continuous[0].profile = Profile::Circle {
                    radius_m: 0.07,
                    sides: 12,
                }
            }
            _ => {
                v[1].style_id = "truss-100".into();
                m.segments[1].style_id = "truss-100".into();
            }
        }
        assert!(
            m.validate(10., &v)
                .unwrap_err()
                .to_string()
                .contains("incompatible running rails/style")
        );
    }
    let mut v = variants();
    v[1].definition.materials[0].roughness = 0.9;
    assert!(
        map(10., &[4.])
            .validate(10., &v)
            .unwrap_err()
            .to_string()
            .contains("conflicting material")
    );
}
#[test]
fn ties_keep_global_phase_and_rigid_footprints_do_not_cross_boundaries() {
    let t = track();
    let mut v = variants();
    for variant in &mut v {
        let c = &mut variant.definition.repeated[0];
        c.start_m = 0.75;
        c.spacing_m = 1.5;
        c.end_margin_m = 0.75;
    }
    let out = build(&t, &map(10., &[4.25]), &v);
    let placements: Vec<_> = out
        .mesh
        .components
        .iter()
        .filter(|c| c.role == Role::Hardware)
        .flat_map(|c| c.placements.iter().map(|p| p.station_m))
        .collect();
    assert_eq!(placements, vec![0.75, 2.25, 3.75, 5.25, 6.75, 8.25]);
    assert!(out.omitted_hardware.is_empty());
    // Tie centered exactly on a seam belongs to the right interval, but its
    // finite rigid footprint straddles it: omit with an explicit diagnostic.
    let out = build(&t, &map(10., &[3.75]), &v);
    assert_eq!(
        out.omitted_hardware,
        vec![("diagnostic-crossbar".into(), 3.75)]
    );
    assert!(
        !out.mesh
            .components
            .iter()
            .flat_map(|c| &c.placements)
            .any(|p| p.station_m == 3.75)
    );
    let out = build(&t, &map(10., &[3.76]), &v);
    assert_eq!(out.omitted_hardware, vec![("cross-tie".into(), 3.75)]);
}
#[test]
fn unchanged_adjacent_assignments_coalesce_without_caps_or_duplicate_ties() {
    let t = track();
    let v = variants();
    let mut m = map(10., &[4.25]);
    m.segments[1].variant_id = "source-single".into();
    let out = build(&t, &m, &v);
    let full = build(&t, &map(10., &[]), &v);
    assert_eq!(out.mesh, full.mesh);
    assert!(out.boundaries.is_empty());
}
#[test]
fn banking_inversion_curvature_normals_uv_and_support_references_remain_valid() {
    let mut section = Section::straight(1, "synthetic vertical loop", std::f64::consts::TAU * 12.);
    section.kind = SectionKind::Arc {
        radius_m: 12.,
        plane_angle_rad: 0.,
    };
    let initial = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, 0.4).unwrap(),
        ..Default::default()
    };
    let t = generate(&initial, &[section], Default::default()).unwrap();
    let snapshot = t.clone();
    let v = variants();
    let m = map(t.length_m(), &[15.25, 40.75]);
    let valid = m.validate(t.length_m(), &v).unwrap();
    let out = build(&t, &m, &v);
    assert!(t.samples.iter().any(|p| p.frame.normal().y < -0.9));
    let baseline = construction::build(&t, &v[0].definition, Default::default()).unwrap();
    assert_eq!(rails(&out.mesh), rails(&baseline));
    for c in &out.mesh.components {
        c.mesh.validate().unwrap();
    }
    for station in [0., 15.25, 40.75, t.length_m()] {
        let a = Attachment::at_station(&t, station, Vec3::new(0., -0.9, 0.1)).unwrap();
        assert_eq!(a.resolve(&t).unwrap(), a.resolve(&snapshot).unwrap());
        assert_eq!(
            segmented::attachment_reference(&t, &valid, station, None).unwrap(),
            t.sample_at_distance(station).unwrap()
        );
        assert!(
            segmented::attachment_reference(&t, &valid, station, Some("unverified-mount"))
                .unwrap_err()
                .to_string()
                .contains("not verified")
        );
    }
    assert_eq!(t, snapshot);
}
#[test]
fn repeatability_global_budgets_and_mismatched_track_length() {
    let t = track();
    let v = variants();
    let m = map(10., &[2.25, 4.25, 6.25, 8.25]);
    let valid = m.validate(10., &v).unwrap();
    let out = build(&t, &m, &v);
    assert_eq!(out.mesh, build(&t, &m, &v).mesh);
    for limits in [
        MeshLimits {
            max_vertices: out.mesh.stats.vertices - 1,
            ..Default::default()
        },
        MeshLimits {
            max_instances: 1,
            ..Default::default()
        },
        MeshLimits {
            max_bytes: 10,
            ..Default::default()
        },
        MeshLimits {
            max_indices: 10,
            ..Default::default()
        },
    ] {
        assert!(segmented::build(&t, &valid, limits, &NoAssets).is_err());
    }
    let shorter = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "short", 9.)],
        Default::default(),
    )
    .unwrap();
    assert!(segmented::build(&shorter, &valid, Default::default(), &NoAssets).is_err());
    let mut too_many = map(10., &[]);
    too_many.segments = vec![too_many.segments[0].clone(); MAX_SEGMENTS + 1];
    assert!(too_many.validate(10., &v).is_err());
}
#[test]
fn sidecar_strict_version_fields_size_and_no_defaults() {
    assert!(ConstructionMap::from_json(r#"{"version":1,"segments":[],"unknown":true}"#).is_err());
    assert!(ConstructionMap::from_json(r#"{"version":1}"#).is_err());
    assert!(ConstructionMap::from_json(&" ".repeat(MAX_MAP_BYTES + 1)).is_err());
    let v = variants();
    let mut m = map(10., &[]);
    m.version = 2;
    assert!(m.validate(10., &v).is_err());
}
