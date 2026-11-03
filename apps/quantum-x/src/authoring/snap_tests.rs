use super::{
    graph_canvas, graph_view,
    snap::{self, Settings, Space},
    workbench::Workbench,
};
use qx_geometry::{
    authoring::{ChannelKey, Domain},
    transition::Unit,
};

#[test]
fn snap_units_are_independent_and_invalid_increments_are_rejected() {
    let mut s = Settings {
        enabled: true,
        ..Default::default()
    };
    s.graph.station_m = 2.5;
    s.graph.fine_station_m = 0.125;
    s.graph.time_s = 0.025;
    s.graph.force_g = 0.2;
    s.graph.acceleration_mps2 = 0.5;
    assert_eq!(s.graph.x_step(Domain::Distance, false), 2.5);
    assert_eq!(s.graph.x_step(Domain::Distance, true), 0.125);
    assert_eq!(s.graph.x_step(Domain::Time, false), 0.025);
    s.graph.points = false;
    assert!(
        s.graph_step(snap::GraphAxis::Value(Unit::G), false, false)
            .is_none()
    );
    assert_eq!(
        s.graph_step(snap::GraphAxis::Handle(Unit::G), false, false),
        Some(0.2)
    );
    s.graph.boundaries = false;
    assert!(
        s.graph_step(snap::GraphAxis::Boundary(Domain::Distance), false, false)
            .is_none()
    );
    for (unit, raw, expected) in [
        (Unit::G, 1.31, 1.4),
        (Unit::MetresPerSecondSquared, 1.31, 1.5),
        (Unit::RadiansPerMetre, 0.011, 0.01),
        (Unit::RadiansPerSecond, 0.11, 0.1),
    ] {
        assert!((snap::scalar(raw, Some(s.graph.y_step(unit)), &[], 0.) - expected).abs() < 1e-14);
    }
    assert_eq!(snap::scalar(1.234567890123, None, &[], 0.), 1.234567890123);
    assert_eq!(snap::scalar(1.234, Some(0.1), &[1.235], 0.005), 1.235);
    assert!((snap::scalar(1.234, Some(0.1), &[1.235], 0.0005) - 1.2).abs() < 1e-14);
    for invalid in [0., -1., f64::NAN, f64::INFINITY] {
        s.graph.station_m = invalid;
        assert!(s.validate().is_err());
    }
}

#[test]
fn spatial_coordinates_keep_world_local_angles_stations_and_bypass_separate() {
    let mut s = Settings {
        enabled: true,
        ..Default::default()
    };
    s.spatial.grid_m = 2.;
    s.spatial.distance_m = 0.25;
    s.spatial.station_m = 0.5;
    assert_eq!(s.spatial_value(1.31, Space::World, &[], false, false), 2.);
    assert_eq!(s.spatial_value(1.31, Space::Local, &[], false, false), 1.25);
    assert_eq!(
        s.spatial_value(1.31, Space::Station, &[], false, false),
        1.5
    );
    let rad = s.spatial_value(13_f64.to_radians(), Space::Angle, &[], false, false);
    assert!((rad.to_degrees() - 15.).abs() < 1e-12);
    assert_eq!(s.spatial_value(1.31, Space::World, &[], false, true), 1.31);
    s.enabled = false;
    assert_eq!(s.spatial_value(1.31, Space::World, &[], false, false), 1.31);
    s.enabled = true;
    s.spatial.nearby_points = true;
    assert_eq!(
        s.spatial_value(1.31, Space::World, &[1.32], false, false),
        1.32
    );
}

#[test]
fn graph_extent_growth_shrink_undo_and_selection_preserve_useful_views() {
    let mut s = super::m5_tests::session();
    s.select(qx_geometry::authoring::SectionId(2));
    s.select_channel(ChannelKey::Normal);
    graph_canvas::fit(&mut s);
    let old = s.draft.extent;
    let y = s.graph_view.view.y;
    s.draft = qx_document::editing::resize(&s.draft, old * 2.).unwrap();
    graph_view::sync(&mut s);
    assert_eq!(s.graph_view.view.x.max, old * 2.);
    assert_eq!(s.graph_view.view.y, y);
    let zoomed = s.graph_view.view.zoom((old / 2., 1.), 0.5, old * 2.);
    s.graph_view.set(zoomed);
    s.draft = qx_document::editing::resize(&s.draft, old * 3.).unwrap();
    graph_view::sync(&mut s);
    assert_eq!(s.graph_view.view, zoomed);
    graph_canvas::fit_region(&mut s);
    assert_eq!(s.graph_view.view.x.max, old * 3.);
    assert_eq!(s.graph_view.view.y, y.zoom(1., 0.5).unwrap());
    s.cancel_section();
    graph_view::sync(&mut s);
    assert_eq!(s.graph_view.view.x.max, old);
    let view = s.graph_view.view;
    s.select(qx_geometry::authoring::SectionId(1));
    s.select(qx_geometry::authoring::SectionId(2));
    assert_eq!(s.graph_view.view, view);
}

#[test]
fn snap_preferences_are_backward_compatible_and_do_not_enter_project_documents() {
    let mut prefs = Workbench::default();
    prefs.snap.enabled = true;
    prefs.snap.graph.time_s = 0.125;
    prefs.snap.spatial.angle_degrees = 7.5;
    let encoded = serde_json::to_vec(&prefs).unwrap();
    assert_eq!(Workbench::decode(&encoded).unwrap().snap, prefs.snap);
    let mut old = serde_json::to_value(&prefs).unwrap();
    old.as_object_mut().unwrap().remove("snap");
    assert_eq!(
        Workbench::decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .snap,
        Settings::default()
    );
    let mut s = super::m5_tests::session();
    let original = s.editor.project().clone();
    s.workbench = prefs;
    assert!(!s.has_drafts() && !s.editor.can_undo());
    let json = qx_document::persistence::to_json(s.editor.project(), &s.catalog).unwrap();
    assert!(!json.contains("\"snap\""));
    assert_eq!(
        qx_document::persistence::from_json(&json, &s.catalog).unwrap(),
        original
    );
}

#[test]
fn snap_spatial_drag_retains_raw_accumulator_and_only_changes_the_edited_axis() {
    let ctx = egui::Context::default();
    let mut snap = Settings {
        enabled: true,
        ..Default::default()
    };
    let mut point = qx_geometry::math::Vec3::new(1.234567, 2.345678, 3.456789);
    let original = point;
    let mut rect = egui::Rect::NOTHING;
    let mut id = egui::Id::NULL;
    let frame = |point: &mut qx_geometry::math::Vec3,
                 rect: &mut egui::Rect,
                 id: &mut egui::Id,
                 snap: Settings,
                 events: Vec<egui::Event>,
                 modifiers| {
        let mut events = events;
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400., 200.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    let r = super::snap_ui::number(
                        ui,
                        &mut point.x,
                        "X m ",
                        0.05,
                        snap,
                        Space::World,
                        &[],
                    );
                    *rect = r.rect;
                    *id = r.id;
                });
            },
        )
        .textures_delta
        .clear();
    };
    for _ in 0..3 {
        frame(
            &mut point,
            &mut rect,
            &mut id,
            snap,
            vec![],
            egui::Modifiers::NONE,
        );
    }
    assert_eq!(point, original, "enabling snap must not edit idle values");
    let pos = rect.center();
    let pointer =
        |pos, pressed| super::m5_tests::pointer(pos, egui::PointerButton::Primary, pressed);
    frame(
        &mut point,
        &mut rect,
        &mut id,
        snap,
        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        egui::Modifiers::NONE,
    );
    let moved = pos + egui::vec2(25., 0.);
    frame(
        &mut point,
        &mut rect,
        &mut id,
        snap,
        vec![egui::Event::PointerMoved(moved)],
        egui::Modifiers::NONE,
    );
    let raw = ctx
        .data(|d| d.get_temp::<f64>(id))
        .expect("precision accumulator");
    assert_eq!(point.x, raw.round());
    assert_eq!((point.y, point.z), (original.y, original.z));
    frame(
        &mut point,
        &mut rect,
        &mut id,
        snap,
        vec![],
        egui::Modifiers::ALT,
    );
    assert_eq!(point.x, raw);
    snap.enabled = false;
    frame(
        &mut point,
        &mut rect,
        &mut id,
        snap,
        vec![],
        egui::Modifiers::NONE,
    );
    assert_eq!(point.x, raw);
    frame(
        &mut point,
        &mut rect,
        &mut id,
        snap,
        vec![pointer(moved, false)],
        egui::Modifiers::NONE,
    );
    assert_eq!(point.x, raw);
}

#[test]
fn snap_support_controls_stage_world_local_station_and_foundation_edits_transactionally() {
    use super::session::{Session, blank_project, scene};
    use qx_geometry::{
        authoring::Section,
        math::{Quaternion, Vec3},
        supports::*,
    };
    let catalog = qx_style::Catalog::built_in().unwrap();
    let mut p = blank_project();
    p.initial.orientation = Quaternion::axis_angle(Vec3::X, 0.7).unwrap();
    p.sections = vec![Section::straight(1, "authored line", 100.)];
    let track = p.generate(&catalog, Default::default()).unwrap();
    let mut structure = Structure::empty(1);
    let world = Vec3::new(1.234567, 2.345678, 3.456789);
    let offset = Vec3::new(0.13, -0.37, 0.29);
    structure.nodes = vec![
        Node {
            id: 1,
            name: "World point".into(),
            position: NodePosition::World {
                position_m: world,
                foundation: false,
            },
        },
        Node {
            id: 2,
            name: "Track point".into(),
            position: NodePosition::Track {
                attachment: Attachment::at_station(&track, 10.3, offset).unwrap(),
            },
        },
        Node {
            id: 3,
            name: "Foundation point".into(),
            position: NodePosition::Foundation {
                attachment: Attachment::at_station(&track, 15.4, offset).unwrap(),
                elevation_m: 1.31,
            },
        },
    ];
    p.supports.push(structure);
    let original = p.clone();
    let sc = scene(&p, &catalog, Default::default()).unwrap();
    let mut s = Session::new(p, None, catalog).unwrap();
    s.expansion.structure = Some(1);
    s.expansion.node = Some(1);
    s.workbench.snap.enabled = true;
    s.workbench.snap.spatial.grid_m = 2.;
    s.workbench.snap.spatial.distance_m = 0.25;
    s.workbench.snap.spatial.station_m = 0.5;
    let ctx = egui::Context::default();
    let frame = |s: &mut Session, events| {
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 1400.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|ui| {
                    ui.set_max_width(500.);
                    super::support_ui::show(s, ui, &sc);
                });
            },
        );
        let text: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(t) => {
                    Some((t.galley.job.text.clone(), t.pos + t.galley.size() * 0.5))
                }
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        text
    };
    let drag = |s: &mut Session, label: &str, dx| {
        for _ in 0..3 {
            frame(s, vec![]);
        }
        let text = frame(s, vec![]);
        let pos = text
            .iter()
            .find(|(t, _)| t == label)
            .unwrap_or_else(|| panic!("missing {label}: {text:?}"))
            .1;
        let moved = pos + egui::vec2(dx, 0.);
        frame(
            s,
            vec![
                egui::Event::PointerMoved(pos),
                super::m5_tests::pointer(pos, egui::PointerButton::Primary, true),
            ],
        );
        frame(s, vec![egui::Event::PointerMoved(moved)]);
        frame(
            s,
            vec![super::m5_tests::pointer(
                moved,
                egui::PointerButton::Primary,
                false,
            )],
        );
    };
    drag(&mut s, "X m ", 25.);
    assert!(
        matches!(s.expansion.supports[0].nodes[0].position,NodePosition::World {position_m,..} if position_m==Vec3::new(2.,world.y,world.z))
    );
    s.expansion.node = Some(2);
    drag(&mut s, "Track station m ", 23.);
    drag(&mut s, "X m ", 25.);
    let NodePosition::Track { attachment } = &s.expansion.supports[0].nodes[1].position else {
        unreachable!()
    };
    assert!((attachment.station(&track).unwrap() - 12.5).abs() < 1e-12);
    assert_eq!(attachment.offset_m, Vec3::new(1.5, offset.y, offset.z));
    s.expansion.node = Some(3);
    drag(&mut s, "Foundation elevation Y m ", 25.);
    let NodePosition::Foundation {
        attachment,
        elevation_m,
    } = &s.expansion.supports[0].nodes[2].position
    else {
        unreachable!()
    };
    assert_eq!(*elevation_m, 4.);
    assert_eq!(attachment.offset_m, offset);
    assert_eq!(s.editor.project(), &original);
    assert!(s.apply_expansion(), "{:?}", s.expansion.message);
    let applied = s.editor.project().clone();
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    s.undo(true);
    assert_eq!(s.editor.project(), &applied);
    let json = qx_document::persistence::to_json(&applied, &s.catalog).unwrap();
    assert_eq!(
        qx_document::persistence::from_json(&json, &s.catalog).unwrap(),
        applied
    );
}
