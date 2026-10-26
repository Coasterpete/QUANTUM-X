use qx_document::{
    command::{Command, Editor},
    persistence,
    sample::*,
};
use qx_geometry::{
    authoring::{ChannelKey, SectionId, SectionKind},
    curve::{ControlPoint, Curve, clamped_knots},
    curve_edit::CurveEdit,
    math::Vec3,
    solver::GenerationOptions,
};
use qx_style::Catalog;
#[test]
fn all_fixtures_round_trip_and_regenerate_deterministically() {
    let catalog = Catalog::built_in().unwrap();
    for project in [
        spline_comparison_project(),
        mixed_authoring_project(),
        numerical_reference_project(),
    ] {
        let track = project
            .generate(&catalog, GenerationOptions::default())
            .unwrap();
        let reopened =
            persistence::from_json(&persistence::to_json(&project, &catalog).unwrap(), &catalog)
                .unwrap();
        assert_eq!(project, reopened);
        assert_eq!(
            track,
            reopened
                .generate(&catalog, GenerationOptions::default())
                .unwrap()
        );
        assert_eq!(track.sections.len(), project.sections.len());
        assert_eq!(track.graphs.len() + 1, track.samples.len());
        assert!(
            track
                .samples
                .windows(2)
                .all(|w| w[1].distance_m > w[0].distance_m)
        );
        for s in &track.samples {
            let f = s.frame;
            assert!((f.tangent().cross(f.normal()) - f.lateral()).length() < 1e-12);
        }
        for s in &project.sections {
            if matches!(s.kind, SectionKind::Spline { .. }) {
                assert!(
                    track
                        .graphs
                        .iter()
                        .filter(|g| g.section_id == s.id)
                        .all(|g| g.authored_normal_g.is_none() && g.authored_lateral_g.is_none())
                );
            }
        }
    }
}
#[test]
fn reference_project_has_independently_known_length_and_circle_load() {
    let p = numerical_reference_project();
    let t = p
        .generate(&Catalog::built_in().unwrap(), GenerationOptions::default())
        .unwrap();
    assert!((t.length_m() - (80. + 10. * std::f64::consts::PI)).abs() < 1e-8);
    assert!(
        t.graphs
            .iter()
            .filter(|g| g.section_id == SectionId(3))
            .all(|g| (g.curvature_per_m - 0.05).abs() < 1e-7)
    );
}
#[test]
fn editing_is_transactional_and_undo_restores_generated_geometry() {
    let catalog = Catalog::built_in().unwrap();
    let p = spline_comparison_project();
    let original = p.generate(&catalog, GenerationOptions::default()).unwrap();
    let mut editor = Editor::new(p.clone(), catalog.clone(), 16).unwrap();
    let edit = |section_id, edit| Command::EditSpline {
        section_id: SectionId(section_id),
        edit,
    };
    editor
        .execute([
            edit(
                100,
                CurveEdit::MovePoint {
                    id: 3,
                    position_m: Vec3::new(13., 5., 1.),
                },
            ),
            edit(
                100,
                CurveEdit::FitBezierJoin {
                    endpoint_id: 4,
                    continuity: qx_geometry::curve_edit::BezierContinuity::C1,
                },
            ),
        ])
        .unwrap();
    assert_ne!(
        original,
        editor
            .project()
            .generate(&catalog, GenerationOptions::default())
            .unwrap()
    );
    assert!(editor.undo());
    assert_eq!(editor.project(), &p);
    assert_eq!(
        original,
        editor
            .project()
            .generate(&catalog, GenerationOptions::default())
            .unwrap()
    );
    assert!(editor.redo());
    let checkpoint = editor.project().clone();
    assert!(
        editor
            .execute([edit(
                100,
                CurveEdit::MovePoint {
                    id: 1,
                    position_m: Vec3::Y
                }
            )])
            .is_err()
    );
    assert_eq!(editor.project(), &checkpoint);
    assert!(
        editor
            .execute([edit(102, CurveEdit::SetWeight { id: 2, weight: -1. })])
            .is_err()
    );
    assert_eq!(editor.project(), &checkpoint);
    editor
        .execute([
            edit(
                101,
                CurveEdit::InsertKnot {
                    parameter: 0.37,
                    new_id: 900,
                },
            ),
            edit(102, CurveEdit::SetWeight { id: 2, weight: 0.8 }),
        ])
        .unwrap();
    assert!(editor.undo());
    assert_eq!(editor.project(), &checkpoint);
    assert!(editor.redo());
    let before_roll = editor.project().sections[1].roll_rate.clone();
    editor
        .execute([edit(
            101,
            CurveEdit::MovePoint {
                id: 4,
                position_m: Vec3::new(21., 9., 5.),
            },
        )])
        .unwrap();
    assert_eq!(before_roll, editor.project().sections[1].roll_rate);
    // Unsupported topology changes fail the whole batch, without changing history.
    let before = editor.project().clone();
    assert!(
        editor
            .execute([edit(101, CurveEdit::RemovePoint { id: 2 })])
            .is_err()
    );
    assert_eq!(editor.project(), &before);
    editor
        .execute([Command::EditChannel {
            section_id: SectionId(100),
            channel: ChannelKey::Roll,
            edit: qx_document::command::ChannelEdit::Replace {
                transition: qx_geometry::transition::Transition::constant(1, 1., 0.1),
            },
        }])
        .unwrap();
    editor
        .execute([Command::ValidateConnections {
            options: GenerationOptions::default(),
        }])
        .unwrap();
}
#[test]
fn coordinated_control_insertion_degree_change_conversion_and_removal() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = numerical_reference_project();
    p.sections.truncate(1);
    let mut editor = Editor::new(p.clone(), catalog, 8).unwrap();
    let e = |edit| Command::EditSpline {
        section_id: SectionId(1),
        edit,
    };
    editor.execute([e(CurveEdit::ConvertToNurbs)]).unwrap();
    assert!(matches!(
        editor.project().sections[0].kind,
        SectionKind::Spline {
            curve: Curve::Nurbs { .. }
        }
    ));
    editor
        .execute([
            e(CurveEdit::InsertPoint {
                after: Some(3),
                point: ControlPoint {
                    id: 10,
                    position_m: Vec3::new(25., 0., 0.),
                },
                weight: 1.,
            }),
            e(CurveEdit::SetKnots(clamped_knots(5, 3).unwrap())),
        ])
        .unwrap();
    editor
        .execute([e(CurveEdit::SetDegree {
            degree: 2,
            knots: clamped_knots(5, 2).unwrap(),
        })])
        .unwrap();
    editor
        .execute([
            e(CurveEdit::RemovePoint { id: 10 }),
            e(CurveEdit::SetKnots(clamped_knots(4, 2).unwrap())),
        ])
        .unwrap();
    if let SectionKind::Spline { curve } = &editor.project().sections[0].kind {
        assert_eq!(curve.points().len(), 4);
    } else {
        panic!("spline expected");
    }
}
#[test]
fn version_one_migration_preserves_pass1_output_and_rejects_new_data() {
    let catalog = Catalog::built_in().unwrap();
    let fixture = include_str!("../../../examples/foundation.qx.json");
    let migrated = persistence::from_json(fixture, &catalog).unwrap();
    assert_eq!(migrated.schema_version, qx_document::SCHEMA_VERSION);
    assert_eq!(migrated, foundation_project());
    let baseline = migrated
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    assert_eq!(baseline.samples.len(), 1521);
    assert_eq!(baseline.diagnostics.len(), 3);
    assert!(
        (baseline.samples.last().unwrap().position_m
            - Vec3::new(114.04327140833351, 22.251725311225908, 75.46754437004525))
        .length()
            < 1e-12
    );
    let json = persistence::to_json(&spline_comparison_project(), &catalog)
        .unwrap()
        .replacen("\"schema_version\": 4", "\"schema_version\": 1", 1);
    assert!(persistence::from_json(&json, &catalog).is_err());
}
#[test]
fn transported_frames_cross_inflections_and_normalized_roll_survives_length_edits() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = spline_comparison_project();
    p.sections.truncate(1);
    let t = p.generate(&catalog, GenerationOptions::default()).unwrap();
    assert!(
        t.samples
            .windows(2)
            .all(|w| w[0].frame.normal().dot(w[1].frame.normal()) > 0.99)
    );
    p.sections[0].roll_rate = qx_geometry::transition::Channel::constant(
        qx_geometry::transition::Unit::RadiansPerNormalized,
        1.,
        0.8,
    );
    let t1 = p.generate(&catalog, GenerationOptions::default()).unwrap();
    if let SectionKind::Spline { curve } = &mut p.sections[0].kind {
        for point in curve.points_mut() {
            point.position_m = point.position_m * 2.;
        }
    }
    let t2 = p.generate(&catalog, GenerationOptions::default()).unwrap();
    assert!((t2.length_m() - 2. * t1.length_m()).abs() < 1e-7);
    assert!(
        (t1.samples.last().unwrap().frame.normal() - t2.samples.last().unwrap().frame.normal())
            .length()
            < 1e-5
    );
    assert!(
        t1.graphs
            .iter()
            .all(|g| (g.roll_rate_rad_per_m - 0.8 / t1.length_m()).abs() < 2e-5)
    );
}

#[test]
fn spline_loop_transports_through_vertical_and_strict_boundary_rules_are_enforced() {
    use qx_geometry::authoring::{BoundaryPolicy, Section};
    let catalog = Catalog::built_in().unwrap();
    let mut p = numerical_reference_project();
    p.sections = (1..=4)
        .map(|id| {
            let mut s = Section::spline(id, "Quarter vertical loop", reference_arc(10.));
            s.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
            s
        })
        .collect();
    let t = p.generate(&catalog, GenerationOptions::default()).unwrap();
    let last = t.samples.last().unwrap();
    assert!(last.position_m.length() < 1e-8);
    assert!((last.frame.tangent() - Vec3::X).length() < 1e-10);
    assert!((last.frame.normal() - Vec3::Y).length() < 1e-10);
    assert!(
        t.connections
            .iter()
            .all(|c| c.curvature_vector_gap_per_m < 1e-10)
    );
    assert!(
        t.samples
            .windows(2)
            .all(|w| w[0].frame.normal().dot(w[1].frame.normal()) > 0.999)
    );
    p.sections[1] = Section::straight(2, "Strict straight", 5.);
    p.sections[1].incoming_boundary = BoundaryPolicy::RequireContinuousRates;
    assert!(p.generate(&catalog, GenerationOptions::default()).is_err());
    p.sections[1].incoming_boundary = BoundaryPolicy::AllowRateJump;
    p.sections[2].incoming_boundary = BoundaryPolicy::AllowRateJump;
    let t = p.generate(&catalog, GenerationOptions::default()).unwrap();
    assert!(t.diagnostics.iter().any(|d| d.code == "section_rate_jump"));
    let mut wrong = Section::spline(20, "Unaligned", reference_arc(10.));
    if let SectionKind::Spline { curve } = &mut wrong.kind {
        for point in curve.points_mut() {
            point.position_m = point.position_m + Vec3::Y;
        }
    }
    assert!(wrong.validate().is_err());
    if let SectionKind::Spline { curve } = &mut wrong.kind {
        curve.align_start().unwrap();
    }
    wrong.validate().unwrap();
}
