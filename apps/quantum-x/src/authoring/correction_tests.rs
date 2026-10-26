//! Headless input regressions for the owner's M5 correction recording.
use super::{
    graph_view::{self, View},
    m5_tests::{graph_frame, pointer, session},
    panels,
    plot::{Axis, Plot},
    session::Session,
};
use egui::{Modifiers, PointerButton, Pos2, Rect};
use qx_geometry::{
    authoring::{ChannelKey, Domain, SectionId},
    transition::{Channel, Unit},
};

fn wheel(delta: f32, modifiers: Modifiers) -> egui::Event {
    egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0., delta),
        modifiers,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
    }
}
#[test]
fn extreme_zoom_is_bounded_and_mapping_roundtrips_at_each_scale() {
    let mut view = View::exact([0., 60., -1., 3.], 60.).unwrap();
    for factor in [0.01, 100.] {
        for _ in 0..400 {
            view = view.zoom(
                (
                    view.x.min + view.x.span() * 0.3,
                    view.y.min + view.y.span() * 0.7,
                ),
                factor,
                60.,
            );
            assert_eq!(View::exact(view.bounds(), 60.).unwrap(), view);
            let rect = Rect::from_min_size(egui::pos2(93., 31.), egui::vec2(723., 187.));
            let plot = Plot::new(rect, [view.x.min, view.x.max], [view.y.min, view.y.max]).unwrap();
            for fraction in [0., 0.25, 0.5, 0.75, 1.] {
                let p = egui::pos2(
                    rect.left() + fraction * rect.width(),
                    rect.top() + fraction * rect.height(),
                );
                let (x, y) = plot.unmap(p);
                assert!((plot.map(x, y) - p).length() < 0.02, "{view:?} {p:?}");
            }
        }
    }
    assert_eq!(view.x, Axis::new(0., 60.).unwrap());
    assert!(view.y.min >= -1e24 && view.y.max <= 1e24);
}
#[test]
fn pointer_centered_zoom_preserves_both_fractions_until_a_domain_bound() {
    let view = View::exact([10., 50., -3., 7.], 60.).unwrap();
    for fraction in [0., 0.1, 0.5, 0.9, 1.] {
        let anchor = (
            view.x.min + fraction * view.x.span(),
            view.y.min + (1. - fraction) * view.y.span(),
        );
        let next = view.zoom(anchor, 0.7, 60.);
        assert!((next.x.fraction(anchor.0) - fraction).abs() < 1e-12);
        assert!((next.y.fraction(anchor.1) - (1. - fraction)).abs() < 1e-12);
    }
    let edge = view.pan((-10000., 0.), 60.);
    assert_eq!(edge.x, Axis::new(0., 40.).unwrap());
    let edge = view.pan((10000., 0.), 60.);
    assert_eq!(edge.x, Axis::new(20., 60.).unwrap());
}
#[test]
fn invalid_exact_ranges_retain_last_view_and_never_write_authored_data() {
    let mut s = session();
    let project = s.editor.project().clone();
    let draft = s.draft.clone();
    let before = s.graph_view.view;
    for bounds in [
        [0., 0., -1., 1.],
        [5., 2., -1., 1.],
        [-1., 5., -1., 1.],
        [0., 1000., -1., 1.],
        [0., 5., 2., 1.],
        [0., 5., f64::NAN, 1.],
        [0., f64::INFINITY, -1., 1.],
        [0., 5., 0., 1e-20],
        [0., 5., -1e25, 1e25],
    ] {
        s.graph_view.range_edit = bounds;
        s.graph_view.apply_range(s.draft.extent);
        assert!(s.graph_view.error.is_some());
        assert_eq!(s.graph_view.view, before);
    }
    s.graph_view.range_edit = [1.125, 5.25, -0.000001, 0.000003];
    s.graph_view.apply_range(s.draft.extent);
    assert_eq!(s.graph_view.view.bounds(), s.graph_view.range_edit);
    assert!(s.graph_view.error.is_none());
    assert_eq!(s.editor.project(), &project);
    assert_eq!(s.draft, draft);
    assert!(!s.editor.can_undo());
    for extent in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(View::exact(before.bounds(), extent).is_err());
        assert_eq!(before.zoom((0., 0.), 0.1, extent), before);
        assert_eq!(before.pan((1., 1.), extent), before);
    }
}
#[test]
fn zero_fit_has_an_explicit_useful_scale_and_nonlinear_fit_includes_the_bump() {
    let zero = Channel::constant(Unit::RadiansPerMetre, 10., 0.);
    assert_eq!(View::fit(&zero).unwrap().bounds(), [0., 10., -0.05, 0.05]);
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let channel = s.draft.channel_mut(ChannelKey::Normal).unwrap();
    channel.transitions[0].amplitude = 2.;
    let view = View::fit(channel).unwrap();
    assert!(view.y.max > 3.35);
    for t in &channel.transitions {
        for i in 0..=1000 {
            let value = t.evaluate(t.length * i as f64 / 1000.).unwrap().value;
            assert!(value >= view.y.min && value <= view.y.max);
        }
    }
}
#[test]
fn section_channel_and_domain_switch_rules_preserve_drafts_and_remember_views() {
    let mut s = session();
    let zero_view = s.graph_view.view;
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let force_view = View::exact([5., 40., 0.5, 2.5], 60.).unwrap();
    s.graph_view.set(force_view);
    s.draft.name = "Precise staged owner values".into();
    let draft = s.draft.clone();
    s.select_channel(ChannelKey::Roll);
    assert!(s.graph_view.view.y.max < 0.1);
    s.graph_view
        .set(View::exact([0., 60., -0.2, 0.2], 60.).unwrap());
    s.select_channel(ChannelKey::Normal);
    assert_eq!(s.graph_view.view, force_view);
    assert_eq!(s.draft, draft);
    s.select(SectionId(1));
    assert_eq!(s.selected, SectionId(2));
    s.sync_draft();
    s.select_channel(ChannelKey::Roll);
    s.select(SectionId(1));
    assert_eq!(s.graph_view.view, zero_view);
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    assert_eq!(s.graph_view.view, force_view);
    s.draft = qx_document::editing::convert_domain(&s.draft, Domain::Time, 20.).unwrap();
    graph_view::sync(&mut s);
    assert_eq!(s.graph_view.view.x, Axis::new(0., 3.).unwrap());
    assert_ne!(s.graph_view.view, force_view);
    assert!(!s.editor.can_undo());
}
#[test]
fn middle_drag_on_a_value_handle_is_view_only_and_cannot_create_a_draft() {
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let ctx = egui::Context::default();
    graph_frame(&mut s, &ctx, vec![], Modifiers::NONE);
    let (pos, _, _) = graph_frame(&mut s, &ctx, vec![], Modifiers::NONE);
    let draft = s.draft.clone();
    let project = s.editor.project().clone();
    let view = s.graph_view.view;
    graph_frame(
        &mut s,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, PointerButton::Middle, true),
        ],
        Modifiers::NONE,
    );
    let moved = pos + egui::vec2(13., 18.);
    graph_frame(
        &mut s,
        &ctx,
        vec![egui::Event::PointerMoved(moved)],
        Modifiers::NONE,
    );
    graph_frame(
        &mut s,
        &ctx,
        vec![pointer(moved, PointerButton::Middle, false)],
        Modifiers::NONE,
    );
    assert_ne!(s.graph_view.view, view);
    assert_eq!(s.draft, draft);
    assert_eq!(s.editor.project(), &project);
    assert!(s.graph_drag.is_none());
    assert!(!s.editor.can_undo());
}
#[test]
fn wheel_over_endpoint_zooms_about_pointer_and_release_does_not_leak_scroll() {
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let ctx = egui::Context::default();
    graph_frame(&mut s, &ctx, vec![], Modifiers::NONE);
    let (pos, _, outer) = graph_frame(&mut s, &ctx, vec![], Modifiers::NONE);
    let before = s.graph_view.view;
    let rect = super::graph_canvas::plot_rect(outer);
    let mapping = Plot::new(
        rect,
        [before.x.min, before.x.max],
        [before.y.min, before.y.max],
    )
    .unwrap();
    let anchor = mapping.unmap(pos);
    graph_frame(
        &mut s,
        &ctx,
        vec![egui::Event::PointerMoved(pos), wheel(80., Modifiers::CTRL)],
        Modifiers::CTRL,
    );
    let after = s.graph_view.view;
    assert!(after.x.span() < before.x.span());
    assert!((after.x.fraction(anchor.0) - before.x.fraction(anchor.0)).abs() < 1e-10);
    assert!((after.y.fraction(anchor.1) - before.y.fraction(anchor.1)).abs() < 1e-10);
    let timeline_id = egui::Id::unique(("section navigator", true, 2_u64));
    let timeline = ctx.read_response(timeline_id).unwrap().rect;
    for _ in 0..8 {
        graph_frame(
            &mut s,
            &ctx,
            vec![egui::Event::PointerMoved(timeline.center())],
            Modifiers::NONE,
        );
    }
    assert_eq!(ctx.read_response(timeline_id).unwrap().rect, timeline);
    assert_eq!(s.graph_view.view, after);
    assert!(!s.editor.can_undo());
}

fn workspace_frame(
    s: &mut Session,
    ctx: &egui::Context,
    size: [f32; 2],
    events: Vec<egui::Event>,
) -> Vec<(String, Rect)> {
    let scene = super::session::scene(s.editor.project(), &s.catalog, s.options).unwrap();
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            panels::workspace(s, ctx, &scene, &mut Default::default());
        },
    );
    fn texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.job.text.clone(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            egui::Shape::Vec(v) => {
                for shape in v {
                    texts(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut result = vec![];
    for shape in &output.shapes {
        texts(&shape.shape, &mut result);
    }
    output.textures_delta.clear();
    result
}
#[test]
fn scrolling_tools_and_timeline_cannot_move_graph_header_in_docked_or_floating_panes() {
    for size in [[1440., 960.], [960., 640.]] {
        for floating in [false, true] {
            let mut s = session();
            s.select(SectionId(2));
            s.select_channel(ChannelKey::Normal);
            if floating {
                s.workbench.layouts[0].float(super::workbench::Pane::Transition);
            }
            let project = s.editor.project().clone();
            let ctx = egui::Context::default();
            let mut text = vec![];
            for _ in 0..6 {
                text = workspace_frame(&mut s, &ctx, size, vec![]);
            }
            let timeline_id = egui::Id::unique(("section navigator", true, 2_u64));
            let timeline = ctx.read_response(timeline_id).unwrap().rect;
            let canvas_id = egui::Id::unique(("graph canvas", 2_u64, ChannelKey::Normal as u8));
            let header = text
                .iter()
                .find(|(s, _)| s.starts_with("Precision editor"))
                .unwrap()
                .1;
            assert!(
                header.bottom() <= size[1],
                "{size:?} {floating}: {header:?}"
            );
            let click = header.center();
            workspace_frame(
                &mut s,
                &ctx,
                size,
                vec![
                    egui::Event::PointerMoved(click),
                    pointer(click, PointerButton::Primary, true),
                ],
            );
            workspace_frame(
                &mut s,
                &ctx,
                size,
                vec![pointer(click, PointerButton::Primary, false)],
            );
            // Floating egui windows settle their content size after a disclosure opens.
            for _ in 0..8 {
                workspace_frame(&mut s, &ctx, size, vec![]);
            }
            let canvas = ctx.read_response(canvas_id).unwrap().rect;
            for pos in [header.center(), canvas.center(), timeline.center()] {
                for _ in 0..8 {
                    workspace_frame(
                        &mut s,
                        &ctx,
                        size,
                        vec![egui::Event::PointerMoved(pos), wheel(-40., Modifiers::NONE)],
                    );
                }
                assert_eq!(
                    ctx.read_response(timeline_id).unwrap().rect.top(),
                    timeline.top()
                );
                let actual = ctx.read_response(canvas_id).unwrap().rect;
                assert_eq!(
                    actual.min, canvas.min,
                    "scroll must not move the graph origin"
                );
                // egui's floating-window sizing settles at fractional point precision.
                assert!(
                    (actual.max - canvas.max).length() < 1.,
                    "{actual:?} {canvas:?}"
                );
            }
            assert_eq!(s.editor.project(), &project);
            assert!(!s.editor.can_undo());
        }
    }
}

#[test]
fn overpass_is_primary_for_every_text_style_with_mono_metrics_and_unicode_fallbacks() {
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let mut output = ctx.run_pass(Default::default(), |ctx| {
            super::theme::install(ctx);
        });
        output.textures_delta.clear();
    }
    ctx.fonts_mut(|fonts| {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let chain = &fonts.definitions().families[&family];
            assert_eq!(chain[0], "Overpass Mono");
            assert!(chain.len() > 1, "preserve bundled fallback chain");
            let font = egui::FontId::new(13., family);
            assert!(fonts.has_glyphs(&font, "± ° ² → ∞"));
            assert!((fonts.glyph_width(&font, 'i') - fonts.glyph_width(&font, 'W')).abs() < 1e-4);
        }
        assert!(!fonts.definitions().font_data.contains_key("Red Hat Mono"));
    });
    for font in ctx.style_of(egui::Theme::Dark).text_styles.values() {
        ctx.fonts(|fonts| {
            assert_eq!(
                fonts.definitions().families[&font.family][0],
                "Overpass Mono"
            )
        });
    }
}

fn precision_frame(
    s: &mut Session,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<(String, Rect)> {
    let mut output = ctx.run_pass(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1200., 500.))),
            events,
            ..Default::default()
        },
        |ctx| {
            ctx.root_ui(|root| {
                egui::CentralPanel::default()
                    .show(root, |ui| super::graph::numeric_transition(s, ui));
            });
        },
    );
    let result = output
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
    result
}
#[test]
fn precise_numeric_entry_and_split_remain_drafts_and_apply_as_single_undo_steps() {
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let original = s.editor.project().clone();
    let ctx = egui::Context::default();
    precision_frame(&mut s, &ctx, vec![]);
    let texts = precision_frame(&mut s, &ctx, vec![]);
    let label = texts.iter().find(|(text, _)| text == "Start").unwrap().1;
    let pos = egui::pos2(label.right() + 24., label.center().y);
    precision_frame(
        &mut s,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, PointerButton::Primary, true),
        ],
    );
    precision_frame(
        &mut s,
        &ctx,
        vec![pointer(pos, PointerButton::Primary, false)],
    );
    precision_frame(&mut s, &ctx, vec![egui::Event::Text("1.2345678901".into())]);
    let enter = egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    precision_frame(&mut s, &ctx, vec![enter]);
    assert!(
        (s.draft.channel_mut(ChannelKey::Normal).unwrap().transitions[0].start - 1.2345678901)
            .abs()
            < 1e-12
    );
    assert_eq!(s.editor.project(), &original);
    assert!(!s.editor.can_undo());
    s.apply_draft();
    let numeric_project = s.editor.project().clone();
    s.undo(false);
    assert_eq!(s.editor.project(), &original);
    assert!(!s.editor.can_undo());
    s.undo(true);
    assert_eq!(s.editor.project(), &numeric_project);
    let texts = precision_frame(&mut s, &ctx, vec![]);
    let pos = texts
        .iter()
        .find(|(text, _)| text == "Split into two Hermite regions")
        .unwrap()
        .1
        .center();
    precision_frame(
        &mut s,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            pointer(pos, PointerButton::Primary, true),
        ],
    );
    precision_frame(
        &mut s,
        &ctx,
        vec![pointer(pos, PointerButton::Primary, false)],
    );
    assert_eq!(s.editor.project(), &numeric_project);
    assert_eq!(
        s.draft
            .channel_mut(ChannelKey::Normal)
            .unwrap()
            .transitions
            .len(),
        3
    );
    assert_eq!(s.draft.extent, numeric_project.sections[1].extent);
    assert_eq!(s.draft.roll_rate, numeric_project.sections[1].roll_rate);
    s.apply_draft();
    let split_project = s.editor.project().clone();
    assert_ne!(split_project, numeric_project);
    s.undo(false);
    assert_eq!(s.editor.project(), &numeric_project);
    s.undo(true);
    assert_eq!(s.editor.project(), &split_project);
}
#[test]
fn hover_inspection_reports_evaluated_authored_values_in_the_visible_domain() {
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    s.graph_view
        .set(View::exact([10., 45., 0.5, 2.5], 60.).unwrap());
    let ctx = egui::Context::default();
    for _ in 0..6 {
        workspace_frame(&mut s, &ctx, [1440., 960.], vec![]);
    }
    let canvas = ctx
        .read_response(egui::Id::unique((
            "graph canvas",
            2_u64,
            ChannelKey::Normal as u8,
        )))
        .unwrap()
        .rect;
    let rect = super::graph_canvas::plot_rect(canvas);
    let pos = egui::pos2(rect.left() + rect.width() * 0.37, rect.center().y);
    let view = s.graph_view.view;
    let input = Plot::new(rect, [view.x.min, view.x.max], [view.y.min, view.y.max])
        .unwrap()
        .unmap(pos)
        .0;
    let value = s
        .draft
        .channel_mut(ChannelKey::Normal)
        .unwrap()
        .evaluate(input)
        .unwrap()
        .value;
    let text = workspace_frame(
        &mut s,
        &ctx,
        [1440., 960.],
        vec![egui::Event::PointerMoved(pos)],
    );
    assert!(
        text.iter()
            .any(|(t, _)| *t == format!("{input:.6} m   {value:.6} g"))
    );
    assert!(!s.draft_dirty());
    assert!(!s.editor.can_undo());
}

#[test]
fn endpoint_targets_keep_their_logical_size_at_extreme_zoom_without_editing_values() {
    let mut s = session();
    s.select(SectionId(2));
    s.select_channel(ChannelKey::Normal);
    let project = s.editor.project().clone();
    let ctx = egui::Context::default();
    let id = egui::Id::unique((
        "graph endpoint",
        2_u64,
        ChannelKey::Normal as u8,
        1_u64,
        true,
    ));
    for factor in [0.1, 10.] {
        for _ in 0..24 {
            let view = s.graph_view.view.zoom((30., 1.7), factor, 60.);
            s.graph_view.set(view);
            graph_frame(&mut s, &ctx, vec![], Modifiers::NONE);
            let response = ctx.read_response(id).unwrap();
            assert_eq!(response.rect.size(), egui::vec2(24., 24.));
        }
    }
    assert_eq!(s.editor.project(), &project);
    assert_eq!(s.draft, project.sections[1]);
    assert!(!s.editor.can_undo());
}

#[test]
fn short_zero_graph_and_large_offset_graph_show_distinct_exact_y_bounds() {
    let mut s = session();
    let ctx = egui::Context::default();
    for y in [[-0.05, 0.05], [1e10, 1e10 + 0.1]] {
        let view = View::exact([0., 12., y[0], y[1]], 12.).unwrap();
        s.graph_view.set(view);
        let mut text = vec![];
        for _ in 0..6 {
            text = workspace_frame(&mut s, &ctx, [960., 640.], vec![]);
        }
        let canvas = ctx
            .read_response(egui::Id::unique((
                "graph canvas",
                1_u64,
                ChannelKey::Roll as u8,
            )))
            .unwrap()
            .rect;
        let labels = [
            super::plot::number(y[0], view.y),
            super::plot::number(y[1], view.y),
        ];
        assert_ne!(labels[0], labels[1]);
        for label in labels {
            let bounds = text.iter().find(|(t, _)| *t == label).unwrap().1;
            assert!(bounds.min.x >= canvas.min.x && bounds.max.x <= canvas.max.x);
            assert!(bounds.min.y >= canvas.min.y && bounds.max.y <= canvas.max.y);
        }
        assert_eq!(s.graph_view.view, view);
        assert!(!s.draft_dirty());
    }
}
