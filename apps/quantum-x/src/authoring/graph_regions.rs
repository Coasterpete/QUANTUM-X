//! Direct region commands and local NURBS controls on the existing graph.
use super::{
    graph_gesture::{self, Kind, Target},
    plot::Plot,
    session::Session,
};
use egui::{Color32, Sense, Stroke};
use qx_geometry::transition::{Function, Transition};

pub(super) fn split(session: &mut Session, station: f64) {
    match qx_document::region_edit::split_at(&session.graph_section(), session.graph_key(), station)
    {
        Ok((section, right)) => {
            session.accept_graph_section(section);
            session.set_graph_transition(right);
            session.status =
                "Transition split exactly; right transition selected. Apply or Cancel the draft."
                    .into();
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
        let raw = mapping.unmap(pos).0;
        let section = session.graph_section();
        let keys: Vec<_> = section
            .channels()
            .iter()
            .flat_map(|(_, c)| {
                let mut cursor = 0.;
                c.transitions.iter().map(move |t| {
                    cursor += t.length;
                    cursor
                })
            })
            .collect();
        let settings = session.workbench.snap;
        let step = settings.graph_step(
            super::snap::GraphAxis::Boundary(section.domain),
            ui.input(|i| i.modifiers.shift),
            ui.input(|i| i.modifiers.alt),
        );
        let station = super::snap::scalar(
            raw,
            step,
            if step.is_some() && settings.graph.nearby_keys {
                &keys
            } else {
                &[]
            },
            settings.graph.key_radius_points * mapping.x.span() / mapping.rect.width() as f64,
        );
        let mut section = session.graph_section();
        let channel = section.channel_mut(session.graph_key()).unwrap();
        let mut start = 0.;
        for (index, t) in channel.transitions.iter().enumerate() {
            if (start..=start + t.length).contains(&station) {
                session.set_graph_transition(index);
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
        let channel = session
            .graph_section()
            .channel_mut(session.graph_key())
            .unwrap()
            .clone();
        let Some(index) = channel.transitions.iter().position(|t| t.id == region) else {
            return;
        };
        let start: f64 = channel.transitions[..index].iter().map(|t| t.length).sum();
        let t = &channel.transitions[index];
        ui.label(format!(
            "Transition {} · {start:.6}–{:.6} {unit}",
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
            match qx_document::region_edit::merge(
                &session.graph_section(),
                session.graph_key(),
                index,
            ) {
                Ok(next) => {
                    session.accept_graph_section(next);
                    session.set_graph_transition(index);
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
            match qx_document::editing::remove_transition(
                &session.graph_section(),
                session.graph_key(),
                region,
            ) {
                Ok(next) => {
                    session.accept_graph_section(next);
                    session.set_graph_transition(index.saturating_sub(1));
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
    if qx_document::region_edit::is_plateau(t) {
        return plateau_control(session, ui, mapping, t, index, start, color);
    }
    let Ok(converted) = t.to_nurbs() else {
        // Analytic cosine remains analytic until an explicit reshape is chosen.
        return None;
    };
    let Function::Nurbs { profile } = &converted.function else {
        unreachable!()
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
            device: session.graph_device(),
            section: session.graph_id(),
            channel: session.graph_key(),
            transition: t.id,
            kind: Kind::Control { index: control },
        };
        let handle = ui
            .interact(
                egui::Rect::from_center_size(pos, egui::vec2(18., 18.)),
                target.id(),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(target.cursor());
        graph_gesture::capture(session, mapping, &handle, target, index);
        let dragging = session
            .graph_drag
            .as_ref()
            .is_some_and(|d| d.target == target);
        painter.circle(
            pos,
            5.,
            if handle.hovered() || dragging || session.graph_selection == Some(target) {
                Color32::WHITE
            } else {
                Color32::BLACK
            },
            Stroke::new(1.5, color),
        );
        if handle.hovered() || dragging {
            feedback = Some(format!(
                "Shape control {} · station {:.8} · {:.8} {} · endpoint jets fixed · Shift: fine · Alt: bypass snap · Esc: cancel",
                control - 2,
                start
                    + shape.knots[control + 1..=control + shape.degree]
                        .iter()
                        .sum::<f64>()
                        / shape.degree as f64
                        * t.length,
                shape.values[control],
                super::snap::unit_label(if session.graph_device().is_some() {
                    qx_geometry::transition::Unit::MetresPerSecondSquared
                } else {
                    session
                        .graph_section()
                        .channel_mut(session.graph_key())
                        .unwrap()
                        .unit
                })
            ));
        }
        handle.on_hover_text("Shape/easing handle: drag horizontally to adjust easing timing; drag vertically to change the scalar control value. Both endpoint values, slopes and second derivatives stay fixed. Shift: fine · Alt: bypass snap · Esc: cancel. Apply commits.");
    }
    feedback
}

fn plateau_control(
    session: &mut Session,
    ui: &egui::Ui,
    mapping: &Plot,
    t: &Transition,
    index: usize,
    start: f64,
    color: Color32,
) -> Option<String> {
    let pos = mapping.map(start + t.length * 0.5, t.start);
    if !mapping.rect.shrink(9.).contains(pos) {
        return None;
    }
    let target = Target {
        device: session.graph_device(),
        section: session.graph_id(),
        channel: session.graph_key(),
        transition: t.id,
        kind: Kind::Plateau,
    };
    let handle = ui
        .interact(
            egui::Rect::from_center_size(pos, egui::vec2(28., 18.)),
            target.id(),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(target.cursor());
    graph_gesture::capture(session, mapping, &handle, target, index);
    let dragging = session
        .graph_drag
        .as_ref()
        .is_some_and(|d| d.target == target);
    ui.painter()
        .with_clip_rect(mapping.rect.intersect(ui.clip_rect()))
        .rect(
            egui::Rect::from_center_size(pos, egui::vec2(20., 7.)),
            1.,
            if handle.hovered() || dragging || session.graph_selection == Some(target) {
                Color32::WHITE
            } else {
                color
            },
            Stroke::new(1., Color32::BLACK),
            egui::StrokeKind::Inside,
        );
    let feedback = (handle.hovered() || dragging).then(|| format!("Plateau level {:.8} · length {:.8} · drag vertically; connected entrance / exit retained · Apply commits", t.start, t.length));
    handle.on_hover_text("Plateau level: drag vertically to change the whole hold and its constrained ramp joins. Timing and both outer ramp endpoints stay fixed. Apply commits.");
    feedback
}
