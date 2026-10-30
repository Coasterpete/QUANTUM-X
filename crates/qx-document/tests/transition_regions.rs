use qx_document::{
    command::{Command, Editor},
    editing, persistence, region_edit,
    sample::foundation_project,
};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionKind},
    profile::Continuity,
    solver::GenerationOptions,
    transition::{Channel, Function, Transition, Unit},
};
use qx_style::Catalog;

fn section() -> Section {
    let mut s = Section::straight(1, "Synthetic pitch split regression", 10.);
    s.kind = SectionKind::RiderLocal {
        pitch_rate: Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![Transition {
                id: 41,
                length: 10.,
                start: 0.,
                end: 0.03,
                amplitude: 0.006,
                function: Function::Hermite {
                    start_slope: 0.001,
                    end_slope: -0.002,
                },
            }],
        },
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    s
}
fn channel(s: &Section) -> &Channel {
    s.channels()
        .into_iter()
        .find(|(name, _)| *name == "pitch")
        .unwrap()
        .1
}

#[test]
fn station_split_shape_edits_and_removal_keep_surviving_boundary_contracts() {
    let mut s = section();
    s.channel_mut(ChannelKey::Pitch).unwrap().transitions =
        vec![Transition::nurbs(41, 10., 0., 0.03)];
    let original = s.clone();
    for station in [2., 4., 7.] {
        let (next, right) = region_edit::split_at(&s, ChannelKey::Pitch, station).unwrap();
        assert_eq!(right, channel(&s).transitions.len());
        s = next;
        compare(&original, &s);
    }
    for invalid in [0., 2., 4., 7., 10., -1., 11., f64::NAN] {
        assert!(region_edit::split_at(&s, ChannelKey::Pitch, invalid).is_err());
    }
    let untouched = channel(&s).transitions[2..].to_vec();
    for step in 0..20 {
        let value = 0.005 + step as f64 * 0.0001;
        let edited = region_edit::shape_control(&s, ChannelKey::Pitch, 1, 3, value).unwrap();
        assert!(channel(&edited).continuity().unwrap().is_empty());
        for x in [2., 4.] {
            let a = channel(&s).evaluate(x).unwrap();
            let b = channel(&edited).evaluate(x).unwrap();
            assert!((a.value - b.value).abs() < 1e-14);
            assert!((a.derivative - b.derivative).abs() < 1e-12);
            assert!((a.second_derivative - b.second_derivative).abs() < 1e-10);
        }
        assert_eq!(&channel(&edited).transitions[2..], &untouched);
        assert_eq!(edited.roll_rate, original.roll_rate);
        s = edited;
    }
    for index in 0..4 {
        let removed =
            editing::remove_transition(&s, ChannelKey::Pitch, channel(&s).transitions[index].id)
                .unwrap();
        assert_eq!(channel(&removed).transitions.len(), 3);
        assert_eq!(channel(&removed).length(), 10.);
        assert!(channel(&removed).continuity().unwrap().is_empty());
        for x in [0., 10.] {
            let a = channel(&s).evaluate(x).unwrap();
            let b = channel(&removed).evaluate(x).unwrap();
            assert!((a.value - b.value).abs() < 1e-14);
            assert!((a.derivative - b.derivative).abs() < 1e-12);
            assert!((a.second_derivative - b.second_derivative).abs() < 1e-10);
        }
        assert_eq!(removed.roll_rate, s.roll_rate);
    }
    assert!(region_edit::shape_control(&s, ChannelKey::Pitch, 1, 0, 1.).is_err());
    assert!(region_edit::shape_control(&s, ChannelKey::Pitch, 1, usize::MAX, 1.).is_err());
    assert!(region_edit::shape_control(&s, ChannelKey::Pitch, 1, 3, f64::NAN).is_err());
    // Recombination preserves edited profiles as well as unedited splits.
    let merged = region_edit::merge(&s, ChannelKey::Pitch, 1).unwrap();
    compare(&s, &merged);
}
fn compare(a: &Section, b: &Section) {
    for i in 0..=1000 {
        let x = a.extent * i as f64 / 1000.;
        let a = channel(a).evaluate(x).unwrap();
        let b = channel(b).evaluate(x).unwrap();
        assert!((a.value - b.value).abs() < 2e-14);
        assert!((a.derivative - b.derivative).abs() < 2e-12);
        assert!((a.second_derivative - b.second_derivative).abs() < 2e-9);
    }
}
#[test]
fn repeated_splits_merge_and_timing_edits_preserve_station_constraints_and_other_channels() {
    let s = section();
    let a = editing::split_transition(&s, ChannelKey::Pitch, 41, 0.3577).unwrap();
    compare(&s, &a);
    assert_eq!(a.extent, s.extent);
    assert_eq!(a.roll_rate, s.roll_rate);
    assert_eq!(channel(&a).transitions[0].id, 41);
    let b = editing::split_transition(&a, ChannelKey::Pitch, 42, 0.999).unwrap();
    compare(&s, &b);
    assert!(channel(&b).continuity().unwrap().is_empty());
    let merged = region_edit::merge(&b, ChannelKey::Pitch, 1).unwrap();
    compare(&s, &merged);
    let merged = region_edit::merge(&merged, ChannelKey::Pitch, 0).unwrap();
    compare(&s, &merged);
    let moved = region_edit::boundary(&a, ChannelKey::Pitch, 0, 4.25).unwrap();
    assert_eq!(moved.roll_rate, s.roll_rate);
    assert_eq!(channel(&moved).length(), 10.);
    assert!(channel(&moved).continuity().unwrap().is_empty());
    for (x, y) in [(0., 0.), (10., 10.)] {
        let e = channel(&a).evaluate(x).unwrap();
        let f = channel(&moved).evaluate(y).unwrap();
        assert!(
            (e.derivative - f.derivative).abs() < 1e-12
                && (e.second_derivative - f.second_derivative).abs() < 1e-12
        );
    }
}
#[test]
fn shared_endpoint_edits_keep_c2_and_preserve_uninvolved_authored_regions() {
    let s = editing::split_transition(&section(), ChannelKey::Pitch, 41, 0.3577).unwrap();
    let s = editing::split_transition(&s, ChannelKey::Pitch, 42, 0.5).unwrap();
    let untouched = channel(&s).transitions[2].clone();
    let next = region_edit::endpoint(&s, ChannelKey::Pitch, 0, true, 0.014).unwrap();
    assert!(channel(&next).continuity().unwrap().is_empty());
    assert_eq!(channel(&next).transitions[2], untouched);
    assert_eq!(next.roll_rate, s.roll_rate);
    assert!((channel(&next).evaluate(3.577).unwrap().value - 0.014).abs() < 1e-14);
    let mut corrupt = channel(&next).transitions[1].clone();
    if let Function::Nurbs { profile } = &mut corrupt.function {
        profile.values[0] += 0.005;
        corrupt.start = profile.values[0];
    }
    let error = region_edit::replace(&next, ChannelKey::Pitch, 1, corrupt).unwrap_err();
    assert!(error.to_string().contains("violate C2"));
    // Only the advanced explicit choice permits a step; no hidden neighbor fitting.
    let free = region_edit::continuity(&next, ChannelKey::Pitch, 1, Continuity::Free).unwrap();
    let free = region_edit::endpoint(&free, ChannelKey::Pitch, 1, false, 0.02).unwrap();
    assert!(!channel(&free).continuity().unwrap().is_empty());
    assert_eq!(channel(&free).transitions[0], channel(&next).transitions[0]);
    assert!(region_edit::continuity(&free, ChannelKey::Pitch, 1, Continuity::C2).is_err());
    let repaired = region_edit::join_incoming(&free, ChannelKey::Pitch, 1).unwrap();
    assert!(channel(&repaired).continuity().unwrap().is_empty());
    assert_eq!(
        channel(&repaired).transitions[2],
        channel(&free).transitions[2]
    );
    assert_eq!(
        channel(&repaired).transitions[1].start,
        channel(&free).transitions[1].start
    );
    assert!(region_edit::reorder(&s, ChannelKey::Pitch, 0, 1).is_err());
    let merged = region_edit::merge(&s, ChannelKey::Pitch, 0).unwrap();
    let mut changed = channel(&merged).transitions[0].clone();
    if let Function::Nurbs { profile } = &mut changed.function {
        profile.values[3] += 0.02;
    }
    assert!(
        region_edit::replace(&merged, ChannelKey::Pitch, 0, changed).is_err(),
        "repeated internal knots must retain their C2 contract"
    );
}
#[test]
fn split_geometry_curvature_banking_and_undo_save_reopen_remain_equivalent() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = foundation_project();
    p.sections = vec![section()];
    let options = GenerationOptions {
        max_step_m: 0.025,
        ..Default::default()
    };
    let before = p.generate(&catalog, options).unwrap();
    let mut editor = Editor::new(p.clone(), catalog.clone(), 16).unwrap();
    let split = editing::split_transition(&p.sections[0], ChannelKey::Pitch, 41, 0.3577).unwrap();
    editor
        .execute([Command::ReplaceSection { section: split }])
        .unwrap();
    let after = editor.project().generate(&catalog, options).unwrap();
    assert_eq!(before.samples.len(), after.samples.len());
    let mut max_position: f64 = 0.;
    let mut max_curvature: f64 = 0.;
    for (a, b) in before.samples.iter().zip(&after.samples) {
        max_position = max_position.max((a.position_m - b.position_m).length());
        assert!((a.frame.tangent() - b.frame.tangent()).length() < 2e-13);
        assert!((a.frame.normal() - b.frame.normal()).length() < 2e-13);
    }
    for (a, b) in before.graphs.iter().zip(&after.graphs) {
        max_curvature = max_curvature.max((a.curvature_per_m - b.curvature_per_m).abs());
        assert!(
            (a.normal_g - b.normal_g).abs() < 1e-10 && (a.lateral_g - b.lateral_g).abs() < 1e-10
        );
    }
    assert!(max_position < 2e-12 && max_curvature < 2e-11);
    println!(
        "3.577 m split: max position error {max_position:.16e} m; max curvature error {max_curvature:.16e} /m; {} matched stations",
        before.samples.len()
    );
    let edited = editor.project().clone();
    assert!(editor.undo());
    assert_eq!(editor.project(), &p);
    assert!(editor.redo());
    assert_eq!(editor.project(), &edited);
    let reopened =
        persistence::from_json(&persistence::to_json(&edited, &catalog).unwrap(), &catalog)
            .unwrap();
    assert_eq!(edited, reopened);
    assert_eq!(after, reopened.generate(&catalog, options).unwrap());
    let timed = editing::convert_domain(
        &edited.sections[0],
        qx_geometry::authoring::Domain::Time,
        20.,
    )
    .unwrap();
    let restored =
        editing::convert_domain(&timed, qx_geometry::authoring::Domain::Distance, 20.).unwrap();
    compare(&edited.sections[0], &restored);
}
#[test]
fn schema_five_hermite_migrates_exactly_without_erasing_jumps_or_ids() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = foundation_project();
    p.sections = vec![section()];
    let split = editing::split_transition(&p.sections[0], ChannelKey::Pitch, 41, 0.3577).unwrap();
    // Construct actual legacy Hermite records with nonzero amplitude and an explicit step.
    let mut legacy = split;
    for (i, t) in legacy
        .channel_mut(ChannelKey::Pitch)
        .unwrap()
        .transitions
        .iter_mut()
        .enumerate()
    {
        t.function = Function::Hermite {
            start_slope: 0.001,
            end_slope: 0.002,
        };
        t.amplitude = 0.004;
        if i == 1 {
            t.start += 0.003;
        }
    }
    p.sections = vec![legacy.clone()];
    let before = p.generate(&catalog, GenerationOptions::default()).unwrap();
    let mut json = serde_json::to_value(&p).unwrap();
    json["schema_version"] = 5.into();
    let migrated = persistence::from_json(&json.to_string(), &catalog).unwrap();
    assert_eq!(migrated.schema_version, 6);
    compare(&legacy, &migrated.sections[0]);
    for t in &channel(&migrated.sections[0]).transitions {
        assert!(matches!(t.function, Function::Nurbs { .. }));
        assert_eq!(t.incoming_continuity(), Continuity::Free);
    }
    assert_eq!(
        channel(&migrated.sections[0])
            .transitions
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        vec![41, 42]
    );
    assert_eq!(
        channel(&legacy).continuity().unwrap().len(),
        channel(&migrated.sections[0]).continuity().unwrap().len()
    );
    let after = migrated
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    for (a, b) in before.samples.iter().zip(&after.samples) {
        assert!((a.position_m - b.position_m).length() < 2e-12);
    }
    let mut invalid = serde_json::to_value(&migrated).unwrap();
    invalid["schema_version"] = 5.into();
    assert!(persistence::from_json(&invalid.to_string(), &catalog).is_err());
}
#[test]
fn normalized_roll_splits_preserve_every_positional_curve_representation_and_banking() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = qx_document::sample::spline_comparison_project();
    for s in &mut p.sections {
        s.roll_rate.transitions = vec![Transition {
            id: 1,
            length: 1.,
            start: 0.,
            end: 0.04,
            amplitude: 0.01,
            function: Function::Hermite {
                start_slope: 0.02,
                end_slope: -0.01,
            },
        }];
    }
    let before = p.generate(&catalog, Default::default()).unwrap();
    let curves: Vec<_> = p.sections.iter().map(|s| s.kind.clone()).collect();
    for s in &mut p.sections {
        *s = editing::split_transition(s, ChannelKey::Roll, 1, 0.3577).unwrap();
    }
    let after = p.generate(&catalog, Default::default()).unwrap();
    assert_eq!(
        curves,
        p.sections
            .iter()
            .map(|s| s.kind.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(before.samples.len(), after.samples.len());
    for (a, b) in before.samples.iter().zip(&after.samples) {
        assert_eq!(a.distance_m, b.distance_m);
        assert_eq!(a.position_m, b.position_m);
        assert!((a.frame.tangent() - b.frame.tangent()).length() < 2e-13);
        assert!((a.frame.normal() - b.frame.normal()).length() < 2e-13);
    }
}
