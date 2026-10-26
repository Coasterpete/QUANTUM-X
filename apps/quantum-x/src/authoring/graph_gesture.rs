//! A graph press owns one authored target until release/cancel, independently
//! of its moving/clipped hit rectangle. Coordinates and scale are frozen at press.
use super::{plot::Plot, session::Session};
use egui::{Context, PointerButton, Pos2};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionId},
    transition::Function,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Endpoint { end: bool },
    Boundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub section: SectionId,
    pub channel: ChannelKey,
    pub transition: u64,
    pub kind: Kind,
}
impl Target {
    pub fn id(self) -> egui::Id {
        match self.kind {
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
        }
    }
    pub fn cursor(self) -> egui::CursorIcon {
        match self.kind {
            Kind::Endpoint { .. } => egui::CursorIcon::ResizeVertical,
            Kind::Boundary => egui::CursorIcon::ResizeHorizontal,
        }
    }
}

#[derive(Clone)]
pub(super) struct Drag {
    pub before: Section,
    pub target: Target,
    pub status_height: f32,
    auto_apply: bool,
    origin: Pos2,
    last_position: Pos2,
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
    if session.graph_drag.is_none()
        && handle.is_pointer_button_down_on()
        && handle
            .ctx
            .input(|i| i.pointer.button_pressed(PointerButton::Primary))
        && let Some(origin) = handle.ctx.input(|i| i.pointer.press_origin())
    {
        session.graph_feedback = None;
        session.graph_drag = Some(Drag {
            before: session.draft.clone(),
            target,
            status_height: super::commands::status_height(session),
            auto_apply: !session.has_drafts(),
            origin,
            last_position: origin,
            started: false,
            value_per_point: mapping.y.span() / mapping.rect.height() as f64,
            domain_per_point: mapping.x.span() / mapping.rect.width() as f64,
            error: None,
        });
        session.transition = index;
        handle
            .ctx
            .memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(handle.id)));
    }
}

pub(super) fn cancel(session: &mut Session, reason: &str) {
    if let Some(drag) = session.graph_drag.take() {
        let changed = session.draft != drag.before;
        session.draft = drag.before;
        if changed {
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
    if !focused || session.selected != drag.target.section || session.channel != drag.target.channel
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
        if drag.started && pos != drag.last_position {
            drag.last_position = pos;
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
                    let length =
                        channel.transitions[index].length + delta.x as f64 * drag.domain_per_point;
                    super::graph_canvas::boundary(&drag.before, drag.target.channel, index, length)
                }
                Kind::Endpoint { end } => {
                    let t = &mut channel.transitions[index];
                    let value =
                        if end { t.end } else { t.start } - delta.y as f64 * drag.value_per_point;
                    if end {
                        t.end = value;
                    } else {
                        t.start = value;
                    }
                    if matches!(t.function, Function::Constant {}) {
                        t.start = value;
                        t.end = value;
                    }
                    channel.validate().map(|()| next).map_err(|e| e.to_string())
                }
            };
            match candidate {
                Ok(next) => {
                    session.draft = next;
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

fn finish(session: &mut Session, drag: Drag) {
    let error = if let Some(error) = drag.error {
        Some(error)
    } else if session.draft == drag.before {
        return;
    } else if drag.auto_apply {
        if session.apply_draft() {
            return;
        }
        Some(session.status.clone())
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
        let changed = session.draft != drag.before;
        session.draft = drag.before;
        if changed {
            session.refresh_view();
        }
        let message =
            format!("Graph drag rejected: {error} Preceding draft and document/history retained.");
        session.graph_feedback = Some(message.clone());
        session.status = message;
    } else {
        session.graph_feedback =
            Some("Graph gesture staged with the existing draft. Apply or Cancel to finish.".into());
    }
}
