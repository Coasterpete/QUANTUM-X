//! Real egui event sequences; these never send input to the Windows desktop.
use super::{
    graph_canvas,
    m5_tests::pointer,
    panels,
    session::{Session, scene},
    workbench::{Pane, Profile},
};
use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2};
use qx_geometry::authoring::{ChannelKey, SectionId};
use qx_rendering::{DisplayOptions, InspectionScene};
use std::time::{Duration, Instant};

struct Harness {
    session: Session,
    ctx: egui::Context,
    size: Vec2,
    scene: InspectionScene,
    texts: Vec<String>,
    cursor: egui::CursorIcon,
    canvas_rect: Rect,
    handles: Vec<((u64, Option<bool>), Rect)>,
    focused: bool,
}
impl Harness {
    fn new(key: ChannelKey, scale: f32, floating: bool) -> Self {
        Self::layout(key, scale, if floating { 1 } else { 0 })
    }
    fn layout(key: ChannelKey, scale: f32, layout: u8) -> Self {
        let mut session = super::m5_tests::session();
        session.select(SectionId(2));
        session.select_channel(key);
        session.workbench.window.ui_scale = scale;
        if layout == 1 {
            session.workbench.layouts[Profile::Track.index()].float(Pane::Transition);
        } else if layout == 2 {
            // A real alternate dock tree with the Transition Editor on the left.
            let mut tiles = egui_tiles::Tiles::default();
            let graph = tiles.insert_pane(Pane::Transition);
            let viewport = tiles.insert_pane(Pane::Viewport);
            let track = tiles.insert_pane(Pane::Track);
            let support = tiles.insert_pane(Pane::Support);
            let root = tiles.insert_horizontal_tile(vec![graph, viewport, track, support]);
            session.workbench.layouts[0].tree =
                egui_tiles::Tree::new(egui::Id::unique("drag test side dock"), root, tiles);
        }
        let scene = scene(session.editor.project(), &session.catalog, session.options).unwrap();
        let mut h = Self {
            session,
            ctx: egui::Context::default(),
            size: egui::vec2(1600., 1000.),
            scene,
            texts: vec![],
            cursor: egui::CursorIcon::Default,
            canvas_rect: Rect::NOTHING,
            handles: vec![],
            focused: true,
        };
        for _ in 0..5 {
            h.frame(vec![]);
        }
        h
    }
    fn frame(&mut self, events: Vec<Event>) {
        let mut output = self.ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
                events,
                focused: self.focused,
                ..Default::default()
            },
            |ctx| {
                panels::workspace(
                    &mut self.session,
                    ctx,
                    &self.scene,
                    &mut DisplayOptions::default(),
                );
                // Read current-pass widget rectangles before egui swaps pass state.
                self.canvas_rect = ctx
                    .read_response(egui::Id::unique((
                        "graph canvas",
                        2_u64,
                        self.session.channel as u8,
                    )))
                    .map_or(Rect::NOTHING, |r| r.rect);
                self.handles.clear();
                for transition in 1..=2_u64 {
                    for end in [Some(false), Some(true), None] {
                        let id = if let Some(end) = end {
                            egui::Id::unique((
                                "graph endpoint",
                                2_u64,
                                self.session.channel as u8,
                                transition,
                                end,
                            ))
                        } else {
                            egui::Id::unique((
                                "boundary",
                                2_u64,
                                self.session.channel as u8,
                                transition,
                            ))
                        };
                        if let Some(r) = ctx.read_response(id) {
                            self.handles.push(((transition, end), r.rect));
                        }
                    }
                }
            },
        );
        self.texts = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        self.cursor = output.platform_output.cursor_icon;
        output.textures_delta.clear();
    }
    fn target(&self, transition: u64, end: Option<bool>) -> Pos2 {
        let rect = self
            .handles
            .iter()
            .find(|(key, _)| *key == (transition, end))
            .expect("rendered handle")
            .1;
        assert!(
            rect.is_positive() && rect.is_finite(),
            "handle must be reachable: {rect:?}"
        );
        rect.center()
    }
    fn press(&mut self, pos: Pos2) {
        self.frame(vec![
            Event::PointerMoved(pos),
            pointer(pos, PointerButton::Primary, true),
        ]);
    }
    fn move_to(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
    }
    fn release(&mut self, pos: Pos2) {
        self.frame(vec![
            Event::PointerMoved(pos),
            pointer(pos, PointerButton::Primary, false),
        ]);
    }
    fn key(&mut self, key: egui::Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
    }
    fn value(&mut self, index: usize, end: bool) -> f64 {
        let t = &self
            .session
            .draft
            .channel_mut(self.session.channel)
            .unwrap()
            .transitions[index];
        if end { t.end } else { t.start }
    }
    fn canvas(&self) -> Rect {
        self.canvas_rect
    }
    fn wait(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(scene) = self.session.poll() {
                self.scene = scene;
                return;
            }
            assert!(
                !self.session.status.starts_with("Generation failed"),
                "{}",
                self.session.status
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("generation timed out: {}", self.session.status);
    }
}

#[test]
fn multi_frame_drag_holds_value_and_applies_total_displacement_once() {
    let mut h = Harness::new(ChannelKey::Normal, 1., false);
    let pos = h.target(1, Some(true));
    let start = h.value(0, true);
    let per_point =
        h.session.graph_view.view.y.span() / graph_canvas::plot_rect(h.canvas()).height() as f64;
    h.press(pos);
    h.move_to(pos - egui::vec2(0., 12.));
    h.move_to(pos - egui::vec2(0., 28.));
    let expected = start + 28. * per_point;
    assert!(
        (h.value(0, true) - expected).abs() < 1e-10,
        "must use displacement since press"
    );
    let revision = h.session.revision;
    for _ in 0..4 {
        h.frame(vec![]);
        assert!(
            (h.value(0, true) - expected).abs() < 1e-10,
            "pause must retain value"
        );
    }
    assert_eq!(
        h.session.revision, revision,
        "stationary frames cannot regenerate"
    );
    h.wait();
    let preview = h.scene.track.clone();
    let original = h.session.editor.project().clone();
    assert_ne!(
        preview,
        original
            .generate(&h.session.catalog, h.session.options)
            .unwrap()
    );
    assert!(h.session.visible_preview);
    h.release(pos - egui::vec2(0., 28.));
    assert!((h.value(0, true) - expected).abs() < 1e-10);
    assert!(!h.session.draft_dirty());
    assert_eq!(
        h.session.revision, revision,
        "release must reuse identical preview"
    );
    assert!(!h.session.visible_preview);
    let committed = h.session.editor.project().clone();
    assert_eq!(
        preview,
        committed
            .generate(&h.session.catalog, h.session.options)
            .unwrap()
    );
    h.key(egui::Key::Z, Modifiers::CTRL);
    assert_eq!(h.session.editor.project(), &original);
    assert!(
        !h.session.editor.can_undo(),
        "one gesture is one undo transaction"
    );
    h.key(egui::Key::Y, Modifiers::CTRL);
    assert_eq!(h.session.editor.project(), &committed);
}

#[test]
fn both_channels_all_endpoint_sides_at_five_scales_and_three_dock_layouts() {
    for key in [ChannelKey::Normal, ChannelKey::Roll] {
        for scale in [0.75, 1., 1.25, 1.5, 2.] {
            for layout in 0..3 {
                for (index, end) in [(0, false), (0, true), (1, false), (1, true)] {
                    let mut h = Harness::layout(key, scale, layout);
                    let original = h.session.editor.project().clone();
                    let before = h.session.draft.clone();
                    let pos = h.target(index as u64 + 1, Some(end));
                    let value = h.value(index, end);
                    h.move_to(pos);
                    assert_eq!(
                        h.cursor,
                        egui::CursorIcon::ResizeVertical,
                        "{key:?} {scale} {layout} {index} {end} pos {pos:?}, canvas {:?}, response {:?}",
                        h.canvas(),
                        h.ctx.read_response(egui::Id::unique((
                            "graph endpoint",
                            2_u64,
                            key as u8,
                            index as u64 + 1,
                            end
                        )))
                    );
                    h.press(pos);
                    // Floating resize constraints can settle between passes;
                    // authoring must use the mapping actually captured on press.
                    let factor = h.session.graph_view.view.y.span()
                        / graph_canvas::plot_rect(h.canvas()).height() as f64;
                    assert_eq!(h.session.transition, index);
                    assert!(
                        h.session.graph_drag.is_some(),
                        "capture on press: {key:?} {scale} {layout} {index} {end}"
                    );
                    h.move_to(pos - egui::vec2(0., 10.));
                    h.frame(vec![]);
                    h.release(pos - egui::vec2(0., 17.)); // release batch includes final motion
                    let mut expected = before;
                    let t = &mut expected.channel_mut(key).unwrap().transitions[index];
                    if end {
                        t.end = value + 17. * factor;
                    } else {
                        t.start = value + 17. * factor;
                    }
                    assert_eq!(
                        h.session.draft, expected,
                        "only target changes: {key:?} {scale} {layout} {index} {end}"
                    );
                    assert_eq!(h.session.selected, SectionId(2));
                    assert_eq!(h.session.workbench.active, Profile::Track);
                    assert!(!h.session.draft_dirty(), "{}", h.session.status);
                    h.session.undo(false);
                    assert_eq!(h.session.editor.project(), &original);
                    assert!(!h.session.editor.can_undo());
                }
            }
        }
    }
}

#[test]
fn diamond_drags_only_the_two_neighbor_lengths_at_every_scale_and_layout() {
    for key in [ChannelKey::Normal, ChannelKey::Roll] {
        for scale in [0.75, 1., 1.25, 1.5, 2.] {
            for layout in 0..3 {
                let mut h = Harness::layout(key, scale, layout);
                let before = h.session.draft.clone();
                let pos = h.target(1, None);
                let factor = h.session.graph_view.view.x.span()
                    / graph_canvas::plot_rect(h.canvas()).width() as f64;
                h.press(pos);
                h.move_to(pos + egui::vec2(10., 12.));
                h.move_to(pos + egui::vec2(17., -35.));
                let pending = h.session.draft.clone();
                let revision = h.session.revision;
                for _ in 0..3 {
                    h.frame(vec![]);
                }
                assert_eq!(h.session.draft, pending);
                assert_eq!(h.session.revision, revision);
                h.release(pos + egui::vec2(23., -35.));
                let expected = graph_canvas::boundary(&before, key, 0, 30. + 23. * factor).unwrap();
                assert_eq!(
                    h.session.draft, expected,
                    "{key:?} scale {scale}, layout {layout}"
                );
                assert!(!h.session.draft_dirty());
                assert_eq!(h.session.transition, 0);
            }
        }
    }
}

#[test]
fn capture_survives_graph_exit_pointer_gone_and_release_over_another_pane() {
    for key in [ChannelKey::Normal, ChannelKey::Roll] {
        for floating in [false, true] {
            let mut h = Harness::new(key, 1.25, floating);
            let original = h.session.editor.project().clone();
            let pos = h.target(2, Some(true));
            let value = h.value(1, true);
            let outside = egui::pos2(h.canvas().right() + 90., h.canvas().top() - 70.);
            h.press(pos);
            let factor = h.session.graph_view.view.y.span()
                / graph_canvas::plot_rect(h.canvas()).height() as f64;
            let target = h.session.graph_drag.as_ref().unwrap().target;
            h.move_to(outside);
            h.frame(vec![Event::PointerGone]);
            h.frame(vec![]);
            assert_eq!(h.session.graph_drag.as_ref().unwrap().target, target);
            assert!(
                h.texts
                    .iter()
                    .any(|s| s.contains("Dragging Transition 2 end:")),
                "active readout outside plot"
            );
            h.release(outside - egui::vec2(10., 9.));
            let expected = value - ((outside.y - 9.) - pos.y) as f64 * factor;
            assert!(
                (h.value(1, true) - expected).abs() < 1e-10,
                "{key:?} floating {floating}: expected {expected}, got {}: {} {:?}",
                h.value(1, true),
                h.session.status,
                h.session.graph_feedback
            );
            assert!(h.session.graph_drag.is_none());
            assert_eq!(h.session.selected, SectionId(2));
            assert!(!h.session.draft_dirty());
            h.session.undo(false);
            assert_eq!(h.session.editor.project(), &original);
        }
    }
}

#[test]
fn existing_drafts_stage_valid_release_and_escape_restores_exact_prepress_state() {
    for key in [ChannelKey::Normal, ChannelKey::Roll] {
        for cancel in [false, true] {
            let mut h = Harness::new(key, 1.5, true);
            h.session.draft.name = "Owner's preceding section draft".into();
            h.session.initial_draft.position_m.y = 3.;
            h.session.metadata_draft.author = "Pending author".into();
            h.frame(vec![]);
            let before = h.session.draft.clone();
            let original = h.session.editor.project().clone();
            let pos = h.target(2, Some(false));
            h.press(pos);
            let moved = pos - egui::vec2(0., 26.);
            h.move_to(moved);
            assert_ne!(h.session.draft, before);
            if cancel {
                h.key(egui::Key::Escape, Modifiers::NONE);
                h.move_to(moved - egui::vec2(0., 18.)); // same held button cannot re-capture
            }
            h.release(moved);
            assert_eq!(h.session.editor.project(), &original);
            assert!(!h.session.editor.can_undo());
            assert_eq!(h.session.initial_draft.position_m.y, 3.);
            assert_eq!(h.session.metadata_draft.author, "Pending author");
            if cancel {
                assert_eq!(h.session.draft, before);
            } else {
                assert_eq!(h.session.draft.name, before.name);
                assert_ne!(h.session.draft, before);
                assert!(h.session.apply_draft());
                // Remove the separately staged settings before document undo.
                h.session.initial_draft = original.initial.clone();
                h.session.metadata_draft = original.metadata.clone();
                h.session.undo(false);
                assert_eq!(h.session.editor.project(), &original);
                assert!(!h.session.editor.can_undo());
            }
        }
    }
}

#[test]
fn click_threshold_and_return_to_origin_create_no_history_or_regeneration() {
    let mut h = Harness::new(ChannelKey::Normal, 1., false);
    let before = h.session.draft.clone();
    let pos = h.target(1, Some(true));
    let revision = h.session.revision;
    h.press(pos);
    h.move_to(pos - egui::vec2(0., 1.));
    assert_eq!(h.session.draft, before);
    h.release(pos - egui::vec2(0., 1.));
    assert_eq!(h.session.draft, before);
    assert_eq!(h.session.revision, revision);
    assert!(!h.session.editor.can_undo());
    h.press(pos);
    h.key(egui::Key::Escape, Modifiers::NONE);
    h.release(pos);
    assert_eq!(h.session.draft, before);
    assert_eq!(
        h.session.revision, revision,
        "canceling a click cannot regenerate"
    );
    h.press(pos);
    h.move_to(pos - egui::vec2(0., 25.));
    h.release(pos);
    assert_eq!(h.session.draft, before);
    assert!(!h.session.editor.can_undo());
}

#[test]
fn invalid_boundary_retains_last_valid_draft_reports_reason_and_can_recover() {
    for recover in [false, true] {
        let mut h = Harness::new(ChannelKey::Roll, 1., false);
        h.session.draft.name = "Keep preceding draft".into();
        h.frame(vec![]);
        let before = h.session.draft.clone();
        let original = h.session.editor.project().clone();
        let pos = h.target(1, None);
        h.press(pos);
        h.move_to(pos + egui::vec2(12., 0.));
        let valid = h.session.draft.clone();
        h.wait();
        let geometry = h.scene.track.clone();
        let invalid = pos - egui::vec2(h.canvas().width() * 2., 0.);
        h.move_to(invalid);
        h.frame(vec![]);
        assert_eq!(h.session.draft, valid);
        assert_eq!(h.scene.track, geometry);
        assert!(
            h.session
                .graph_feedback
                .as_ref()
                .unwrap()
                .contains("at least 1e-8")
        );
        assert_eq!(h.cursor, egui::CursorIcon::NotAllowed);
        if recover {
            h.move_to(pos + egui::vec2(18., 0.));
            assert!(h.session.graph_feedback.is_none());
            h.release(pos + egui::vec2(18., 0.));
            assert_ne!(h.session.draft, before);
        } else {
            h.release(invalid);
            assert_eq!(h.session.draft, before);
            assert!(
                h.session
                    .graph_feedback
                    .as_ref()
                    .unwrap()
                    .contains("Graph drag rejected:")
            );
            h.wait();
            assert!(
                h.session
                    .graph_feedback
                    .as_ref()
                    .unwrap()
                    .contains("at least 1e-8"),
                "worker cannot erase rejection"
            );
            assert_eq!(
                h.scene.track,
                h.session
                    .preview_project()
                    .generate(&h.session.catalog, h.session.options)
                    .unwrap()
            );
        }
        assert_eq!(h.session.editor.project(), &original);
        assert!(!h.session.editor.can_undo());
    }
}

#[test]
fn out_of_range_endpoint_is_rejected_without_replacing_draft_or_geometry() {
    let mut h = Harness::new(ChannelKey::Normal, 1., false);
    h.session
        .graph_view
        .set(super::graph_view::View::exact([0., 60., -1e14, 1e14], 60.).unwrap());
    for _ in 0..3 {
        h.frame(vec![]);
    }
    let before = h.session.draft.clone();
    let geometry = h.scene.track.clone();
    let pos = h.target(1, Some(true));
    h.press(pos);
    let invalid = pos - egui::vec2(0., 50.);
    h.move_to(invalid);
    assert_eq!(h.session.draft, before);
    assert_eq!(h.scene.track, geometry);
    assert!(
        h.session
            .graph_feedback
            .as_ref()
            .unwrap()
            .contains("bounded by 1e12")
    );
    h.release(invalid);
    assert_eq!(h.session.draft, before);
    assert!(!h.session.editor.can_undo());
}

#[test]
fn strict_connection_failure_on_release_rolls_back_without_a_history_entry() {
    let mut h = Harness::new(ChannelKey::Normal, 1., false);
    let mut project = h.session.editor.project().clone();
    project.sections.truncate(2);
    let mut following = qx_document::insertion::initialize(
        &project,
        &project
            .generate(&h.session.catalog, h.session.options)
            .unwrap(),
        2,
        panels::new_section(3, 0),
    )
    .unwrap();
    following.incoming_boundary = qx_geometry::authoring::BoundaryPolicy::RequireContinuousRates;
    project.sections.push(following);
    h.session = Session::new(project.clone(), None, h.session.catalog.clone()).unwrap();
    h.session.select(SectionId(2));
    h.session.select_channel(ChannelKey::Normal);
    h.scene = scene(&project, &h.session.catalog, h.session.options).unwrap();
    for _ in 0..4 {
        h.frame(vec![]);
    }
    let before = h.session.draft.clone();
    let pos = h.target(2, Some(true));
    h.press(pos);
    h.move_to(pos - egui::vec2(0., 20.));
    h.release(pos - egui::vec2(0., 20.));
    assert_eq!(h.session.editor.project(), &project);
    assert_eq!(h.session.draft, before);
    let error = h.session.graph_feedback.as_ref().unwrap();
    assert!(
        error.contains("curvature gap") && error.contains("section 3"),
        "{error}"
    );
    assert!(!h.session.editor.can_undo());
    h.wait();
    assert_eq!(
        h.scene.track,
        project
            .generate(&h.session.catalog, h.session.options)
            .unwrap()
    );
    assert!(
        h.session
            .graph_feedback
            .as_ref()
            .unwrap()
            .contains("curvature gap")
    );
}

#[test]
fn preview_role_change_is_reused_even_when_worker_has_not_published_yet() {
    let mut h = Harness::new(ChannelKey::Roll, 1., false);
    let pos = h.target(1, Some(true));
    h.press(pos);
    h.move_to(pos - egui::vec2(0., 22.));
    let revision = h.session.revision;
    h.release(pos - egui::vec2(0., 22.));
    assert_eq!(h.session.revision, revision);
    h.wait();
    assert_eq!(h.session.visible_revision, revision);
    assert!(!h.session.visible_preview);
    assert!(h.session.status.starts_with("Document"));
}

#[test]
fn crossing_adjacent_endpoint_and_boundary_targets_cannot_change_drag_identity() {
    for key in [ChannelKey::Normal, ChannelKey::Roll] {
        let mut h = Harness::new(key, 1., false);
        let before = h.session.draft.clone();
        let pos = h.target(1, Some(true));
        let neighbor = h.target(2, Some(false));
        assert!(
            neighbor.x - pos.x >= 24.,
            "shared join targets must not overlap"
        );
        h.press(pos);
        let factor = h.session.graph_view.view.y.span()
            / graph_canvas::plot_rect(h.canvas()).height() as f64;
        let target = h.session.graph_drag.as_ref().unwrap().target;
        h.move_to(neighbor - egui::vec2(0., 12.));
        h.move_to(h.target(1, None));
        assert_eq!(h.session.graph_drag.as_ref().unwrap().target, target);
        assert_eq!(h.session.transition, 0);
        let final_pos = neighbor - egui::vec2(0., 20.);
        h.release(final_pos);
        let mut expected = before;
        expected.channel_mut(key).unwrap().transitions[0].end -=
            (final_pos.y - pos.y) as f64 * factor;
        assert_eq!(h.session.draft, expected);
    }
}

#[test]
fn escape_and_focus_loss_cancel_clean_gestures_without_recapture_or_undo() {
    for focus_loss in [false, true] {
        let mut h = Harness::new(ChannelKey::Roll, 1., false);
        let before = h.session.draft.clone();
        let pos = h.target(1, Some(true));
        h.press(pos);
        h.move_to(pos - egui::vec2(0., 24.));
        assert_ne!(h.session.draft, before);
        if focus_loss {
            h.focused = false;
            h.frame(vec![Event::WindowFocused(false)]);
            h.focused = true;
            h.frame(vec![Event::WindowFocused(true)]);
        } else {
            h.key(egui::Key::Escape, Modifiers::NONE);
        }
        assert!(h.session.graph_drag.is_none());
        h.move_to(pos - egui::vec2(0., 30.));
        h.release(pos - egui::vec2(0., 30.));
        assert_eq!(h.session.draft, before);
        assert!(!h.session.editor.can_undo());
        assert!(!h.session.draft_dirty());
    }
}

#[test]
fn gesture_keeps_press_scale_when_workspace_resizes_and_finishes_without_visible_pane() {
    let mut h = Harness::new(ChannelKey::Normal, 1., false);
    let pos = h.target(1, Some(true));
    let value = h.value(0, true);
    h.press(pos);
    let factor =
        h.session.graph_view.view.y.span() / graph_canvas::plot_rect(h.canvas()).height() as f64;
    h.move_to(pos - egui::vec2(0., 12.));
    h.size = egui::vec2(1200., 800.);
    h.move_to(pos - egui::vec2(0., 20.));
    assert!((h.value(0, true) - (value + 20. * factor)).abs() < 1e-12);
    let pane = h.session.workbench.layouts[0]
        .pane_id(Pane::Transition)
        .unwrap();
    h.session.workbench.layouts[0].tree.set_visible(pane, false);
    h.release(pos - egui::vec2(0., 28.));
    assert!((h.value(0, true) - (value + 28. * factor)).abs() < 1e-12);
    assert!(h.session.graph_drag.is_none());
    assert!(!h.session.draft_dirty());
}

#[test]
fn endpoint_drag_preserves_every_transition_function_and_constant_semantics() {
    use qx_geometry::transition::Function;
    for function in [
        Function::Constant {},
        Function::Linear {},
        Function::Quadratic {},
        Function::CubicSmoothstep {},
        Function::QuinticSmoothstep {},
        Function::Cosine {},
        Function::Hermite {
            start_slope: 0.001,
            end_slope: -0.002,
        },
    ] {
        let mut h = Harness::new(ChannelKey::Roll, 1., false);
        let t = &mut h.session.draft.roll_rate.transitions[0];
        t.function = function.clone();
        if matches!(function, Function::Constant {}) {
            t.start = t.end;
        }
        let before = h.session.draft.clone();
        for _ in 0..2 {
            h.frame(vec![]);
        }
        let pos = h.target(1, Some(true));
        h.press(pos);
        let factor = h.session.graph_view.view.y.span()
            / graph_canvas::plot_rect(h.canvas()).height() as f64;
        h.move_to(pos - egui::vec2(0., 18.));
        h.release(pos - egui::vec2(0., 18.));
        let mut expected = before;
        let t = &mut expected.roll_rate.transitions[0];
        t.end += 18. * factor;
        if matches!(function, Function::Constant {}) {
            t.start = t.end;
        }
        assert_eq!(h.session.draft, expected);
        assert!(h.session.draft.validate().is_ok());
    }
}
#[test]
fn m7_real_graph_gestures_regenerate_solids_and_reuse_accepted_preview() {
    for channel in [ChannelKey::Normal, ChannelKey::Lateral, ChannelKey::Roll] {
        let mut h = Harness::new(channel, 1., false);
        h.session.regenerate();
        h.wait();
        let initial = h.scene.mesh.as_ref().unwrap().clone();
        let pos = h.target(1, Some(true));
        h.press(pos);
        h.move_to(pos - egui::vec2(0., 8.));
        h.wait();
        let preview = h.scene.mesh.as_ref().unwrap().clone();
        assert_ne!(initial.components[0].mesh, preview.components[0].mesh);
        assert!(h.session.visible_preview);
        let revision = h.session.revision;
        h.release(pos - egui::vec2(0., 8.));
        assert!(!h.session.visible_preview);
        assert_eq!(h.session.revision, revision);
        assert!(std::sync::Arc::ptr_eq(
            &preview,
            h.scene.mesh.as_ref().unwrap()
        ));
        h.session.undo(false);
        h.wait();
        assert_eq!(
            &initial.components,
            &h.scene.mesh.as_ref().unwrap().components
        );
        h.session.undo(true);
        h.wait();
        assert_eq!(
            &preview.components,
            &h.scene.mesh.as_ref().unwrap().components
        );
    }
}
