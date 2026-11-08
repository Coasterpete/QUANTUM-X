use qx_document::{
    command::{Command, Editor},
    editing, persistence, region_edit as edit,
    sample::foundation_project,
};
use qx_geometry::{
    authoring::{ChannelKey, Domain, Section, SectionKind},
    profile::Continuity,
    transition::{Channel, Function, Transition, Unit},
};
use qx_style::Catalog;

fn channel(s: &Section, key: ChannelKey) -> Channel {
    s.clone().channel_mut(key).unwrap().clone()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
fn jets(a: &Transition, b: &Transition) {
    for end in [false, true] {
        let a = a.evaluate(if end { a.length } else { 0. }).unwrap();
        let b = b.evaluate(if end { b.length } else { 0. }).unwrap();
        near(a.value, b.value);
        near(a.derivative, b.derivative);
        near(a.second_derivative, b.second_derivative);
    }
}

#[test]
fn precision_ramp_hold_exit_has_exact_plateau_c2_joins_and_independent_lengths() {
    let original = Section::straight(1, "Synthetic precision fixture", 10.);
    let s = edit::ramp_plateau(&original, ChannelKey::Roll, 0).unwrap();
    let s = edit::plateau_level(&s, ChannelKey::Roll, 1, 0.025).unwrap();
    let s = edit::boundary(&s, ChannelKey::Roll, 0, 2.).unwrap();
    let s = edit::boundary(&s, ChannelKey::Roll, 1, 5.).unwrap();
    let c = channel(&s, ChannelKey::Roll);
    assert_eq!(
        c.transitions.iter().map(|t| t.length).collect::<Vec<_>>(),
        vec![2., 5., 3.]
    );
    assert!(c.continuity().unwrap().is_empty());
    for x in [0., 10.] {
        let e = c.evaluate(x).unwrap();
        near(e.value, 0.);
        near(e.derivative, 0.);
        near(e.second_derivative, 0.);
    }
    for i in 0..=100 {
        let e = c.evaluate(2. + 5. * i as f64 / 100.).unwrap();
        near(e.value, 0.025);
        near(e.derivative, 0.);
        near(e.second_derivative, 0.);
    }
    let raised = edit::point(&s, ChannelKey::Roll, 1, true, 7., 0.04).unwrap();
    let raised = channel(&raised, ChannelKey::Roll);
    assert!(raised.continuity().unwrap().is_empty());
    assert!(edit::is_plateau(&raised.transitions[1]));
    near(raised.transitions[0].start, 0.);
    near(raised.transitions[2].end, 0.);
    let peak = raised.transitions[0].end;
    assert!((1..20).all(|i| {
        let v = raised.transitions[0]
            .evaluate(2. * i as f64 / 20.)
            .unwrap()
            .value;
        v > 0. && v < peak
    }));
    assert!(edit::boundary(&s, ChannelKey::Roll, 0, 0.).is_err());
    assert!(edit::boundary(&s, ChannelKey::Roll, 0, 10.).is_err());
    assert!(edit::plateau_level(&s, ChannelKey::Roll, 1, f64::NAN).is_err());
}

#[test]
fn precision_shape_timing_edits_preserve_jets_knots_and_explicit_constraints() {
    let mut s = Section::straight(1, "Synthetic easing", 10.);
    s.roll_rate.transitions = vec![Transition {
        id: 7,
        length: 10.,
        start: 0.,
        end: 0.04,
        amplitude: 0.,
        function: Function::QuinticSmoothstep {},
    }];
    let t = s.roll_rate.transitions[0].to_nurbs().unwrap();
    let Function::Nurbs { mut profile } = t.function else {
        unreachable!()
    };
    profile.refine_for_edit().unwrap();
    let u = profile.knots[4..=3 + profile.degree].iter().sum::<f64>() / profile.degree as f64;
    let edited = edit::shape_point(
        &s,
        ChannelKey::Roll,
        0,
        3,
        (u + 0.025) * 10.,
        profile.values[3] + 0.003,
    )
    .unwrap();
    jets(
        &s.roll_rate.transitions[0],
        &edited.roll_rate.transitions[0],
    );
    let Function::Nurbs { profile: p } = &edited.roll_rate.transitions[0].function else {
        unreachable!()
    };
    near(
        p.knots[4..=3 + p.degree].iter().sum::<f64>() / p.degree as f64,
        u + 0.025,
    );
    near(p.values[3], profile.values[3] + 0.003);
    assert_ne!(
        s.roll_rate.evaluate(5.).unwrap().value,
        edited.roll_rate.evaluate(5.).unwrap().value
    );
    let resized = editing::resize(&edited, 20.).unwrap();
    for x in [0., 1., 5., 9., 10.] {
        let a = edited.roll_rate.evaluate(x).unwrap();
        let b = resized.roll_rate.evaluate(2. * x).unwrap();
        near(a.value, b.value);
        near(a.derivative, 2. * b.derivative);
    }
    assert_eq!(
        edit::shape_point(&s, ChannelKey::Roll, 0, 3, u * 10., profile.values[3]).unwrap(),
        s
    );
    let mut shape = p.clone();
    let count = shape.values.len();
    for _ in 0..10 {
        shape.refine_for_edit().unwrap();
    }
    assert_eq!(
        shape.values.len(),
        count,
        "viewing moved timing knots must not insert more controls"
    );
    assert!(edit::shape_point(&s, ChannelKey::Roll, 0, 3, -1., 0.).is_err());
    assert!(edit::shape_point(&s, ChannelKey::Roll, 0, 0, 2., 0.).is_err());
    assert!(edit::shape_point(&s, ChannelKey::Roll, 0, 3, u * 10., f64::INFINITY).is_err());
}

#[test]
fn precision_point_moves_both_axes_atomically_and_does_not_change_other_channels() {
    let mut s = Section::straight(1, "Synthetic point", 10.);
    s.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    let s = edit::ramp_plateau(&s, ChannelKey::Pitch, 0).unwrap();
    let moved = edit::point(&s, ChannelKey::Pitch, 0, true, 2.5, 0.02).unwrap();
    let c = channel(&moved, ChannelKey::Pitch);
    near(c.transitions[0].length, 2.5);
    near(c.transitions[1].start, 0.02);
    near(c.length(), 10.);
    assert!(c.continuity().unwrap().is_empty());
    assert_eq!(s.roll_rate, moved.roll_rate);
    assert_eq!(
        channel(&s, ChannelKey::Yaw),
        channel(&moved, ChannelKey::Yaw)
    );
    assert!(edit::point(&s, ChannelKey::Pitch, 0, true, 15., 0.02).is_err());
    assert!(edit::point(&s, ChannelKey::Pitch, 0, false, 1., 0.02).is_err());
}

#[test]
fn precision_all_region_channels_and_domains_keep_units_and_extent() {
    let mut cases = vec![Section::straight(1, "straight", 10.)];
    let mut arc = cases[0].clone();
    arc.kind = SectionKind::Arc {
        radius_m: 100.,
        plane_angle_rad: 0.,
    };
    cases.push(arc);
    let mut force = cases[0].clone();
    force.kind = SectionKind::Force {
        normal_g: Channel::constant(Unit::G, 10., 1.),
        lateral_g: Channel::constant(Unit::G, 10., 0.),
    };
    cases.push(force);
    let mut geometric = cases[0].clone();
    geometric.kind = SectionKind::Geometric {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    cases.push(geometric);
    let mut rider = cases.last().unwrap().clone();
    if let SectionKind::Geometric {
        pitch_rate,
        yaw_rate,
    } = rider.kind
    {
        rider.kind = SectionKind::RiderLocal {
            pitch_rate,
            yaw_rate,
        };
    }
    cases.push(rider);
    for source in cases {
        for domain in [Domain::Distance, Domain::Time] {
            let s = editing::convert_domain(&source, domain, 10.).unwrap();
            for key in editing::channel_keys(&s) {
                let unit = channel(&s, key).unit;
                let level = if unit == Unit::G { 1.1 } else { 0.01 };
                let edited = edit::ramp_plateau(&s, key, 0).unwrap();
                let edited = edit::plateau_level(&edited, key, 1, level).unwrap();
                assert_eq!(edited.domain, domain);
                assert_eq!(channel(&edited, key).unit, unit);
                near(edited.extent, s.extent);
                assert!(channel(&edited, key).continuity().unwrap().is_empty());
            }
        }
    }
}

#[test]
fn precision_document_history_and_reopen_generate_identical_authoritative_track() {
    let catalog = Catalog::built_in().unwrap();
    let mut p = foundation_project();
    p.sections = vec![Section::straight(1, "Synthetic roll fixture", 10.)];
    let before = p.generate(&catalog, Default::default()).unwrap();
    let s = edit::ramp_plateau(&p.sections[0], ChannelKey::Roll, 0).unwrap();
    let s = edit::plateau_level(&s, ChannelKey::Roll, 1, 0.02).unwrap();
    let mut editor = Editor::new(p.clone(), catalog.clone(), 32).unwrap();
    editor
        .execute(vec![Command::ReplaceSection { section: s }])
        .unwrap();
    let after = editor
        .project()
        .generate(&catalog, Default::default())
        .unwrap();
    assert_ne!(before, after);
    let json = persistence::to_json(editor.project(), &catalog).unwrap();
    let reopened = persistence::from_json(&json, &catalog).unwrap();
    assert_eq!(reopened, *editor.project());
    assert_eq!(
        reopened.generate(&catalog, Default::default()).unwrap(),
        after
    );
    assert!(editor.undo());
    assert_eq!(*editor.project(), p);
    assert!(editor.redo());
    assert_eq!(*editor.project(), reopened);
    assert!(
        channel(&reopened.sections[0], ChannelKey::Roll)
            .transitions
            .iter()
            .skip(1)
            .all(|t| t.incoming_continuity() == Continuity::C2)
    );
}
