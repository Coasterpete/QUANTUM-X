#[path = "support/rail_fixtures.rs"]
mod fixtures;
use fixtures::*;
use qx_geometry::{
    authoring::{InitialConditions, Section},
    math::{Frame, Quaternion, Vec3},
    rail_geometry::*,
    solver::{GenerationOptions, generate},
    topology::LayoutMode,
};
use qx_style::{
    construction::{Role, TrackConstruction},
    construction_segments::*,
};

fn path(offset: Vec3) -> WheelCenterPath {
    WheelCenterPath {
        rail: RailReference {
            id: "explicit-test-rail".into(),
            offset_m: Vec3::new(0., 0., 0.6),
        },
        center_offset_m: offset - Vec3::new(0., 0., 0.6),
    }
}
fn window(s: f64) -> StationWindow {
    StationWindow {
        min_m: s - 1.,
        max_m: s + 1.,
    }
}

#[test]
fn construction_offsets_are_references_not_automatic_surfaces() {
    let definition = TrackConstruction::generic_dual();
    let running: Vec<_> = definition
        .continuous
        .iter()
        .filter(|s| s.role == Role::Running)
        .collect();
    assert_eq!(running.len(), 2);
    let t = sampled(10., 1., |s| (Vec3::X * s, Quaternion::IDENTITY));
    let geometry = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    for rail in running {
        let reference = RailReference::from_construction(&definition, &rail.id).unwrap();
        assert_eq!(
            reference.offset_m,
            Vec3::new(rail.offset_m[0], rail.offset_m[1], rail.offset_m[2])
        );
        let p = WheelCenterPath {
            rail: reference,
            center_offset_m: Vec3::new(0., 0.18, 0.),
        };
        let sample = geometry.sample(&p, 3.).unwrap();
        assert!(
            (sample.rail_center_m
                - Vec3::new(3. + rail.offset_m[0], rail.offset_m[1], rail.offset_m[2]))
            .length()
                < 1e-12
        );
        assert!((sample.wheel_center_m - sample.rail_center_m - Vec3::Y * 0.18).length() < 1e-12);
    }
    assert!(RailReference::from_construction(&definition, "missing").is_err());
    let structure = definition
        .continuous
        .iter()
        .find(|s| s.role == Role::Structure)
        .unwrap();
    assert!(RailReference::from_construction(&definition, &structure.id).is_err());
    let variants = vec![ConstructionVariant {
        style_id: "synthetic".into(),
        id: "one".into(),
        diagnostic_only: true,
        definition: definition.clone(),
    }];
    let map = ConstructionMap {
        version: 1,
        segments: vec![ConstructionSegment {
            id: "s".into(),
            start_m: 0.,
            end_m: 10.,
            style_id: "synthetic".into(),
            variant_id: "one".into(),
        }],
    };
    let map = map.validate(10., &variants).unwrap();
    let id = &definition
        .continuous
        .iter()
        .find(|s| s.role == Role::Running)
        .unwrap()
        .id;
    assert_eq!(
        RailReference::from_map(&map, 10., id).unwrap(),
        RailReference::from_construction(&definition, id).unwrap()
    );
    assert!(RailReference::from_map(&map, 9., id).is_err());
}

#[test]
fn offset_tangents_include_twist_and_small_angle_interpolation() {
    // Both branches of Quaternion::slerp, including antipodal sample quaternions.
    for rate in [0.002, 0.8] {
        let mut t = sampled(10., 0.5, |s| {
            (
                Vec3::X * s,
                Quaternion::axis_angle(Vec3::X, rate * s).unwrap(),
            )
        });
        let q = t.samples[9].frame.orientation();
        t.samples[9].frame = Frame::new(Quaternion {
            w: -q.w,
            x: -q.x,
            y: -q.y,
            z: -q.z,
        })
        .unwrap();
        let geometry = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
        let p = path(Vec3::new(0., 0.2, 0.7));
        let s = 4.185;
        let q = geometry.sample(&p, s).unwrap();
        let b = rate * s;
        let expected = Vec3::new(
            s,
            0.2 * b.cos() - 0.7 * b.sin(),
            0.2 * b.sin() + 0.7 * b.cos(),
        );
        assert!((q.wheel_center_m - expected).length() < 2e-11);
        let exact = Vec3::new(
            1.,
            rate * (-0.2 * b.sin() - 0.7 * b.cos()),
            rate * (0.2 * b.cos() - 0.7 * b.sin()),
        );
        assert!((q.derivative - exact).length() < 2e-10);
        // Independent existing sampler, not the new path evaluator's derivative.
        let at = |x| {
            let v = t.sample_at_distance(x).unwrap();
            v.position_m + v.frame.local_to_world(Vec3::new(0., 0.2, 0.7))
        };
        let fd = (at(s + 1e-5) - at(s - 1e-5)) / 2e-5;
        assert!((q.derivative - fd).length() < 1e-9);
        assert!((q.tangent - Vec3::X).length() > rate * 0.5);
        // Actual nearest station is displaced by the longitudinal component of
        // a cross-frame displacement, unlike a centerline-station projection.
        let point = q.wheel_center_m + Vec3::Y * 0.03;
        let near = geometry
            .nearest(&p, point, window(s), SearchOptions::default())
            .unwrap();
        assert_eq!(near.status, SearchStatus::Resolved);
        assert!((near.candidates[0].sample.station_m - s).abs() > rate * 0.005);
    }
}

#[test]
fn analytic_curves_helices_and_varying_twist_have_independent_targets() {
    for (radius, rise, twist, accel) in [
        (3., 0., 0., 0.),
        (8., 0.3, 0.04, 0.),
        (8., 0.3, 0.02, 0.001),
    ] {
        let bank = |s: f64| 0.4 + twist * s + accel * s * s;
        let t = sampled(30., 0.002, |s| helix(s, radius, rise, bank(s)));
        let geometry = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
        let offset = Vec3::new(0., 0.2, 0.65);
        let p = path(offset);
        for s in [2.013, 8.187, 15.653, 27.351] {
            let expected = helix_locus(s, radius, rise, bank(s), offset);
            let actual = geometry.sample(&p, s).unwrap();
            let position_error = (actual.wheel_center_m - expected).length();
            assert!(position_error < 2e-7, "position {position_error:e}");
            let fd = (helix_locus(s + 1e-5, radius, rise, bank(s + 1e-5), offset)
                - helix_locus(s - 1e-5, radius, rise, bank(s - 1e-5), offset))
                / 2e-5;
            assert!((actual.derivative - fd).length() < 2e-5);
            let target = geometry
                .nearest(&p, expected, window(s), SearchOptions::default())
                .unwrap();
            assert_eq!(target.status, SearchStatus::Resolved);
            let c = &target.candidates[0];
            assert!(c.separation_m < 2.1e-6);
            assert!((c.sample.station_m - s).abs() < 3e-6);
            println!(
                "helix R={radius} rise={rise} twist={twist} accel={accel} s={s}: position={position_error:.6e} station_error={:.6e} wheel_error={:.6e} segments={}",
                (c.sample.station_m - s).abs(),
                c.perpendicular_error_m,
                target.segments_tested
            );
        }
    }
}

#[test]
fn vertical_loop_and_inversion_keep_rail_relative_axes() {
    let r = 6.;
    let t = sampled(std::f64::consts::TAU * r, 0.03, |s| {
        let a = s / r;
        (
            Vec3::new(r * a.sin(), r * (1. - a.cos()), 0.),
            Quaternion::axis_angle(Vec3::Z, a).unwrap(),
        )
    });
    let geometry = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let p = path(Vec3::new(0., 0.2, 0.6));
    for angle in [
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    ] {
        let s = r * angle;
        let n = Vec3::new(-angle.sin(), angle.cos(), 0.);
        let expected = Vec3::new((r - 0.2) * angle.sin(), r - (r - 0.2) * angle.cos(), 0.6);
        let q = geometry.sample(&p, s).unwrap();
        assert!((q.wheel_center_m - expected).length() < 1e-9);
        let point = expected + n * 0.04 + Vec3::Z * 0.03;
        let near = geometry
            .nearest(&p, point, window(s), SearchOptions::default())
            .unwrap();
        assert_eq!(near.status, SearchStatus::Resolved);
        let c = &near.candidates[0];
        assert!((c.perpendicular_error_m - 0.05).abs() < 2e-8);
        assert!((c.vertical_error_m.unwrap() - 0.04).abs() < 2e-8);
        assert!((c.lateral_error_m.unwrap() - 0.03).abs() < 1e-12);
        assert!((c.sample.station_m - s).abs() < 2e-5);
    }
}

#[test]
fn bounded_search_reports_remote_branches_endpoints_and_missing_precision() {
    let r = 3.;
    let circumference = std::f64::consts::TAU * r;
    let t = sampled(circumference * 2., 0.02, |s| helix(s, r, 0., 0.));
    let geometry = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let p = path(Vec3::new(0., 0., 0.6));
    let point = Vec3::new(r * 1_f64.sin(), 0., r - (r - 0.6) * 1_f64.cos())
        + Vec3::new(-0.6 * 1_f64.sin(), 0., 0.);
    let options = SearchOptions {
        max_span_m: 30.,
        max_segments: 16384,
        ..Default::default()
    };
    let near = geometry
        .nearest(
            &p,
            point,
            StationWindow {
                min_m: 1.,
                max_m: circumference + 5.,
            },
            options,
        )
        .unwrap();
    assert_eq!(near.status, SearchStatus::Ambiguous);
    assert_eq!(near.candidates.len(), 2);
    assert!((near.candidates[0].sample.station_m - 3.).abs() < 3e-6);
    assert!((near.candidates[1].sample.station_m - 3. - circumference).abs() < 3e-6);
    let local = geometry.nearest(&p, point, window(3.), options).unwrap();
    assert_eq!(local.status, SearchStatus::Resolved);
    let line = sampled(10., 1., |s| (Vec3::X * s, Quaternion::IDENTITY));
    let g = PhysicalRailGeometry::new(&line, LayoutMode::Open).unwrap();
    let p = path(Vec3::ZERO);
    let end = g
        .nearest(
            &p,
            Vec3::X * 12.,
            StationWindow {
                min_m: 8.,
                max_m: 10.,
            },
            options,
        )
        .unwrap();
    assert_eq!(end.status, SearchStatus::BoundaryMinimum);
    assert_eq!(end.candidates[0].perpendicular_error_m, 0.);
    assert_eq!(end.candidates[0].separation_m, 2.);
    assert!(
        g.nearest(
            &p,
            Vec3::ZERO,
            StationWindow {
                min_m: -1.,
                max_m: 1.
            },
            options
        )
        .is_err()
    );
    assert!(
        geometry
            .nearest(
                &p,
                point,
                window(3.),
                SearchOptions {
                    max_segments: 1,
                    ..options
                }
            )
            .is_err()
    );
    let huge = sampled(10., 1., |s| {
        (Vec3::new(1e9 + s, 0., 0.), Quaternion::IDENTITY)
    });
    let huge = PhysicalRailGeometry::new(&huge, LayoutMode::Open).unwrap();
    assert!(
        huge.nearest(&p, Vec3::X * 1e9, window(3.), options)
            .is_err()
    );
}

#[test]
fn topology_and_invalid_geometry_are_explicit() {
    let length = std::f64::consts::TAU * 8.;
    let t = sampled(length, 0.03, |s| helix(s, 8., 0., 0.));
    let p = path(Vec3::new(0., 0.2, 0.6));
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Circuit).unwrap();
    let point = helix_locus(-0.2, 8., 0., 0., Vec3::new(0., 0.2, 0.6));
    let n = g
        .nearest(
            &p,
            point,
            StationWindow {
                min_m: -1.,
                max_m: 1.,
            },
            Default::default(),
        )
        .unwrap();
    assert_eq!(n.status, SearchStatus::Resolved);
    assert!((n.candidates[0].sample.station_m + 0.2).abs() < 3e-6);
    for mode in [LayoutMode::Open, LayoutMode::Shuttle] {
        let open = PhysicalRailGeometry::new(&t, mode).unwrap();
        assert!(open.sample(&p, -0.2).is_err());
    }
    let mut gap = t.clone();
    gap.samples.last_mut().unwrap().position_m.x += 0.0005;
    let gap = PhysicalRailGeometry::new(&gap, LayoutMode::Circuit).unwrap();
    assert!(
        gap.nearest(
            &p,
            point,
            StationWindow {
                min_m: -1.,
                max_m: 1.
            },
            Default::default()
        )
        .is_err()
    );
    let mut bad = t.clone();
    bad.samples[1].distance_m = 0.;
    assert!(PhysicalRailGeometry::new(&bad, LayoutMode::Open).is_err());
    bad = t.clone();
    bad.samples[1].position_m.x = f64::NAN;
    assert!(PhysicalRailGeometry::new(&bad, LayoutMode::Open).is_err());
    let line = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "physical", 10.)],
        GenerationOptions::default(),
    )
    .unwrap();
    assert!(PhysicalRailGeometry::new(&line, LayoutMode::Circuit).is_err());
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let mut bad_path = p;
    bad_path.center_offset_m.y = f64::INFINITY;
    assert!(g.sample(&bad_path, 1.).is_err());
}

#[test]
fn derived_physical_reference_is_used_instead_of_authored_rider_path() {
    use qx_geometry::heartline::{AuthoredReference, OffsetProfile, generate_reference};
    let rider = generate(
        &InitialConditions {
            position_m: Vec3::Y * 3.,
            ..Default::default()
        },
        &[Section::straight(1, "authored rider reference", 10.)],
        GenerationOptions::default(),
    )
    .unwrap();
    let reference = generate_reference(
        &rider,
        &OffsetProfile::Constant {
            offset_m: Vec3::Y * 1.2,
        },
        AuthoredReference::RiderReference,
        9.80665,
    )
    .unwrap();
    let physical = qx_geometry::supports::physical_attachment_track(&rider, &reference).unwrap();
    let g = PhysicalRailGeometry::new(&physical, LayoutMode::Open).unwrap();
    let q = g.sample(&path(Vec3::new(0., 0.2, 0.6)), 4.321).unwrap();
    assert!((q.rail_center_m - Vec3::new(4.321, 1.8, 0.6)).length() < 1e-12);
    assert!((q.wheel_center_m - Vec3::new(4.321, 2., 0.6)).length() < 1e-12);
    assert_eq!(rider.samples[0].position_m.y, 3.);
}

#[test]
fn small_angle_derivative_matches_actual_nlerp_and_flat_minima_are_not_unique() {
    for rate in [1e-9, 0.1] {
        let t = sampled(10., 0.5, |s| {
            (
                Vec3::X * s,
                Quaternion::axis_angle(Vec3::X, rate * s).unwrap(),
            )
        });
        let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
        let p = path(Vec3::new(0., 0.2, 0.7));
        let s = 4.185;
        let actual = g.sample(&p, s).unwrap();
        let at = |s| {
            let a = t.sample_at_distance(s).unwrap();
            a.position_m + a.frame.local_to_world(Vec3::new(0., 0.2, 0.7))
        };
        let fd = (at(s + 1e-5) - at(s - 1e-5)) / 2e-5;
        assert!((actual.derivative - fd).length() < 1e-9);
        assert!(actual.derivative.z.abs() > rate * 0.1);
    }
    let t = sampled(10., 0.01, |s| helix(s, 3., 0., 0.));
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let n = g
        .nearest(
            &path(Vec3::ZERO),
            Vec3::new(0., 0., 3.),
            StationWindow {
                min_m: 2.,
                max_m: 4.,
            },
            Default::default(),
        )
        .unwrap();
    assert_eq!(n.status, SearchStatus::Ambiguous);
}

#[test]
fn abrupt_banking_derivative_exposes_both_tangents_at_offset_path_corner() {
    let t = sampled(20., 0.1, |s| {
        (
            Vec3::X * s,
            Quaternion::axis_angle(Vec3::X, (s - 10.).max(0.) * 0.8).unwrap(),
        )
    });
    let g = PhysicalRailGeometry::new(&t, LayoutMode::Open).unwrap();
    let p = path(Vec3::new(0., 0.2, 0.7));
    let sample = g.sample(&p, 10.).unwrap();
    assert_eq!(sample.incoming_tangent, Some(Vec3::X));
    assert!((sample.tangent - Vec3::X).length() > 0.4);
    let n = g
        .nearest(
            &p,
            Vec3::new(10., 0.2, 0.7),
            window(10.),
            Default::default(),
        )
        .unwrap();
    assert_eq!(n.status, SearchStatus::Resolved);
    let c = &n.candidates[0];
    assert_eq!(c.sample.station_m, 10.);
    assert!(c.sample.incoming_tangent.is_some());
    assert!(c.vertical_error_m.is_none() && c.lateral_error_m.is_none());
    assert_eq!(c.separation_m, c.perpendicular_error_m);
}
