//! Pointer authoring is a reversible draft gesture, committed at most once on release.
use super::{
    graph_gesture::{self, Kind, Target},
    plot::Plot,
    session::Session,
};
use egui::{Color32, Pos2, Sense, Stroke};
use qx_geometry::{
    authoring::{ChannelKey, Section},
    transition::Function,
};

pub(super) fn channel_style(key: ChannelKey) -> (&'static str, Color32) {
    match key {
        ChannelKey::Normal => ("Normal force", Color32::from_rgb(130, 202, 232)),
        ChannelKey::Lateral => ("Lateral force", Color32::from_rgb(231, 185, 107)),
        ChannelKey::Roll => ("Roll rate", Color32::from_rgb(189, 166, 226)),
        ChannelKey::Pitch => ("Pitch rate", Color32::from_rgb(151, 209, 157)),
        ChannelKey::Yaw => ("Yaw rate", Color32::from_rgb(232, 155, 151)),
    }
}
pub(super) fn fit(session: &mut Session) {
    super::graph_view::sync(session);
    if let Some(view) =
        super::graph_view::View::fit(session.draft.channel_mut(session.channel).unwrap())
    {
        session.graph_view.set(view);
    }
}
pub(super) fn boundary(
    section: &Section,
    key: ChannelKey,
    index: usize,
    local_end: f64,
) -> Result<Section, String> {
    let mut next = section.clone();
    let channel = next.channel_mut(key).map_err(|e| e.to_string())?;
    if index + 1 >= channel.transitions.len() {
        return Err(
            "Only internal boundaries can move; use section extent to resize the whole section."
                .into(),
        );
    }
    let total = channel.transitions[index].length + channel.transitions[index + 1].length;
    if !local_end.is_finite() || local_end < 1e-8 || total - local_end < 1e-8 {
        return Err(
            "Boundary must leave both adjacent transitions at least 1e-8 domain units.".into(),
        );
    }
    channel.transitions[index].length = local_end;
    channel.transitions[index + 1].length = total - local_end;
    channel.validate().map_err(|e| e.to_string())?;
    Ok(next)
}
/// egui smooths wheel notches across frames. A notch over the graph must not later
/// scroll the timeline/tools merely because the pointer crossed into them.
pub(super) fn route_scroll_tail(session: &Session, ui: &egui::Ui) {
    let id = egui::Id::unique("graph wheel ownership");
    let fresh = ui.input(|i| {
        i.raw
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::MouseWheel { .. }))
    });
    if fresh {
        let over_graph = ui
            .ctx()
            .read_response(egui::Id::unique((
                "graph canvas",
                session.selected.0,
                session.channel as u8,
            )))
            .is_some_and(|r| {
                ui.ctx()
                    .pointer_hover_pos()
                    .is_some_and(|p| r.rect.contains(p) && ui.rect_contains_pointer(r.rect))
            });
        ui.ctx().data_mut(|d| d.insert_temp(id, over_graph));
    }
    if ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
}
pub(super) fn plot_rect(outer: egui::Rect) -> egui::Rect {
    // Reserve scientific labels without moving the plot under a pointer as their
    // precision changes during zoom. Only resizing the pane can change this gutter.
    let gutter = 148_f32.min(outer.width() * 0.4);
    egui::Rect::from_min_max(
        outer.min + egui::vec2(gutter, 28.),
        outer.max - egui::vec2(32., 18.),
    )
}
pub(super) fn show(session: &mut Session, ui: &mut egui::Ui) {
    super::graph_view::sync(session);
    let extent = session.draft.channel_mut(session.channel).unwrap().length();
    let height = ui.available_height().clamp(66., 600.);
    // Unwrapped precision labels or toolbar minimum widths must never push the
    // mathematical right endpoint past the pane's actual interactive clip.
    let width = ui
        .available_width()
        .min(ui.clip_rect().right() - ui.cursor().left())
        .max(2.);
    let (_, outer) = ui.allocate_space(egui::vec2(width, height));
    let response = ui.interact(
        outer,
        egui::Id::unique(("graph canvas", session.selected.0, session.channel as u8)),
        Sense::click_and_drag(),
    );
    let super::graph_view::View { x, y } = session.graph_view.view;
    let rect = plot_rect(outer);
    let Some(mapping) = Plot::new(rect, [x.min, x.max], [y.min, y.max]) else {
        return;
    };
    let x_unit = match session.draft.domain {
        qx_geometry::authoring::Domain::Time => "s",
        qx_geometry::authoring::Domain::Distance => "m",
        _ => "normalized",
    };
    let channel = session.draft.channel_mut(session.channel).unwrap().clone();
    let unit = match channel.unit {
        qx_geometry::transition::Unit::G => "g",
        qx_geometry::transition::Unit::RadiansPerSecond => "rad/s",
        qx_geometry::transition::Unit::RadiansPerMetre => "rad/m",
        _ => "rad/u",
    };
    mapping.paint_grid(ui, outer, x_unit, unit);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let (_, color) = channel_style(session.channel);
    if let Some((id, input)) = session.inspect_input
        && id == session.selected
        && (x.min..=x.max).contains(&input)
    {
        let px = mapping.map(input, 0.).x;
        painter.line_segment(
            [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
            Stroke::new(1., Color32::from_rgb(240, 195, 100)),
        );
    }
    let mut cursor = 0.;
    let mut feedback = None;
    for (index, t) in channel.transitions.iter().enumerate() {
        let active = session.transition == index;
        let left = mapping.map(cursor, y.min).x.max(rect.left());
        let right = mapping.map(cursor + t.length, y.max).x.min(rect.right());
        if active && right > left {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(left, rect.top()),
                    egui::pos2(right, rect.bottom()),
                ),
                0.,
                color.gamma_multiply(0.09),
            );
        }
        let boundary_x = mapping.map(cursor, 0.).x;
        painter.line_segment(
            [
                egui::pos2(boundary_x, rect.top()),
                egui::pos2(boundary_x, rect.bottom()),
            ],
            Stroke::new(1., Color32::from_gray(78)),
        );
        let begin_x = (x.min - cursor).clamp(0., t.length);
        let end_x = (x.max - cursor).clamp(0., t.length);
        let points: Vec<Pos2> = (0..=128)
            .filter_map(|i| {
                if end_x <= begin_x {
                    return None;
                }
                let input = begin_x + (end_x - begin_x) * i as f64 / 128.;
                t.evaluate(input)
                    .ok()
                    .map(|e| mapping.map(cursor + input, e.value))
            })
            .collect();
        if points.len() > 1 {
            painter.add(egui::Shape::line(
                points,
                Stroke::new(
                    if active { 2.5 } else { 1.5 },
                    if active {
                        color
                    } else {
                        color.gamma_multiply(0.65)
                    },
                ),
            ));
        }
        // The upper ruler keeps timing targets separate from value handles at graph peaks.
        if index + 1 < channel.transitions.len() {
            let pos = egui::pos2(mapping.map(cursor + t.length, 0.).x, rect.top() - 14.);
            let hit = if (rect.left()..=rect.right()).contains(&pos.x) {
                egui::Rect::from_center_size(pos, egui::vec2(24., 24.)).intersect(outer)
            } else {
                egui::Rect::NOTHING
            };
            let target = Target {
                section: session.selected,
                channel: session.channel,
                transition: t.id,
                kind: Kind::Boundary,
            };
            let handle = ui
                .interact(hit, target.id(), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            graph_gesture::capture(session, &mapping, &handle, target, index);
            let dragging = session
                .graph_drag
                .as_ref()
                .is_some_and(|d| d.target == target);
            let invalid = dragging && session.graph_feedback.is_some();
            ui.painter()
                .with_clip_rect(outer.intersect(ui.clip_rect()))
                .add(egui::Shape::convex_polygon(
                    vec![
                        pos + egui::vec2(0., -5.),
                        pos + egui::vec2(5., 0.),
                        pos + egui::vec2(0., 5.),
                        pos + egui::vec2(-5., 0.),
                    ],
                    if invalid {
                        Color32::from_rgb(240, 195, 100)
                    } else if handle.hovered() || dragging {
                        Color32::WHITE
                    } else {
                        color
                    },
                    Stroke::new(if dragging { 2. } else { 1. }, Color32::WHITE),
                ));
            if handle.hovered() || dragging {
                feedback = Some(format!(
                    "{}Boundary {}: {:.8} {x_unit} · drag horizontally · Esc cancels",
                    if dragging { "Dragging " } else { "" },
                    index + 1,
                    cursor + t.length
                ));
            }
            if handle.clicked() {
                session.transition = index;
            }
            handle.on_hover_text(format!("Boundary {} · {:.8} {x_unit}. Drag horizontally to transfer duration between these two transitions; values and other channels stay unchanged. Esc cancels.", index + 1, cursor + t.length));
        }
        for end in [false, true] {
            let value = if end { t.end } else { t.start };
            let curve_pos = mapping.map(cursor + if end { t.length } else { 0. }, value);
            // Separate the two independently editable values at an internal join.
            let offset = if end && index + 1 < channel.transitions.len() {
                -14.
            } else if !end && index > 0 {
                14.
            } else {
                0.
            };
            let target = Target {
                section: session.selected,
                channel: session.channel,
                transition: t.id,
                kind: Kind::Endpoint { end },
            };
            let dragging = session
                .graph_drag
                .as_ref()
                .is_some_and(|d| d.target == target);
            // Keep the complete square/target inside the plot even at its outer
            // endpoints. A leader records the exact mathematical endpoint.
            let inset = 12_f32.min(rect.width().min(rect.height()) * 0.25);
            let pos = (curve_pos + egui::vec2(offset, 0.)).clamp(
                rect.min + egui::Vec2::splat(inset),
                rect.max - egui::Vec2::splat(inset),
            );
            if pos != curve_pos {
                painter.line_segment([curve_pos, pos], Stroke::new(1., color));
            }
            let visible = rect.expand(0.1).contains(curve_pos) || dragging;
            let hit = if visible {
                egui::Rect::from_center_size(pos, egui::vec2(24., 24.)).intersect(outer)
            } else {
                egui::Rect::NOTHING
            };
            let handle = ui
                .interact(hit, target.id(), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::ResizeVertical);
            graph_gesture::capture(session, &mapping, &handle, target, index);
            let dragging = session
                .graph_drag
                .as_ref()
                .is_some_and(|d| d.target == target);
            if visible {
                let size = if handle.hovered() || dragging {
                    12.
                } else {
                    10.
                };
                painter.rect(
                    egui::Rect::from_center_size(pos, egui::Vec2::splat(size)),
                    1.,
                    if dragging && session.graph_feedback.is_some() {
                        Color32::from_rgb(240, 195, 100)
                    } else if handle.hovered() || dragging {
                        Color32::WHITE
                    } else {
                        color
                    },
                    Stroke::new(if dragging { 2. } else { 1. }, Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
                if dragging {
                    painter.circle_stroke(pos, 10., Stroke::new(1.5, Color32::WHITE));
                }
            }
            if handle.hovered() || dragging {
                feedback = Some(format!(
                    "{}Transition {} {}: {value:.8} {unit} · drag vertically · Esc cancels",
                    if dragging { "Dragging " } else { "" },
                    index + 1,
                    if end { "end" } else { "start" }
                ));
            }
            if handle.clicked() {
                session.transition = index;
            }
            handle.on_hover_text(format!(
                "Transition {} {} · {:.8} {unit}. Drag vertically; {}. Esc cancels.",
                index + 1,
                if end { "end" } else { "start" },
                value,
                if matches!(t.function, Function::Constant {}) {
                    "constant changes both values"
                } else {
                    "changes only this endpoint"
                }
            ));
        }
        if session.graph_drag.is_none()
            && response.clicked()
            && let Some(pos) = response
                .interact_pointer_pos()
                .filter(|p| rect.contains(*p))
        {
            let input =
                x.min + (pos.x - rect.left()) as f64 / rect.width() as f64 * (x.max - x.min);
            if (cursor..=cursor + t.length).contains(&input) {
                session.transition = index;
            }
        }
        cursor += t.length;
    }
    // The reserved status line shows this on the next repaint. An overlay would
    // obscure handles in short graphs; allocated text would resize floating UI.
    session.graph_readout = feedback;
    // Use geometric hover: child handles otherwise hide the canvas hover response.
    if let Some(pos) = ui.ctx().pointer_hover_pos().filter(|p| {
        rect.contains(*p) && ui.clip_rect().contains(*p) && ui.rect_contains_pointer(outer)
    }) {
        let (input, anchor_y) = mapping.unmap(pos);
        if let Ok(e) = channel.evaluate(input) {
            session.inspect_input = Some((session.selected, input));
            let point = mapping.map(input, e.value);
            painter.line_segment(
                [
                    egui::pos2(pos.x, rect.top()),
                    egui::pos2(pos.x, rect.bottom()),
                ],
                Stroke::new(1., Color32::from_gray(100)),
            );
            painter.circle_filled(point, 4., Color32::WHITE);
            let text = format!("{input:.6} {x_unit}   {:.6} {unit}", e.value);
            painter.text(
                rect.right_top() + egui::vec2(-8., 22.),
                egui::Align2::RIGHT_TOP,
                text,
                egui::FontId::monospace(12.),
                Color32::WHITE,
            );
        }
        if session.graph_drag.is_none() {
            let scroll = ui.input_mut(|i| {
                let delta: f32 = i
                    .raw
                    .events
                    .iter()
                    .filter_map(|event| {
                        if let egui::Event::MouseWheel {
                            delta,
                            unit,
                            modifiers,
                            phase: egui::TouchPhase::Move,
                            ..
                        } = event
                            && modifiers.ctrl
                            && !modifiers.alt
                            && !modifiers.shift
                        {
                            Some(
                                delta.y
                                    * match unit {
                                        egui::MouseWheelUnit::Point => 1.,
                                        egui::MouseWheelUnit::Line => 24.,
                                        egui::MouseWheelUnit::Page => rect.height(),
                                    },
                            )
                        } else {
                            None
                        }
                    })
                    .sum();
                i.smooth_scroll_delta = egui::Vec2::ZERO;
                delta
            });
            if scroll != 0. {
                let factor = (-scroll as f64 * 0.005).exp().clamp(0.1, 10.);
                session.graph_view.set(session.graph_view.view.zoom(
                    (input, anchor_y),
                    factor,
                    extent,
                ));
            }
            if !ui.ctx().egui_wants_keyboard_input()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F))
            {
                fit(session);
            }
        }
    }
    // Middle-drag works over handles too, but can never begin an authored edit.
    let pan_id = response.id.with("pan");
    if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle))
        && ui
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|p| rect.contains(p) && ui.rect_contains_pointer(outer))
    {
        ui.ctx().data_mut(|d| d.insert_temp(pan_id, true));
    }
    if ui
        .ctx()
        .data(|d| d.get_temp::<bool>(pan_id))
        .unwrap_or(false)
        && ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle))
        && !ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Middle))
        && session.graph_drag.is_none()
    {
        let delta = ui.input(|i| i.pointer.delta());
        session.graph_view.set(session.graph_view.view.pan(
            (
                -delta.x as f64 / rect.width() as f64 * x.span(),
                delta.y as f64 / rect.height() as f64 * y.span(),
            ),
            extent,
        ));
    }
    if !ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
        ui.ctx().data_mut(|d| d.remove::<bool>(pan_id));
    }
    if let Some(drag) = &session.graph_drag {
        ui.ctx()
            .set_cursor_icon(if session.graph_feedback.is_some() {
                egui::CursorIcon::NotAllowed
            } else {
                drag.target.cursor()
            });
    }
}
