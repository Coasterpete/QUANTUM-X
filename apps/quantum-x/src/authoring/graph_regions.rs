//! Direct region commands and local NURBS controls on the existing graph.
use super::{
    graph_gesture::{self, Kind, Target},
    plot::Plot,
    session::Session,
};
use egui::{Color32, Sense, Stroke};
use qx_geometry::transition::{Function, Transition};

pub(super) fn split(session: &mut Session, station: f64) {
    match qx_document::region_edit::split_at(&session.draft, session.channel, station) {
        Ok((section, right)) => {
            session.draft = section;
            session.transition = right;
            session.status =
                "Region split exactly; right region selected. Apply or Cancel the draft.".into();
        }
        Err(error) => session.status = error.to_string(),
    }
}

pub(super) fn context_menu(
    session: &mut Session,
    ui: &egui::Ui,
    response: &egui::Response,
    mapping: &Plot,
    unit: &str,
) {
    let id = response.id.with("region menu station");
    if response.secondary_clicked()
        && session.graph_drag.is_none()
        && let Some(pos) = response
            .interact_pointer_pos()
            .filter(|p| mapping.rect.contains(*p))
    {
        let station = mapping.unmap(pos).0;
        let channel = session.draft.channel_mut(session.channel).unwrap();
        let mut start = 0.;
        for (index, t) in channel.transitions.iter().enumerate() {
            if (start..=start + t.length).contains(&station) {
                session.transition = index;
                ui.ctx().data_mut(|d| d.insert_temp(id, (t.id, station)));
                break;
            }
            start += t.length;
        }
    }
    response.context_menu(|ui| {
        let Some((region, station)) = ui.ctx().data(|d| d.get_temp::<(u64, f64)>(id)) else {
            return;
        };
        let channel = session.draft.channel_mut(session.channel).unwrap().clone();
        let Some(index) = channel.transitions.iter().position(|t| t.id == region) else {
            return;
        };
        let start: f64 = channel.transitions[..index].iter().map(|t| t.length).sum();
        let t = &channel.transitions[index];
        ui.label(format!(
            "Region {} · {start:.6}–{:.6} {unit}",
            index + 1,
            start + t.length
        ));
        ui.label(format!("Station {station:.8} {unit}"));
        if ui
            .add_enabled(
                station - start >= 1e-8 && start + t.length - station >= 1e-8,
                egui::Button::new("Split here (preserve shape)"),
            )
            .clicked()
        {
            split(session, station);
            ui.close();
        }
        if ui
            .add_enabled(
                index + 1 < channel.transitions.len(),
                egui::Button::new("Merge with next (preserve shape)"),
            )
            .clicked()
        {
            match qx_document::region_edit::merge(&session.draft, session.channel, index) {
                Ok(next) => {
                    session.draft = next;
                    session.transition = index;
                }
                Err(e) => session.status = e.to_string(),
            }
            ui.close();
        }
        if ui
            .add_enabled(
                channel.transitions.len() > 1,
                egui::Button::new("Remove and extend neighbor (reshape)"),
            )
            .clicked()
        {
            match qx_document::editing::remove_transition(&session.draft, session.channel, region) {
                Ok(next) => {
                    session.draft = next;
                    session.transition = index.saturating_sub(1);
                }
                Err(e) => session.status = e.to_string(),
            }
            ui.close();
        }
    });
}

pub(super) fn controls(
    session: &mut Session,
    ui: &egui::Ui,
    mapping: &Plot,
    t: &Transition,
    index: usize,
    start: f64,
    color: Color32,
) -> Option<String> {
    let Function::Nurbs { profile } = &t.function else {
        return None;
    };
    let mut shape = profile.clone();
    if shape.refine_for_edit().is_err() {
        return None;
    }
    let painter = ui
        .painter()
        .with_clip_rect(mapping.rect.intersect(ui.clip_rect()));
    // Greville coordinates locate scalar controls in the station domain. These
    // are control-net ordinates, not interpolated points on the profile.
    let points: Vec<_> = shape
        .values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let u = shape.knots[i + 1..=i + shape.degree].iter().sum::<f64>() / shape.degree as f64;
            mapping.map(start + u * t.length, *value)
        })
        .collect();
    painter.add(egui::Shape::line(
        points.clone(),
        Stroke::new(0.75, color.gamma_multiply(0.35)),
    ));
    let mut feedback = None;
    for (control, &pos) in points
        .iter()
        .enumerate()
        .take(shape.values.len() - 3)
        .skip(3)
    {
        if !mapping.rect.shrink(9.).contains(pos) {
            continue;
        }
        let target = Target {
            section: session.selected,
            channel: session.channel,
            transition: t.id,
            kind: Kind::Control { index: control },
        };
        let handle = ui
            .interact(
                egui::Rect::from_center_size(pos, egui::vec2(18., 18.)),
                target.id(),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeVertical);
        graph_gesture::capture(session, mapping, &handle, target, index);
        let dragging = session
            .graph_drag
            .as_ref()
            .is_some_and(|d| d.target == target);
        painter.circle(
            pos,
            5.,
            if handle.hovered() || dragging {
                Color32::WHITE
            } else {
                Color32::BLACK
            },
            Stroke::new(1.5, color),
        );
        if handle.hovered() || dragging {
            feedback = Some(format!(
                "Shape control {} · {:.8} · local edit; both endpoint jets fixed · Shift: fine · Esc: cancel",
                control - 2,
                shape.values[control]
            ));
        }
        handle.on_hover_text("NURBS control ordinate (not a point on the curve). Drag vertically; both endpoint values, slopes and second derivatives stay fixed. Shift: fine. Esc: cancel.");
    }
    feedback
}
