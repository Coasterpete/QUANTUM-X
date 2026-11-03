//! Precision entry for the very same authored points used by graph gestures.
use super::{
    graph_gesture::{Kind, Target},
    session::Session,
    snap::{GraphAxis, unit_label},
};
use qx_geometry::transition::Function;

pub(super) fn selected(s: &mut Session, ui: &mut egui::Ui) {
    let Some(target) = s.graph_selection.filter(|t| {
        t.device == s.graph_device() && t.section == s.graph_id() && t.channel == s.graph_key()
    }) else {
        ui.label("Select a square point, circular shape handle, plateau bar or boundary diamond.");
        return;
    };
    let mut section = s.graph_section();
    let domain = section.domain;
    let c = section.channel_mut(target.channel).unwrap();
    let Some(index) = c.transitions.iter().position(|t| t.id == target.transition) else {
        return;
    };
    if index != s.graph_transition() {
        ui.label("Select a graph point in the active transition to edit its coordinates.");
        return;
    }
    let start: f64 = c.transitions[..index].iter().map(|t| t.length).sum();
    let Some((mut station, mut value)) = coordinates(s, target) else {
        return;
    };
    let original = (station, value);
    let xunit = match domain {
        qx_geometry::authoring::Domain::Distance => "m",
        qx_geometry::authoring::Domain::Time => "s",
        _ => "u",
    };
    let unit = if target.device.is_some() {
        qx_geometry::transition::Unit::MetresPerSecondSquared
    } else {
        c.unit
    };
    ui.label(format!(
        "Transition {} · {}",
        index + 1,
        match target.kind {
            Kind::Endpoint { end: false } => "start point",
            Kind::Endpoint { end: true } => "end point",
            Kind::Boundary => "boundary",
            Kind::Control { .. } => "shape / easing handle",
            Kind::Plateau => "plateau level",
        }
    ));
    let movable = match target.kind {
        Kind::Boundary | Kind::Control { .. } => true,
        Kind::Endpoint { end } => {
            if end {
                index + 1 < c.transitions.len()
            } else {
                index > 0
            }
        }
        Kind::Plateau => false,
    };
    let xaxis = match target.kind {
        Kind::Control { .. } => GraphAxis::HandleStation(domain),
        Kind::Boundary => GraphAxis::Boundary(domain),
        _ => GraphAxis::PointStation(domain),
    };
    ui.horizontal(|ui| {
        ui.label(format!("Station {xunit}"));
        ui.add_enabled_ui(movable, |ui| {
            super::snap_ui::graph_number(ui, &mut station, "", 0.01, s.workbench.snap, xaxis)
        });
        if target.kind != Kind::Boundary {
            ui.label(unit_label(unit));
            super::snap_ui::graph_number(
                ui,
                &mut value,
                "",
                0.001,
                s.workbench.snap,
                if matches!(target.kind, Kind::Control { .. }) {
                    GraphAxis::Handle(unit)
                } else {
                    GraphAxis::Value(unit)
                },
            );
        }
    });
    if original == (station, value) {
        return;
    }
    let result = match target.kind {
        Kind::Endpoint { end } => {
            qx_document::region_edit::point(&section, target.channel, index, end, station, value)
        }
        Kind::Boundary => {
            qx_document::region_edit::boundary(&section, target.channel, index, station - start)
        }
        Kind::Control { index: control } => qx_document::region_edit::shape_point(
            &section,
            target.channel,
            index,
            control,
            station,
            value,
        ),
        Kind::Plateau => {
            qx_document::region_edit::plateau_level(&section, target.channel, index, value)
        }
    };
    match result {
        Ok(next) => s.accept_graph_section(next),
        Err(e) => s.status = e.to_string(),
    }
}

pub(super) fn coordinates(s: &Session, target: Target) -> Option<(f64, f64)> {
    let mut section = s.graph_section();
    let c = section.channel_mut(target.channel).ok()?;
    let index = c
        .transitions
        .iter()
        .position(|t| t.id == target.transition)?;
    let t = &c.transitions[index];
    let start: f64 = c.transitions[..index].iter().map(|t| t.length).sum();
    Some(match target.kind {
        Kind::Endpoint { end } => (
            start + if end { t.length } else { 0. },
            if end { t.end } else { t.start },
        ),
        Kind::Boundary => (start + t.length, t.end),
        Kind::Plateau => (start + t.length * 0.5, t.start),
        Kind::Control { index } => {
            let converted = t.to_nurbs().ok()?;
            let Function::Nurbs { mut profile } = converted.function else {
                return None;
            };
            profile.refine_for_edit().ok()?;
            if index < 3 || index >= profile.values.len() - 3 {
                return None;
            }
            (
                start
                    + profile.knots[index + 1..=index + profile.degree]
                        .iter()
                        .sum::<f64>()
                        / profile.degree as f64
                        * t.length,
                profile.values[index],
            )
        }
    })
}

pub(super) fn tools(s: &mut Session, ui: &mut egui::Ui) {
    let mut section = s.graph_section();
    let domain = section.domain;
    let c = section.channel_mut(s.graph_key()).unwrap().clone();
    let index = s.graph_transition().min(c.transitions.len() - 1);
    let original = c.transitions[index].length;
    let mut length = original;
    ui.horizontal(|ui| {
        ui.label("Transition length");
        if ui
            .add_enabled_ui(c.transitions.len() > 1, |ui| {
                super::snap_ui::graph_number(
                    ui,
                    &mut length,
                    "",
                    0.01,
                    s.workbench.snap,
                    GraphAxis::Boundary(domain),
                )
            })
            .inner
            .changed()
        {
            let (boundary, extent) = if index + 1 < c.transitions.len() {
                (index, length)
            } else {
                (
                    index - 1,
                    c.transitions[index - 1].length + original - length,
                )
            };
            match qx_document::region_edit::boundary(&section, s.graph_key(), boundary, extent) {
                Ok(next) => s.accept_graph_section(next),
                Err(e) => s.status = e.to_string(),
            }
        }
    });
    ui.small("Length transfers extent with an adjacent transition. Region extent stays fixed.");
    if ui.button("Entrance → plateau → exit").on_hover_text("Explicit reshape of this transition into three connected C2 profiles, each initially one third of its extent. Select the plateau bar to set its level; drag either boundary to adjust ramp / hold lengths. Preview, then Apply or Cancel.").clicked() {
        let index = s.graph_transition();
        match qx_document::region_edit::ramp_plateau(&s.graph_section(), s.graph_key(), index) {
            Ok(next) => {
                if let Err(error) = s.set_graph_section(next) {
                    s.status = error.clone();
                    s.graph_feedback = Some(error);
                    return;
                }
                s.graph_feedback = None;
                s.set_graph_transition(index + 1);
                let mut section = s.graph_section();
                let transition = section.channel_mut(s.graph_key()).unwrap().transitions[index + 1].id;
                s.graph_selection = Some(Target { device: s.graph_device(), section: s.graph_id(), channel: s.graph_key(), transition, kind: Kind::Plateau });
            },
            Err(e) => s.status = e.to_string(),
        }
    }
}
