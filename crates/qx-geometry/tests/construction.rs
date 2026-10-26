use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    construction::{MeshLimits, build},
    math::{Quaternion, Vec3},
    solver::{GenerationOptions, generate},
};
use qx_style::construction::{Alignment, HardwareShape, Profile, Role, TrackConstruction};
fn straight() -> qx_geometry::solver::GeneratedTrack {
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
#[test]
fn exact_offsets_counts_caps_bounds_ids_and_determinism() {
    let t = straight();
    let before = t.clone();
    let c = TrackConstruction::generic_dual();
    let m = build(&t, &c, Default::default()).unwrap();
    assert_eq!(t, before);
    assert_eq!(m, build(&t, &c, Default::default()).unwrap());
    assert_eq!(m.components.len(), 4);
    let n = t.samples.len();
    for (rail, offset) in m.components[..2].iter().zip([-0.55, 0.55]) {
        assert_eq!(rail.role, Role::Running);
        assert_eq!(rail.material, "running");
        assert_eq!(rail.mesh.vertices.len(), n * 24 + 24);
        assert_eq!(rail.mesh.indices.len(), (n - 1) * 72 + 60);
        let p = rail.mesh.vertices[0].position_m;
        assert!((p - Vec3::new(0., 0.065, offset)).length() < 1e-14);
        assert!(
            rail.mesh
                .vertices
                .iter()
                .all(|v| v.position_m.x >= 0. && v.position_m.x <= 10.)
        );
        rail.mesh.validate().unwrap();
        for i in 0..n {
            assert_eq!(rail.mesh.vertices[i * 24].uv[1], t.samples[i].distance_m);
        }
    }
    assert_eq!(m.components[0].id, "rail-left");
    assert_eq!(m.components[1].id, "rail-right");
    assert!(m.bounds.min.z <= -0.615 && m.bounds.max.z >= 0.615);
    // Rings cross the shared section sample exactly once, without internal caps.
    assert_eq!(t.sections[0].end_sample, t.sections[1].start_sample);
    assert_eq!(
        m.components[0]
            .mesh
            .vertices
            .iter()
            .filter(|v| v.station_m == 4.)
            .count(),
        24
    );
}
#[test]
fn monorail_is_one_concave_beam_with_separate_hardware() {
    let m = build(
        &straight(),
        &TrackConstruction::generic_single(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        m.components
            .iter()
            .filter(|c| c.role == Role::Running)
            .count(),
        1
    );
    assert_eq!(m.components.len(), 2);
    assert_eq!(m.components[0].id, "primary-beam");
    m.components[0].mesh.validate().unwrap();
    let mut independent = TrackConstruction::generic_single();
    independent.continuous[0].profile = Profile::Box {
        height_m: 0.3,
        width_m: 0.8,
    };
    assert_ne!(
        m.components[0].mesh,
        build(&straight(), &independent, Default::default())
            .unwrap()
            .components[0]
            .mesh
    );
}
#[test]
fn hardware_uses_arc_distance_unique_stations_and_slerped_frames() {
    let mut c = TrackConstruction::generic_dual();
    c.repeated[0].start_m = 0.;
    c.repeated[0].end_margin_m = 0.;
    c.repeated[0].spacing_m = 2.;
    let mut a = Section::straight(1, "loop", std::f64::consts::TAU * 8.);
    a.kind = SectionKind::Arc {
        radius_m: 8.,
        plane_angle_rad: 0.,
    };
    let t = generate(
        &InitialConditions::default(),
        &[a],
        GenerationOptions::default(),
    )
    .unwrap();
    let m = build(&t, &c, Default::default()).unwrap();
    let hw = m.components.last().unwrap();
    for (i, p) in hw.placements.iter().enumerate() {
        assert_eq!(p.station_m, i as f64 * 2.);
        let s = t.sample_at_distance(p.station_m).unwrap();
        assert!((p.position_m - (s.position_m + s.frame.normal() * -0.16)).length() < 1e-12);
        assert!((p.frame.normal() - s.frame.normal()).length() < 1e-12);
    }
    assert!(hw.placements.iter().any(|p| p.frame.normal().y < -0.95));
    // Identical placement count despite a radically different solver sample count.
    let mut coarse = t.clone();
    coarse.samples = t.samples.iter().step_by(3).cloned().collect();
    if coarse.samples.last() != t.samples.last() {
        coarse.samples.push(t.samples.last().unwrap().clone());
    }
    let cm = build(&coarse, &c, Default::default()).unwrap();
    assert_eq!(
        hw.placements.len(),
        cm.components.last().unwrap().placements.len()
    );
    let m = build(&straight(), &c, Default::default()).unwrap();
    let stations: Vec<_> = m
        .components
        .last()
        .unwrap()
        .placements
        .iter()
        .map(|p| p.station_m)
        .collect();
    assert_eq!(stations, vec![0., 2., 4., 6., 8., 10.]); // join at 4 occurs once
}
#[test]
fn banked_vertical_compound_frames_and_large_world_coordinates_stay_local() {
    for rotation in [
        Quaternion::axis_angle(Vec3::X, 2.8).unwrap(),
        Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap(),
        Quaternion::axis_angle(Vec3::Y, 0.8)
            .unwrap()
            .compose(Quaternion::axis_angle(Vec3::X, 2.4).unwrap()),
    ] {
        let initial = InitialConditions {
            orientation: rotation,
            position_m: Vec3::new(5e8, -5e8, 5e8),
            ..Default::default()
        };
        let t = generate(
            &initial,
            &[Section::straight(1, "precision", 10.)],
            Default::default(),
        )
        .unwrap();
        let m = build(&t, &TrackConstruction::generic_single(), Default::default()).unwrap();
        let source = t.samples[0].frame;
        let local =
            source.world_to_local(m.components[0].mesh.vertices[0].position_m - initial.position_m);
        assert!((local - Vec3::new(0., -0.45, -0.13)).length() < 3e-7);
        assert!(
            m.components[0]
                .mesh
                .vertices
                .iter()
                .all(|v| (v.normal.length() - 1.).abs() < 1e-10)
        );
    }
}
#[test]
fn malformed_profiles_dimensions_materials_assets_and_resource_limits_reject() {
    let t = straight();
    let base = TrackConstruction::generic_dual();
    for profile in [
        Profile::Circle {
            radius_m: -1.,
            sides: 12,
        },
        Profile::Box {
            height_m: f64::NAN,
            width_m: 1.,
        },
        Profile::Polygon {
            points: vec![[0., 0.], [0., 1.], [1., 0.]],
        },
        Profile::Polygon {
            points: vec![[0., 0.], [1., 1.], [0., 1.], [1., 0.]],
        },
        Profile::Polygon {
            points: vec![[0., 0.], [1., 0.], [1., 0.], [0., 1.]],
        },
    ] {
        let mut c = base.clone();
        c.continuous[0].profile = profile;
        assert!(build(&t, &c, Default::default()).is_err());
    }
    let mut c = base.clone();
    c.materials.clear();
    assert!(build(&t, &c, Default::default()).is_err());
    let mut c = base.clone();
    c.repeated[0].shape = HardwareShape::Asset {
        id: "bracket".into(),
        path: "../secret.glb".into(),
        version: 1,
        scale: [1.; 3],
    };
    assert!(c.validate().is_err());
    let HardwareShape::Asset { path, .. } = &mut c.repeated[0].shape else {
        unreachable!()
    };
    *path = "components/bracket.glb".into();
    assert!(
        build(&t, &c, Default::default())
            .unwrap_err()
            .to_string()
            .contains("unresolved component asset")
    );
    for limits in [
        MeshLimits {
            max_vertices: 10,
            ..Default::default()
        },
        MeshLimits {
            max_indices: 10,
            ..Default::default()
        },
        MeshLimits {
            max_instances: 1,
            ..Default::default()
        },
        MeshLimits {
            max_bytes: 1,
            ..Default::default()
        },
    ] {
        assert!(build(&t, &base, limits).is_err());
    }
    let mut c = base;
    c.continuous.remove(0);
    assert!(c.validate().is_err());
}
#[test]
fn corrupt_samples_tiny_segments_and_frame_jumps_reject_without_mutation() {
    let base = straight();
    let c = TrackConstruction::generic_dual();
    for case in 0..5 {
        let mut t = base.clone();
        match case {
            0 => t.samples[1].distance_m = 0.,
            1 => t.samples[1].position_m.x = f64::INFINITY,
            2 => t.samples[1].position_m = t.samples[0].position_m,
            3 => t.samples[1].distance_m = 1e-12,
            _ => {
                t.samples[1].frame =
                    qx_geometry::math::Frame::new(Quaternion::axis_angle(Vec3::X, 2.).unwrap())
                        .unwrap()
            }
        };
        assert!(build(&t, &c, Default::default()).is_err());
    }
    let mut c = c;
    c.repeated[0].alignment = Alignment::WorldAxes;
    c.repeated[0].rotation_rad = [0., 0., 0.3];
    assert!(build(&base, &c, Default::default()).is_ok());
}
#[test]
fn closed_profiles_have_watertight_positional_topology_and_corrupt_indices_fail() {
    use std::collections::BTreeMap;
    for config in [
        TrackConstruction::generic_dual(),
        TrackConstruction::generic_single(),
    ] {
        let mesh = build(&straight(), &config, Default::default()).unwrap();
        for c in &mesh.components {
            let mut keys = BTreeMap::new();
            let mut edges = BTreeMap::<(usize, usize), usize>::new();
            let mut ids = vec![];
            for v in &c.mesh.vertices {
                let key = (
                    (v.position_m.x * 1e8).round() as i64,
                    (v.position_m.y * 1e8).round() as i64,
                    (v.position_m.z * 1e8).round() as i64,
                );
                let next = keys.len();
                ids.push(*keys.entry(key).or_insert(next));
            }
            for t in c.mesh.indices.as_chunks::<3>().0 {
                let [a, b, d] = t.map(|i| ids[i as usize]);
                for (a, b) in [(a, b), (b, d), (d, a)] {
                    *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                }
            }
            assert!(
                edges.values().all(|n| *n == 2),
                "{} has a boundary or nonmanifold edge",
                c.id
            );
            let mut corrupt = c.mesh.clone();
            corrupt.indices[0] = u32::MAX;
            assert!(corrupt.validate().is_err());
            let mut corrupt = c.mesh.clone();
            corrupt.indices.swap(0, 1);
            assert!(corrupt.validate().is_err());
        }
    }
}
