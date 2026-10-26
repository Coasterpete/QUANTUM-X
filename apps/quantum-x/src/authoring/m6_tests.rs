use super::{
    graph_canvas, panels, scenario,
    session::{Session, blank_project, scene},
    workflow::PendingAction,
};
use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
use qx_document::{command::Command, editing, persistence};
use qx_geometry::{
    authoring::{BoundaryPolicy, ChannelKey, Domain, SectionId, SectionKind},
    math::{Quaternion, Vec3},
    transition::Function,
};
use qx_rendering::{DisplayOptions, EditorSurface};
use std::time::{Duration, Instant};

fn blank() -> Session {
    Session::new(
        blank_project(),
        None,
        qx_style::Catalog::built_in().unwrap(),
    )
    .unwrap()
}
fn wait(s: &mut Session) -> Option<qx_rendering::InspectionScene> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(scene) = s.poll() {
            return Some(scene);
        }
        if s.status.starts_with("Generation failed") {
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker timed out: {}", s.status)
}

#[test]
fn insertion_continues_exact_pose_curvature_twist_and_speed_across_domains() {
    for domain in [Domain::Time, Domain::Distance] {
        for kind in 0..3 {
            let mut s = blank();
            s.draft = panels::new_section(1, 1);
            if let SectionKind::RiderLocal {
                pitch_rate,
                yaw_rate,
            } = &mut s.draft.kind
            {
                pitch_rate.transitions[0].start = 0.012;
                pitch_rate.transitions[0].end = 0.012;
                yaw_rate.transitions[0].start = 0.008;
                yaw_rate.transitions[0].end = 0.008;
            }
            s.draft.roll_rate.transitions[0].start = 0.015;
            s.draft.roll_rate.transitions[0].end = 0.015;
            s.draft.reference_speed_mps = Some(25.);
            s.draft = editing::convert_domain(&s.draft, domain, 25.).unwrap();
            assert!(s.apply_draft());
            let before = s.editor.project().generate(&s.catalog, s.options).unwrap();
            let original = s.editor.project().sections[0].clone();
            panels::insert_section(&mut s, kind, false);
            assert_eq!(s.selected, SectionId(2));
            assert_eq!(
                s.channel,
                if kind == 0 {
                    ChannelKey::Normal
                } else {
                    ChannelKey::Pitch
                }
            );
            assert_eq!(s.draft.domain, domain);
            assert_eq!(s.draft.reference_speed_mps, Some(25.));
            s.draft.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
            assert!(s.apply_draft(), "{}", s.status);
            let after = s.editor.project().generate(&s.catalog, s.options).unwrap();
            let join = &after.connections[0];
            assert!(join.curvature_vector_gap_per_m < 1e-12, "{join:?}");
            assert!(join.twist_rate_gap_rad_per_m.abs() < 1e-12);
            assert_eq!(join.reference_speed_gap_mps, 0.);
            let start = &after.samples[after.sections[1].start_sample];
            assert_eq!(start, before.samples.last().unwrap());
            assert!((after.length_m() - 20.).abs() < 1e-10);
            assert_eq!(s.editor.project().sections[0], original);
        }
    }
}

#[test]
fn tilted_banked_initial_pose_sets_gravity_resolved_force_defaults() {
    let mut s = blank();
    s.initial_draft.orientation = Quaternion::axis_angle(Vec3::Z, 0.3)
        .unwrap()
        .compose(Quaternion::axis_angle(Vec3::X, 0.6).unwrap());
    assert!(s.execute(vec![Command::SetInitial(s.initial_draft.clone())]));
    panels::insert_section(&mut s, 0, true);
    let frame = qx_geometry::math::Frame::new(s.editor.project().initial.orientation).unwrap();
    let n = s
        .draft
        .channel_mut(ChannelKey::Normal)
        .unwrap()
        .evaluate(0.)
        .unwrap()
        .value;
    let l = s
        .draft
        .channel_mut(ChannelKey::Lateral)
        .unwrap()
        .evaluate(0.)
        .unwrap()
        .value;
    assert!((n - frame.normal().y).abs() < 1e-14);
    assert!((l - frame.lateral().y).abs() < 1e-14);
    assert!((n - 1.).abs() > 0.1 && l.abs() > 0.1);
}

#[test]
fn every_insertable_family_preserves_previous_inputs_and_roundtrips() {
    for kind in 0..9 {
        for before in [true, false] {
            let mut s = blank();
            let original = s.editor.project().sections[0].clone();
            panels::insert_section(&mut s, kind, before);
            assert_eq!(s.editor.project().sections.len(), 2, "{}", s.status);
            assert_eq!(s.editor.project().sections[usize::from(before)], original);
            let generated = s.editor.project().generate(&s.catalog, s.options).unwrap();
            assert_eq!(
                generated.sections[0].end_sample,
                generated.sections[1].start_sample
            );
            let saved = persistence::to_json(s.editor.project(), &s.catalog).unwrap();
            assert_eq!(
                &persistence::from_json(&saved, &s.catalog).unwrap(),
                s.editor.project()
            );
            s.undo(false);
            assert_eq!(s.editor.project().sections, vec![original]);
        }
    }
}

#[test]
fn pending_insert_resolves_explicitly_with_separate_undo_transactions() {
    for apply in [true, false] {
        let mut s = blank();
        let original = s.editor.project().clone();
        s.draft.name = "Owner draft".into();
        panels::insert_section(&mut s, 0, false);
        assert_eq!(s.selected, SectionId(1));
        assert_eq!(s.editor.project(), &original);
        assert_eq!(
            s.pending_action,
            Some(PendingAction::Insert {
                kind: 0,
                before: false
            })
        );
        s.resolve_pending(apply);
        assert_eq!(s.selected, SectionId(2));
        assert!(s.pending_action.is_none());
        s.undo(false);
        assert_eq!(s.editor.project().sections.len(), 1);
        if apply {
            assert_eq!(s.editor.project().sections[0].name, "Owner draft");
            s.undo(false);
        }
        assert_eq!(s.editor.project(), &original);
        assert!(!s.editor.can_undo());
    }
}

#[test]
fn invalid_draft_cannot_resolve_navigation_or_enter_history() {
    let mut s = blank();
    panels::insert_section(&mut s, 0, false);
    let original = s.editor.project().clone();
    s.draft.roll_rate.transitions[0].length = -1.;
    let draft = s.draft.clone();
    s.select(SectionId(1));
    s.resolve_pending(true);
    assert_eq!(s.selected, SectionId(2));
    assert_eq!(s.draft, draft);
    assert_eq!(s.editor.project(), &original);
    assert!(s.pending_action.is_some());
    assert!(s.draft_error.as_ref().unwrap().contains("length"));
    s.resolve_pending(false);
    assert_eq!(s.selected, SectionId(1));
    assert_eq!(s.editor.project(), &original);
}

#[test]
fn strict_downstream_failure_is_atomic_and_reports_measured_constraint() {
    let mut s = blank();
    panels::insert_section(&mut s, 0, false);
    s.draft.incoming_boundary = BoundaryPolicy::RequireContinuousRates;
    assert!(s.apply_draft());
    s.select(SectionId(1));
    let before = s.editor.project().clone();
    panels::insert_section(&mut s, 4, false);
    assert_eq!(s.editor.project(), &before);
    assert!(s.status.contains("curvature gap"), "{}", s.status);
    assert!(s.status.contains("section 2"));
    s.undo(false); // Strict-policy change was the preceding successful operation.
    assert_eq!(
        s.editor.project().sections[1].incoming_boundary,
        BoundaryPolicy::AllowRateJump
    );
}

#[test]
fn invalid_preview_and_cancel_preserve_revision_and_last_valid_geometry() {
    let mut s = blank();
    s.draft = editing::resize(&s.draft, 15.).unwrap();
    s.preview();
    assert!((wait(&mut s).unwrap().track.length_m() - 15.).abs() < 1e-10);
    let visible = s.visible_revision;
    s.draft.kind = SectionKind::Arc {
        radius_m: 0.0001,
        plane_angle_rad: 0.,
    };
    s.preview();
    assert!(wait(&mut s).is_none());
    assert_eq!(s.visible_revision, visible);
    assert!(s.draft_error.as_ref().unwrap().contains("rotation"));
    assert_eq!(s.editor.project().sections[0].extent, 10.);
    s.cancel_section();
    let restored = wait(&mut s).unwrap();
    assert!((restored.track.length_m() - 10.).abs() < 1e-10);
    assert!(!s.visible_preview);
}

#[test]
fn hill_scenario_is_continuous_reproducible_and_survives_document_roundtrip() {
    let s = scenario::build().unwrap();
    let track = s.editor.project().generate(&s.catalog, s.options).unwrap();
    assert!((track.length_m() - 90.).abs() < 1e-9);
    assert!(track.samples.iter().any(|p| p.position_m.y > 2.));
    assert!(track.samples.iter().any(|p| p.frame.tangent().y < -0.1));
    assert!(
        track
            .graphs
            .iter()
            .any(|p| p.bank_rad.unwrap_or(0.).abs() > 0.1)
    );
    assert!(
        track
            .connections
            .iter()
            .all(|c| c.curvature_vector_gap_per_m < 1e-12 && c.orientation_angle_rad == 0.)
    );
    let json = persistence::to_json(s.editor.project(), &s.catalog).unwrap();
    let reopened = persistence::from_json(&json, &s.catalog).unwrap();
    assert_eq!(track, reopened.generate(&s.catalog, s.options).unwrap());
    let normal = reopened.sections[1]
        .channels()
        .into_iter()
        .find(|(name, _)| *name == "normal")
        .unwrap()
        .1;
    assert_eq!(
        normal
            .transitions
            .iter()
            .map(|t| t.length)
            .collect::<Vec<_>>(),
        vec![22., 18., 20., 20.]
    );
    for t in &normal.transitions {
        assert!(matches!(t.function, Function::QuinticSmoothstep {}));
        assert!(t.evaluate(0.).unwrap().derivative.abs() < 1e-14);
    }
    assert!((normal.transitions[0].evaluate(22.).unwrap().value - 2.).abs() < 1e-14);
}

fn frame(
    s: &mut Session,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<Event>,
) -> (
    qx_rendering::SurfaceOutput,
    Vec<(String, Rect)>,
    DisplayOptions,
) {
    let scene = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut surface = None;
    let mut display = DisplayOptions::default();
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            surface = Some(s.ui(ctx, &scene, &mut display));
        },
    );
    let texts = output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                Some((
                    t.galley.job.text.clone(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                ))
            } else {
                None
            }
        })
        .collect();
    output.textures_delta.clear();
    (surface.unwrap(), texts, display)
}
fn click(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, label: &str) {
    let (_, texts, _) = frame(s, ctx, size, vec![]);
    let pos = texts
        .iter()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| panic!("missing control {label}; visible: {texts:?}"))
        .1
        .center();
    frame(
        s,
        ctx,
        size,
        vec![
            Event::PointerMoved(pos),
            super::m5_tests::pointer(pos, PointerButton::Primary, true),
        ],
    );
    frame(
        s,
        ctx,
        size,
        vec![super::m5_tests::pointer(pos, PointerButton::Primary, false)],
    );
}

#[test]
fn actual_workbench_insertion_and_pending_resolution_buttons_work_at_both_sizes() {
    for size in [egui::vec2(1440., 960.), egui::vec2(960., 640.)] {
        let mut s = blank();
        let ctx = egui::Context::default();
        frame(&mut s, &ctx, size, vec![]);
        frame(&mut s, &ctx, size, vec![]);
        s.draft.name = "Pending owner value".into();
        click(&mut s, &ctx, size, "Insert section…");
        click(&mut s, &ctx, size, "Insert after");
        assert!(s.pending_action.is_some());
        click(&mut s, &ctx, size, "Apply & continue");
        assert_eq!(s.selected, SectionId(2));
        assert_eq!(s.editor.project().sections[0].name, "Pending owner value");
        assert_eq!(s.channel, ChannelKey::Normal);
        let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
        assert!(texts.iter().any(|(t, _)| t == "Normal force"));
        click(&mut s, &ctx, size, "Edit values");
        let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
        assert!(texts.iter().any(|(t, _)| t == "Start"));
    }
}

#[test]
fn framing_requests_only_on_start_or_successful_open_and_hover_never_selects() {
    let mut s = scenario::build().unwrap();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    let first = frame(&mut s, &ctx, size, vec![]).0;
    assert!(first.reset_navigation);
    assert!(!frame(&mut s, &ctx, size, vec![]).0.reset_navigation);
    panels::insert_section(&mut s, 0, false);
    assert!(!frame(&mut s, &ctx, size, vec![]).0.reset_navigation);
    let selected = s.selected;
    s.inspect_station(Some((SectionId(1), 5.)));
    let scene = scene(s.editor.project(), &s.catalog, s.options).unwrap();
    assert!(scene.track.sections.len() > 2);
    assert_eq!(s.selected, selected);
    s.open(None, true);
    wait(&mut s).unwrap();
    assert!(frame(&mut s, &ctx, size, vec![]).0.reset_navigation);
    assert!(!frame(&mut s, &ctx, size, vec![]).0.reset_navigation);
}

#[test]
fn graph_hover_produces_a_corresponding_station_without_document_mutation() {
    let mut s = scenario::build().unwrap();
    wait(&mut s).unwrap();
    let original = s.editor.project().clone();
    s.select_channel(ChannelKey::Normal);
    graph_canvas::fit(&mut s);
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    let rect = ctx
        .read_response(egui::Id::unique((
            "graph canvas",
            2_u64,
            ChannelKey::Normal as u8,
        )))
        .unwrap()
        .rect;
    let plot = graph_canvas::plot_rect(rect);
    let (_, _, display) = frame(&mut s, &ctx, size, vec![Event::PointerMoved(plot.center())]);
    assert!((display.inspection_distance_m.unwrap() - 50.).abs() < 1e-8);
    assert_eq!(s.selected, SectionId(2));
    assert_eq!(s.editor.project(), &original);
    assert!(!s.draft_dirty());
    // View-only zoom still cannot write geometry or turn diagnostics into inputs.
    frame(
        &mut s,
        &ctx,
        size,
        vec![Event::MouseWheel {
            source: egui::MouseWheelSource::Unknown,
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0., 40.),
            modifiers: Modifiers::CTRL,
            phase: egui::TouchPhase::Move,
        }],
    );
    assert_eq!(s.editor.project(), &original);
}

#[test]
fn new_ids_do_not_reuse_undone_or_removed_sections_in_session_view_caches() {
    let mut s = blank();
    panels::insert_section(&mut s, 0, false);
    assert_eq!(s.selected, SectionId(2));
    s.undo(false);
    panels::insert_section(&mut s, 0, false);
    assert_eq!(s.selected, SectionId(3));
    s.remove_selected();
    panels::insert_section(&mut s, 0, false);
    assert_eq!(s.selected, SectionId(4));
}

#[test]
fn vertical_world_default_is_rejected_with_local_alternative_without_history_damage() {
    let mut s = blank();
    s.initial_draft.orientation =
        Quaternion::axis_angle(Vec3::Z, std::f64::consts::FRAC_PI_2).unwrap();
    assert!(s.execute(vec![Command::SetInitial(s.initial_draft.clone())]));
    let original = s.editor.project().clone();
    panels::insert_section(&mut s, 2, false);
    assert_eq!(s.editor.project(), &original);
    assert!(s.status.contains("Rider-local geometric"));
    panels::insert_section(&mut s, 1, false);
    assert_eq!(s.editor.project().sections.len(), 2);
    assert_eq!(s.draft.domain, Domain::Distance);
}

#[test]
fn cancel_and_unrelated_metadata_keep_other_geometric_drafts_previewed() {
    let mut s = blank();
    s.initial_draft.position_m = Vec3::Y * 3.;
    s.draft.name = "staged section".into();
    s.metadata_draft.name = "Project title".into();
    assert!(s.execute(vec![Command::SetMetadata(s.metadata_draft.clone())]));
    let preview = wait(&mut s).unwrap();
    assert_eq!(preview.track.samples[0].position_m.y, 3.);
    assert_eq!(s.draft.name, "staged section");
    s.cancel_section();
    let preview = wait(&mut s).unwrap();
    assert_eq!(preview.track.samples[0].position_m.y, 3.);
    assert!(s.visible_preview);
    assert_eq!(s.initial_draft.position_m.y, 3.);
    assert_eq!(s.editor.project().initial.position_m.y, 0.);
    assert!(!s.draft_dirty());
    s.select_section(SectionId(1));
    assert_eq!(s.initial_draft.position_m.y, 3.);
}

fn click_last(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, label: &str) {
    for _ in 0..3 {
        frame(s, ctx, size, vec![]);
    }
    let (_, texts, _) = frame(s, ctx, size, vec![]);
    let pos = texts
        .iter()
        .rev()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| {
            panic!(
                "missing control {label}; last controls: {:?}",
                texts.iter().rev().take(30).collect::<Vec<_>>()
            )
        })
        .1
        .center();
    click_at(s, ctx, size, pos);
}
fn click_at(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, pos: Pos2) {
    frame(
        s,
        ctx,
        size,
        vec![
            Event::PointerMoved(pos),
            super::m5_tests::pointer(pos, PointerButton::Primary, true),
        ],
    );
    frame(
        s,
        ctx,
        size,
        vec![super::m5_tests::pointer(pos, PointerButton::Primary, false)],
    );
}
fn key(
    s: &mut Session,
    ctx: &egui::Context,
    size: egui::Vec2,
    key: egui::Key,
    modifiers: Modifiers,
) {
    frame(
        s,
        ctx,
        size,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
    );
    frame(
        s,
        ctx,
        size,
        vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }],
    );
}
fn type_after_label(
    s: &mut Session,
    ctx: &egui::Context,
    size: egui::Vec2,
    label: &str,
    text: &str,
) {
    for _ in 0..3 {
        frame(s, ctx, size, vec![]);
    }
    let (_, texts, _) = frame(s, ctx, size, vec![]);
    let label = texts
        .iter()
        .rev()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| panic!("missing numeric label {label}"))
        .1;
    click_at(
        s,
        ctx,
        size,
        egui::pos2(label.right() + 24., label.center().y),
    );
    frame(s, ctx, size, vec![Event::Text(text.into())]);
    key(s, ctx, size, egui::Key::Enter, Modifiers::NONE);
}
fn select_transition(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, index: usize) {
    let count = s.draft.channel_mut(s.channel).unwrap().transitions.len();
    click_last(
        s,
        ctx,
        size,
        &format!("Transition {} / {count}", s.transition + 1),
    );
    let (_, texts, _) = frame(s, ctx, size, vec![]);
    let label = texts
        .iter()
        .find(|(t, _)| t.starts_with(&format!("{} · ", index + 1)) && t.contains("(ID "))
        .unwrap()
        .0
        .clone();
    click_last(s, ctx, size, &label);
}
fn values(s: &mut Session, ctx: &egui::Context, size: egui::Vec2, start: &str, end: &str) {
    click_last(s, ctx, size, "Edit values");
    click_last(s, ctx, size, "Hermite");
    click_last(s, ctx, size, "Quintic smoothstep");
    type_after_label(s, ctx, size, "Start", start);
    type_after_label(s, ctx, size, "End", end);
    let viewport = frame(s, ctx, size, vec![]).0.viewport;
    click_at(s, ctx, size, viewport.center());
}
fn split(s: &mut Session, ctx: &egui::Context, size: egui::Vec2) {
    click_last(s, ctx, size, "Transitions");
    click_last(s, ctx, size, "Split into two Hermite regions");
    let viewport = frame(s, ctx, size, vec![]).0.viewport;
    click_at(s, ctx, size, viewport.center());
}

#[test]
fn complete_hill_can_be_authored_through_real_workbench_widgets_and_saved() {
    let mut s = blank();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    click(&mut s, &ctx, size, "Insert section…");
    click(&mut s, &ctx, size, "Insert after");
    type_after_label(&mut s, &ctx, size, "Length (m)", "80");
    assert_eq!(s.draft.extent, 80.);
    split(&mut s, &ctx, size);
    split(&mut s, &ctx, size);
    select_transition(&mut s, &ctx, size, 2);
    split(&mut s, &ctx, size);
    for (index, (start, end)) in [("1", "2"), ("2", "0"), ("0", "0"), ("0", "1")]
        .into_iter()
        .enumerate()
    {
        select_transition(&mut s, &ctx, size, index);
        values(&mut s, &ctx, size, start, end);
    }
    for (channel, peak) in [("Lateral force", "0.1"), ("Roll rate", "0.006")] {
        click_last(&mut s, &ctx, size, channel);
        split(&mut s, &ctx, size);
        select_transition(&mut s, &ctx, size, 0);
        values(&mut s, &ctx, size, "0", peak);
        select_transition(&mut s, &ctx, size, 1);
        values(&mut s, &ctx, size, peak, "0");
    }
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::CTRL);
    assert!(!s.draft_dirty(), "{}", s.status);
    click_last(&mut s, &ctx, size, "Normal force");
    select_transition(&mut s, &ctx, size, 0);
    click_last(
        &mut s,
        &ctx,
        size,
        "Precision editor · values, boundaries and transition tools",
    );
    type_after_label(&mut s, &ctx, size, "End boundary", "22");
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::CTRL);
    let expected = s.editor.project().clone();
    key(&mut s, &ctx, size, egui::Key::Z, Modifiers::CTRL);
    assert_eq!(
        s.editor.project().sections[1].channels()[1].1.transitions[0].length,
        20.
    );
    key(&mut s, &ctx, size, egui::Key::Y, Modifiers::CTRL);
    assert_eq!(s.editor.project(), &expected);
    let generated = expected.generate(&s.catalog, s.options).unwrap();
    let fixture = scenario::build().unwrap();
    assert_eq!(
        generated,
        fixture
            .editor
            .project()
            .generate(&fixture.catalog, fixture.options)
            .unwrap()
    );
    let directory = std::env::temp_dir().join(format!("qx-m6-ui-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("ui-authored.qx.json");
    s.save_to(&path);
    assert!(!s.dirty());
    s.open(Some(path.clone()), true);
    assert_eq!(s.editor.project(), &expected);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn escape_in_graph_menu_keeps_the_authored_draft() {
    let mut s = blank();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    s.draft.name = "Keep this draft".into();
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    click_last(&mut s, &ctx, size, "Edit values");
    let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
    assert!(
        ctx.any_popup_open(),
        "{:?}",
        texts
            .iter()
            .filter(|(t, _)| t.contains("Edit")
                || t.contains("Authored")
                || t.contains("Start")
                || t.contains("Constant"))
            .collect::<Vec<_>>()
    );
    key(&mut s, &ctx, size, egui::Key::Escape, Modifiers::NONE);
    assert_eq!(s.draft.name, "Keep this draft");
    assert!(s.draft_dirty());
}

#[test]
fn numeric_focus_survives_draft_notice_and_enter_apply_cancel_escape_are_distinct() {
    let mut s = blank();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
    let label = texts.iter().find(|(t, _)| t == "Length (m)").unwrap().1;
    click_at(
        &mut s,
        &ctx,
        size,
        egui::pos2(label.right() + 24., label.center().y),
    );
    let field = ctx.memory(|m| m.focused()).unwrap();
    frame(
        &mut s,
        &ctx,
        size,
        vec![Event::Text("12.3456789012".into())],
    );
    let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
    assert!(texts.iter().any(|(t, _)| t.contains("unapplied draft")));
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(field),
        "the newly inserted draft notice must not move keyboard focus to Cancel"
    );
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::NONE);
    assert!((s.draft.extent - 12.3456789012).abs() < 1e-12);
    assert_eq!(s.editor.project().sections[0].extent, 10.);
    assert!(!s.editor.can_undo());
    click_last(&mut s, &ctx, size, "Apply");
    assert!(!s.draft_dirty());
    assert!((s.editor.project().sections[0].extent - 12.3456789012).abs() < 1e-12);
    type_after_label(&mut s, &ctx, size, "Length (m)", "18.125");
    click_last(&mut s, &ctx, size, "Cancel");
    assert!((s.draft.extent - 12.3456789012).abs() < 1e-12);
    let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
    let label = texts.iter().find(|(t, _)| t == "Length (m)").unwrap().1;
    click_at(
        &mut s,
        &ctx,
        size,
        egui::pos2(label.right() + 24., label.center().y),
    );
    frame(&mut s, &ctx, size, vec![Event::Text("21".into())]);
    key(&mut s, &ctx, size, egui::Key::Escape, Modifiers::NONE);
    assert_eq!(
        s.draft.extent, 21.,
        "Escape leaving a numeric editor must not also cancel the section"
    );
    let viewport = frame(&mut s, &ctx, size, vec![]).0.viewport;
    click_at(&mut s, &ctx, size, viewport.center());
    key(&mut s, &ctx, size, egui::Key::Escape, Modifiers::NONE);
    assert!((s.draft.extent - 12.3456789012).abs() < 1e-12);
    s.undo(false);
    assert_eq!(s.editor.project().sections[0].extent, 10.);
    assert!(
        !s.editor.can_undo(),
        "only the explicit Apply belongs in document history"
    );
}

#[test]
fn typing_and_ctrl_enter_in_one_input_batch_commit_the_new_numeric_value() {
    let mut s = blank();
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    let (_, texts, _) = frame(&mut s, &ctx, size, vec![]);
    let label = texts.iter().find(|(t, _)| t == "Length (m)").unwrap().1;
    click_at(
        &mut s,
        &ctx,
        size,
        egui::pos2(label.right() + 24., label.center().y),
    );
    frame(
        &mut s,
        &ctx,
        size,
        vec![
            Event::Text("27.125".into()),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL,
            },
        ],
    );
    assert_eq!(s.editor.project().sections[0].extent, 27.125);
    assert!(!s.draft_dirty());
    assert!(s.editor.can_undo());
}

#[test]
fn insertion_type_submenu_keeps_its_parent_and_creates_the_chosen_method() {
    for (kind, label) in [
        (0, "Force-vector"),
        (1, "Rider-local geometric"),
        (2, "World-geometric"),
        (3, "Straight"),
        (4, "Circular arc"),
    ] {
        let mut s = blank();
        let ctx = egui::Context::default();
        let size = egui::vec2(960., 640.);
        frame(&mut s, &ctx, size, vec![]);
        frame(&mut s, &ctx, size, vec![]);
        click_last(&mut s, &ctx, size, "Insert section…");
        click_last(&mut s, &ctx, size, "Force-vector");
        click_last(&mut s, &ctx, size, label);
        assert_eq!(s.new_kind, kind);
        click_last(&mut s, &ctx, size, "Insert after");
        assert_eq!(s.editor.project().sections.len(), 2);
        assert_eq!(
            std::mem::discriminant(&s.draft.kind),
            std::mem::discriminant(&panels::new_section(2, kind).kind)
        );
    }
}

#[test]
fn numerically_invalid_open_retains_the_existing_document_draft_and_path() {
    let mut s = blank();
    let previous = s.editor.project().clone();
    s.draft.name = "Keep authored values".into();
    let draft = s.draft.clone();
    let mut invalid = previous.clone();
    invalid.sections[0].kind = SectionKind::Arc {
        radius_m: 0.0001,
        plane_angle_rad: 0.,
    };
    let path = std::env::temp_dir().join(format!("qx-m6-invalid-{}.qx.json", std::process::id()));
    persistence::save(&path, &invalid, &s.catalog).unwrap();
    s.open(Some(path.clone()), true);
    assert_eq!(s.editor.project(), &previous);
    assert_eq!(s.draft, draft);
    assert!(s.path.is_empty());
    assert!(s.status.contains("Open failed") && s.status.contains("rotation"));
    assert_eq!(persistence::load(&path, &s.catalog).unwrap(), invalid);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn amplitude_hermite_slopes_transition_order_and_removal_work_through_controls() {
    let mut s = blank();
    panels::insert_section(&mut s, 0, false);
    let ctx = egui::Context::default();
    let size = egui::vec2(1440., 960.);
    frame(&mut s, &ctx, size, vec![]);
    frame(&mut s, &ctx, size, vec![]);
    let roll = s.draft.roll_rate.clone();
    click_last(&mut s, &ctx, size, "Edit values");
    click_last(&mut s, &ctx, size, "Quintic smoothstep");
    click_last(&mut s, &ctx, size, "Hermite");
    type_after_label(&mut s, &ctx, size, "Amplitude", "0.35");
    type_after_label(&mut s, &ctx, size, "Start slope", "0.01");
    type_after_label(&mut s, &ctx, size, "End slope", "-0.01");
    let authored = s.draft.clone();
    click_last(&mut s, &ctx, size, "Hermite");
    click_last(&mut s, &ctx, size, "Hermite");
    assert_eq!(
        s.draft, authored,
        "reselecting the active function must not erase Hermite derivatives"
    );
    let viewport = frame(&mut s, &ctx, size, vec![]).0.viewport;
    click_at(&mut s, &ctx, size, viewport.center());
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::CTRL);
    let normal = s.draft.channel_mut(ChannelKey::Normal).unwrap().clone();
    assert!((normal.evaluate(5.).unwrap().value - 1.375).abs() < 1e-12);
    assert!((normal.evaluate(0.).unwrap().derivative - 0.01).abs() < 1e-12);
    assert!((normal.evaluate(10.).unwrap().derivative + 0.01).abs() < 1e-12);
    assert!(!s.draft_dirty());
    split(&mut s, &ctx, size);
    let ids = s
        .draft
        .channel_mut(ChannelKey::Normal)
        .unwrap()
        .transitions
        .iter()
        .map(|t| t.id)
        .collect::<Vec<_>>();
    click_last(&mut s, &ctx, size, "Transitions");
    click_last(&mut s, &ctx, size, "Transition later");
    assert_eq!(s.transition, 1);
    assert_eq!(
        s.draft
            .channel_mut(ChannelKey::Normal)
            .unwrap()
            .transitions
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    click_at(&mut s, &ctx, size, viewport.center());
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::CTRL);
    let reordered = s.editor.project().clone();
    click_last(&mut s, &ctx, size, "Transitions");
    click_last(&mut s, &ctx, size, "Remove and extend neighbor");
    click_at(&mut s, &ctx, size, viewport.center());
    key(&mut s, &ctx, size, egui::Key::Enter, Modifiers::CTRL);
    let removed = s.editor.project().clone();
    assert_eq!(
        s.draft
            .channel_mut(ChannelKey::Normal)
            .unwrap()
            .transitions
            .len(),
        1
    );
    assert_eq!(
        s.draft.channel_mut(ChannelKey::Normal).unwrap().length(),
        10.
    );
    assert_eq!(s.draft.roll_rate, roll);
    s.undo(false);
    assert_eq!(s.editor.project(), &reordered);
    s.undo(true);
    assert_eq!(s.editor.project(), &removed);
    click_last(&mut s, &ctx, size, "Transitions");
    click_last(&mut s, &ctx, size, "Remove and extend neighbor");
    assert!(s.status.contains("at least one transition"));
    assert_eq!(s.editor.project(), &removed);
    assert!(!s.draft_dirty());
}
