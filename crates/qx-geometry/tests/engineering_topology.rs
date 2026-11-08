use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    engineering,
    math::{Quaternion, Vec3},
    solver::generate,
    supports::*,
    topology::{self, CompletionSettings, LayoutMode},
};
fn arc(angle: f64) -> Section {
    let mut s = Section::straight(1, "Analytic horizontal circle", 20. * angle);
    s.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    s
}
#[test]
fn explicit_intent_and_endpoint_pose_define_classification() {
    let open = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "line", 30.)],
        Default::default(),
    )
    .unwrap();
    let d = topology::diagnose(&open).unwrap();
    assert!(!d.valid);
    assert_eq!(d.position_gap_m, 30.);
    assert_eq!(topology::status(LayoutMode::Open, d), "Open layout");
    assert_eq!(
        topology::status(LayoutMode::Shuttle, d),
        "Shuttle (open ends)"
    );
    assert_eq!(
        topology::status(LayoutMode::Circuit, d),
        "Circuit incomplete"
    );
    let mut circle = generate(
        &InitialConditions::default(),
        &[arc(std::f64::consts::TAU)],
        Default::default(),
    )
    .unwrap();
    assert!(topology::diagnose(&circle).unwrap().valid);
    circle.samples.last_mut().unwrap().position_m.x += 0.002;
    assert!(!topology::diagnose(&circle).unwrap().valid);
    circle.samples.last_mut().unwrap().position_m = circle.samples[0].position_m;
    circle.samples.last_mut().unwrap().frame =
        qx_geometry::math::Frame::new(Quaternion::axis_angle(Vec3::X, 0.01).unwrap()).unwrap();
    let roll = topology::diagnose(&circle).unwrap();
    assert!(roll.tangent_gap_rad < 1e-8 && !roll.valid);
    circle.samples.last_mut().unwrap().frame =
        qx_geometry::math::Frame::new(Quaternion::axis_angle(Vec3::Y, 0.01).unwrap()).unwrap();
    assert!(topology::diagnose(&circle).unwrap().tangent_gap_rad > 0.009);
}
#[test]
fn bounded_connector_closes_pose_without_mutating_source_and_fails_honestly() {
    let source = vec![arc(1.5 * std::f64::consts::PI)];
    let original = source.clone();
    let initial = InitialConditions::default();
    let c = topology::complete(
        &initial,
        &source,
        LayoutMode::Circuit,
        2,
        CompletionSettings {
            preferred_length_m: 31.5,
            minimum_radius_m: 10.,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert!(c.closure.valid);
    assert_eq!(source, original);
    assert!(c.attempts <= 41);
    let mut completed = source.clone();
    completed.push(c.section);
    let t = generate(&initial, &completed, Default::default()).unwrap();
    assert!(topology::diagnose(&t).unwrap().valid);
    assert!(
        topology::complete(
            &initial,
            &source,
            LayoutMode::Circuit,
            2,
            CompletionSettings {
                preferred_length_m: 1.,
                ..Default::default()
            },
            Default::default()
        )
        .is_err()
    );
    assert!(
        topology::complete(
            &initial,
            &source,
            LayoutMode::Shuttle,
            2,
            Default::default(),
            Default::default()
        )
        .is_err()
    );
    assert_eq!(source, original);
}
#[test]
fn shared_engineering_rows_match_line_and_independent_circle_solution() {
    let i = InitialConditions {
        reference_speed_mps: 10.,
        gravity_mps2: 9.81,
        ..Default::default()
    };
    let line = generate(
        &i,
        &[Section::straight(1, "straight", 10.)],
        Default::default(),
    )
    .unwrap();
    for r in engineering::snapshot(&line, 9.81).unwrap() {
        assert!(r.radius_m.is_none());
        assert!(r.acceleration_mps2.length() < 1e-12);
        assert!((r.geometry.normal_g - 1.).abs() < 1e-12);
        assert_eq!(r.longitudinal_g, 0.);
    }
    let circle = generate(&i, &[arc(2.)], Default::default()).unwrap();
    let rows = engineering::snapshot(&circle, 9.81).unwrap();
    assert_eq!(rows.len(), circle.graphs.len());
    for r in rows {
        assert!((r.radius_m.unwrap() - 20.).abs() < 0.0001);
        assert!((r.acceleration_mps2.length() - 5.).abs() < 0.0001);
        assert!((r.geometry.lateral_g - 5. / 9.81).abs() < 0.0001);
        assert!((r.geometry.normal_g - 1.).abs() < 1e-10);
        assert!(r.longitudinal_g.abs() < 1e-9);
    }
}
#[test]
fn timber_nodes_regenerate_with_section_and_solid_members_have_correct_transforms() {
    let mut initial = InitialConditions {
        position_m: Vec3::new(0., 8., 0.),
        ..Default::default()
    };
    let t = generate(
        &initial,
        &[Section::straight(1, "lead", 20.)],
        Default::default(),
    )
    .unwrap();
    let recipe = TimberRecipe {
        start_m: 2.,
        end_m: 18.,
        foundation_elevation_m: 0.,
        ..Default::default()
    };
    let s = timber(1, recipe.clone(), &t).unwrap();
    s.validate(&[qx_geometry::authoring::SectionId(1)]).unwrap();
    let lattice = timber(
        2,
        TimberRecipe {
            family: TimberFamily::HybridLattice,
            ..recipe
        },
        &t,
    )
    .unwrap();
    assert!(lattice.members.len() > s.members.len());
    let p = s.nodes[1].position.resolve(&t).unwrap();
    initial.position_m.x = 10.;
    initial.position_m.y = 12.;
    let updated = generate(
        &initial,
        &[Section::straight(1, "lead", 40.)],
        Default::default(),
    )
    .unwrap();
    let moved = s.nodes[1].position.resolve(&updated).unwrap();
    assert!((moved.x - p.x - 12.).abs() < 1e-8);
    assert!((moved.y - p.y - 4.).abs() < 1e-8);
    let foundation = s.nodes[0].position.resolve(&updated).unwrap();
    assert_eq!(foundation.y, 0.);
    assert_eq!(foundation.x, moved.x);
    let mut mesh =
        qx_geometry::construction::build(&updated, &Default::default(), Default::default())
            .unwrap();
    let n = mesh.components.len();
    append_mesh(&updated, &[s], &mut mesh).unwrap();
    assert!(mesh.components.len() > n);
    for c in &mesh.components {
        c.mesh.validate().unwrap();
    }
    let post = &mesh.components[n].mesh;
    assert!((post.bounds.max.y - post.bounds.min.y - 11.5).abs() < 1e-8);
    assert!(post.vertices.iter().all(|v| v.normal.is_finite()
        && (v.normal.length() - 1.).abs() < 1e-8
        && v.source_station_m < 0.));
}
#[test]
fn rider_authored_supports_attach_to_the_derived_physical_track() {
    let initial = InitialConditions {
        position_m: Vec3::new(0., 10., 0.),
        ..Default::default()
    };
    let source = generate(
        &initial,
        &[Section::straight(1, "rider path", 20.)],
        Default::default(),
    )
    .unwrap();
    let reference = qx_geometry::heartline::generate_reference(
        &source,
        &qx_geometry::heartline::OffsetProfile::Constant {
            offset_m: Vec3::Y * 2.,
        },
        qx_geometry::heartline::AuthoredReference::RiderReference,
        initial.gravity_mps2,
    )
    .unwrap();
    let physical = physical_attachment_track(&source, &reference).unwrap();
    let a = Attachment::at_station(&physical, 10., Vec3::ZERO).unwrap();
    let p = a.resolve(&physical).unwrap();
    assert!((p - Vec3::new(10., 8., 0.)).length() < 1e-8);
    assert_eq!(source.samples[0].position_m.y, 10.);
}
