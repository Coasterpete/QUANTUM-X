//! A graph press owns one authored target until release/cancel, independently
//! of its moving/clipped hit rectangle. Coordinates and scale are frozen at press.
use super::{plot::Plot, session::Session};
use egui::{Context, PointerButton, Pos2};
use qx_geometry::authoring::{ChannelKey, Section, SectionId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Endpoint { end: bool },
    Boundary,
    Control { index: usize },
    Plateau,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub device: Option<u64>,
    pub section: SectionId,
    pub channel: ChannelKey,
    pub transition: u64,
    pub kind: Kind,
}
impl Target {
    pub fn id(self) -> egui::Id {
        let id = match self.kind {
            Kind::Endpoint { end } => egui::Id::unique((
                "graph endpoint",
                self.section.0,
                self.channel as u8,
                self.transition,
                end,
            )),
            Kind::Boundary => egui::Id::unique((
                "boundary",
                self.section.0,
                self.channel as u8,
                self.transition,
            )),
            Kind::Control { index } => egui::Id::unique((
                "graph shape control",
                self.section.0,
                self.channel as u8,
                self.transition,
                index,
            )),
            Kind::Plateau => egui::Id::unique((
                "graph plateau",
                self.section.0,
                self.channel as u8,
                self.transition,
            )),
        };
        self.device.map_or(id, |device| id.with(("device", device)))
    }
    pub fn cursor(self) -> egui::CursorIcon {
        match self.kind {
            Kind::Endpoint { .. } | Kind::Control { .. } => egui::CursorIcon::Move,
            Kind::Plateau => egui::CursorIcon::ResizeVertical,
            Kind::Boundary => egui::CursorIcon::ResizeHorizontal,
        }
    }
}

#[derive(Clone)]
pub(super) struct Drag {
    pub before: Section,
    pub target: Target,
    pub status_height: f32,
    origin: Pos2,
    last_position: Pos2,
    displacement: [f64; 2],
    started: bool,
    value_per_point: f64,
    domain_per_point: f64,
    error: Option<String>,
}

pub(super) fn capture(
    session: &mut Session,
    mapping: &Plot,
    handle: &egui::Response,
    target: Target,
    index: usize,
) {
    // Native input can move outside the handle in the same frame as the press,
    // before egui assigns widget ownership. A release may arrive one frame later
    // or in the same batch. Hit-test the press, not the final pointer position.
    // Respect the clipped handle and its layer so popups retain input ownership.
    let pressed_origin = handle
        .ctx
        .input(|i| {
            i.pointer
                .button_pressed(PointerButton::Primary)
                .then(|| {
                    i.raw.events.iter().find_map(|e| {
                        if let egui::Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed: true,
                            ..
                        } = e
                        {
                            Some(*pos)
                        } else {
                            None
                        }
                    })
                })
                .flatten()
        })
        .filter(|p| {
            handle.interact_rect.contains(*p) && handle.ctx.layer_id_at(*p) == Some(handle.layer_id)
        });
    if session.graph_drag.is_none()
        && (handle.is_pointer_button_down_on() || pressed_origin.is_some())
        && handle
            .ctx
            .input(|i| i.pointer.button_pressed(PointerButton::Primary))
        && let Some(origin) =
            pressed_origin.or_else(|| handle.ctx.input(|i| i.pointer.press_origin()))
    {
        session.graph_feedback = None;
        session.graph_drag = Some(Drag {
            before: session.graph_section(),
            target,
            status_height: super::commands::status_height(session),
            origin,
            last_position: origin,
            displacement: [0.; 2],
            started: false,
            value_per_point: mapping.y.span() / mapping.rect.height() as f64,
            domain_per_point: mapping.x.span() / mapping.rect.width() as f64,
            error: None,
        });
        session.set_graph_transition(index);
        session.graph_selection = Some(target);
        handle
            .ctx
            .memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(handle.id)));
        if pressed_origin.is_some() {
            update(session, &handle.ctx);
        }
    }
}

pub(super) fn cancel(session: &mut Session, reason: &str) {
    if let Some(drag) = session.graph_drag.take() {
        let changed = session.graph_section() != drag.before;
        let _ = session.set_graph_section_in(drag.target.device, drag.before);
        if changed && session.graph_device().is_none() {
            session.refresh_view();
        }
        session.graph_feedback = Some(reason.into());
        session.status = reason.into();
    }
}

/// Run before the workspace and also from the standalone graph. No handle
/// response, hover, clip rectangle or continued pane visibility is needed.
pub(super) fn update(session: &mut Session, ctx: &Context) {
    let Some(mut drag) = session.graph_drag.take() else {
        return;
    };
    let (pos, down, released, decided, focused) = ctx.input(|i| {
        (
            i.pointer.interact_pos(),
            i.pointer.button_down(PointerButton::Primary),
            i.pointer.button_released(PointerButton::Primary),
            i.pointer.is_decidedly_dragging(),
            i.focused,
        )
    });
    if !focused
        || session.graph_id() != drag.target.section
        || session.graph_key() != drag.target.channel
        || session.graph_device() != drag.target.device
    {
        session.graph_drag = Some(drag);
        cancel(
            session,
            "Graph gesture canceled; preceding draft restored (focus or authoring context changed).",
        );
        return;
    }
    if let Some(pos) = pos {
        let delta = pos - drag.origin;
        drag.started |= decided || delta.length() > ctx.options(|o| o.input_options.max_click_dist);
        if drag.started {
            // Rolling deltas allow live precision mode without jumping when
            // Shift changes. The mapping remains the one captured at press.
            let gain = if ctx.input(|i| i.modifiers.shift) {
                0.1
            } else {
                1.
            };
            let movement = pos - drag.last_position;
            drag.displacement[0] += movement.x as f64 * gain;
            drag.displacement[1] += movement.y as f64 * gain;
            drag.last_position = pos;
            let delta = drag.displacement;
            let fine = ctx.input(|i| i.modifiers.shift);
            let bypass = ctx.input(|i| i.modifiers.alt);
            let mut next = drag.before.clone();
            let channel = next
                .channel_mut(drag.target.channel)
                .expect("captured channel");
            let index = channel
                .transitions
                .iter()
                .position(|t| t.id == drag.target.transition)
                .expect("captured transition");
            let candidate = match drag.target.kind {
                Kind::Boundary => {
                    let start: f64 = channel.transitions[..index].iter().map(|t| t.length).sum();
                    let station = start
                        + channel.transitions[index].length
                        + delta[0] * drag.domain_per_point;
                    let length = if delta[0] == 0. {
                        channel.transitions[index].length
                    } else {
                        snapped(session, &drag, index, station, fine, bypass, true) - start
                    };
                    super::graph_canvas::boundary(&drag.before, drag.target.channel, index, length)
                }
                Kind::Endpoint { end } => {
                    let t = &channel.transitions[index];
                    let value = if end { t.end } else { t.start } - delta[1] * drag.value_per_point;
                    let value = if delta[1] == 0. {
                        value
                    } else {
                        snapped(session, &drag, index, value, fine, bypass, false)
                    };
                    let movable = if end {
                        index + 1 < channel.transitions.len()
                    } else {
                        index > 0
                    };
                    let station: f64 = channel.transitions[..index + usize::from(end)]
                        .iter()
                        .map(|t| t.length)
                        .sum();
                    let station = if movable && delta[0] != 0. {
                        snapped(
                            session,
                            &drag,
                            index,
                            station + delta[0] * drag.domain_per_point,
                            fine,
                            bypass,
                            true,
                        )
                    } else {
                        station
                    };
                    qx_document::region_edit::point(
                        &drag.before,
                        drag.target.channel,
                        index,
                        end,
                        station,
                        value,
                    )
                    .map_err(|e| e.to_string())
                }
                Kind::Control { index: control } => {
                    let converted = channel.transitions[index]
                        .to_nurbs()
                        .map_err(|e| e.to_string());
                    let mut t = match converted {
                        Ok(t) => t,
                        Err(error) => {
                            drag.error = Some(error);
                            session.graph_drag = Some(drag);
                            return;
                        }
                    };
                    let qx_geometry::transition::Function::Nurbs { profile } = &mut t.function
                    else {
                        unreachable!("captured NURBS control")
                    };
                    let candidate = profile.refine_for_edit().map_err(|e| e.to_string());
                    candidate.and_then(|()| {
                        let start: f64 =
                            channel.transitions[..index].iter().map(|t| t.length).sum();
                        let station = start
                            + profile.knots[control + 1..=control + profile.degree]
                                .iter()
                                .sum::<f64>()
                                / profile.degree as f64
                                * t.length;
                        qx_document::region_edit::shape_point(
                            &drag.before,
                            drag.target.channel,
                            index,
                            control,
                            if delta[0] == 0. {
                                station
                            } else {
                                snapped(
                                    session,
                                    &drag,
                                    index,
                                    station + delta[0] * drag.domain_per_point,
                                    fine,
                                    bypass,
                                    true,
                                )
                            },
                            if delta[1] == 0. {
                                profile.values[control]
                            } else {
                                snapped(
                                    session,
                                    &drag,
                                    index,
                                    profile.values[control] - delta[1] * drag.value_per_point,
                                    fine,
                                    bypass,
                                    false,
                                )
                            },
                        )
                        .map_err(|e| e.to_string())
                    })
                }
                Kind::Plateau => qx_document::region_edit::plateau_level(
                    &drag.before,
                    drag.target.channel,
                    index,
                    if delta[1] == 0. {
                        channel.transitions[index].start
                    } else {
                        snapped(
                            session,
                            &drag,
                            index,
                            channel.transitions[index].start - delta[1] * drag.value_per_point,
                            fine,
                            bypass,
                            false,
                        )
                    },
                )
                .map_err(|e| e.to_string()),
            };
            let candidate = candidate.and_then(|next| {
                if next == session.graph_section() {
                    Ok(())
                } else {
                    session.set_graph_section(next)
                }
            });
            match candidate {
                Ok(()) => {
                    drag.error = None;
                    session.graph_feedback = None;
                }
                Err(error) => {
                    session.graph_feedback = Some(format!(
                        "Invalid graph drag: {error} Last valid draft retained; move back into range or Esc to cancel."
                    ));
                    drag.error = Some(error);
                }
            }
        }
    }
    // A release may carry the final movement in the same batch. Evaluate above
    // before ending the transaction. PointerGone alone does not drop capture.
    if released {
        finish(session, drag);
    } else if !down {
        session.graph_drag = Some(drag);
        cancel(
            session,
            "Graph gesture canceled; primary-button capture lost. Preceding draft restored.",
        );
    } else {
        ctx.set_cursor_icon(if drag.error.is_some() {
            egui::CursorIcon::NotAllowed
        } else {
            drag.target.cursor()
        });
        session.graph_drag = Some(drag);
    }
}

fn snapped(
    session: &Session,
    drag: &Drag,
    index: usize,
    raw: f64,
    fine: bool,
    bypass: bool,
    horizontal: bool,
) -> f64 {
    let settings = session.workbench.snap;
    let g = settings.graph;
    let boundary = horizontal;
    let mut before = drag.before.clone();
    let c = before.channel_mut(drag.target.channel).unwrap();
    let unit = if drag.target.device.is_some() {
        qx_geometry::transition::Unit::MetresPerSecondSquared
    } else {
        c.unit
    };
    let axis = if horizontal {
        match drag.target.kind {
            Kind::Boundary => super::snap::GraphAxis::Boundary(before.domain),
            Kind::Control { .. } => super::snap::GraphAxis::HandleStation(before.domain),
            _ => super::snap::GraphAxis::PointStation(before.domain),
        }
    } else {
        match drag.target.kind {
            Kind::Boundary => unreachable!(),
            Kind::Endpoint { .. } => super::snap::GraphAxis::Value(unit),
            Kind::Control { .. } => super::snap::GraphAxis::Handle(unit),
            Kind::Plateau => super::snap::GraphAxis::Value(unit),
        }
    };
    let Some(step) = settings.graph_step(axis, fine, bypass) else {
        return raw;
    };
    let mut keys = Vec::new();
    if g.nearby_keys {
        if boundary {
            for key in qx_document::editing::channel_keys(&before) {
                let channel = before.channel_mut(key).unwrap();
                let mut station = 0.;
                for (i, t) in channel.transitions.iter().enumerate() {
                    station += t.length;
                    let edited_boundary = match drag.target.kind {
                        Kind::Endpoint { end: false } => index.checked_sub(1),
                        _ => Some(index),
                    };
                    if key != drag.target.channel || Some(i) != edited_boundary {
                        keys.push(station);
                    }
                    if g.handles
                        && !qx_document::region_edit::is_plateau(t)
                        && let Ok(converted) = t.to_nurbs()
                        && let qx_geometry::transition::Function::Nurbs { mut profile } =
                            converted.function
                        && profile.refine_for_edit().is_ok()
                    {
                        for control in 3..profile.values.len() - 3 {
                            if key != drag.target.channel
                                || i != index
                                || drag.target.kind != (Kind::Control { index: control })
                            {
                                keys.push(
                                    station - t.length
                                        + profile.knots[control + 1..=control + profile.degree]
                                            .iter()
                                            .sum::<f64>()
                                            / profile.degree as f64
                                            * t.length,
                                );
                            }
                        }
                    }
                }
            }
        } else {
            let c = before.channel_mut(drag.target.channel).unwrap();
            for (i, t) in c.transitions.iter().enumerate() {
                for end in [false, true] {
                    if (i != index || drag.target.kind != (Kind::Endpoint { end }))
                        && !(i == index && drag.target.kind == Kind::Plateau)
                    {
                        keys.push(if end { t.end } else { t.start });
                    }
                }
                if g.handles
                    && !qx_document::region_edit::is_plateau(t)
                    && let Ok(converted) = t.to_nurbs()
                    && let qx_geometry::transition::Function::Nurbs { profile } =
                        &converted.function
                {
                    let mut shape = profile.clone();
                    if shape.refine_for_edit().is_ok() {
                        for (control, &v) in shape
                            .values
                            .iter()
                            .enumerate()
                            .take(shape.values.len() - 3)
                            .skip(3)
                        {
                            if i != index || drag.target.kind != (Kind::Control { index: control })
                            {
                                keys.push(v);
                            }
                        }
                    }
                }
            }
        }
    }
    let scale = if boundary {
        drag.domain_per_point
    } else {
        drag.value_per_point
    };
    super::snap::scalar(raw, Some(step), &keys, g.key_radius_points * scale)
}

fn finish(session: &mut Session, drag: Drag) {
    let error = if let Some(error) = drag.error {
        Some(error)
    } else if session.graph_section() == drag.before {
        return;
    } else if session.graph_device().is_some() {
        None
    } else {
        // Existing drafts stay staged. Validate the complete live candidate at
        // the transaction boundary, including strict downstream connections.
        session
            .preview_project()
            .generate(&session.catalog, session.options)
            .err()
            .map(|e| e.to_string())
    };
    if let Some(error) = error {
        let changed = session.graph_section() != drag.before;
        session.accept_graph_section(drag.before);
        if changed && session.graph_device().is_none() {
            session.refresh_view();
        }
        let message =
            format!("Graph drag rejected: {error} Preceding draft and document/history retained.");
        session.graph_feedback = Some(message.clone());
        session.status = message;
    } else {
        session.graph_feedback = Some("Graph gesture staged. Apply or Cancel to finish.".into());
    }
}
