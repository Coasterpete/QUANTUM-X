use super::{expansion, session::Session};
use qx_document::operations::{DeviceKind, TrackDevice};
use qx_geometry::topology::{self, LayoutMode};

pub(super) fn show(s: &mut Session, ui: &mut egui::Ui) {
    ui.heading("Layout and track devices");
    ui.small("Contextual tools · placement is provisional; all edits require Apply.");
    if ui.button("Engineering plots").clicked() {
        s.expansion.engineering = true;
    }
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [
            (LayoutMode::Open, "Open"),
            (LayoutMode::Shuttle, "Shuttle"),
            (LayoutMode::Circuit, "Circuit"),
        ] {
            ui.selectable_value(&mut s.expansion.operations.layout, mode, label);
        }
    });
    // Work from committed geometry, never an unrelated draft or stale viewport.
    let track = s.expansion.committed_track(s.editor.project(), s.options);
    if let Ok(track) = &track {
        if let Ok(d) = topology::diagnose(track) {
            ui.strong(topology::status(s.expansion.operations.layout, d));
            ui.monospace(format!(
                "Gap {:.6} m\nTangent {:.5}° · Frame {:.5}°",
                d.position_gap_m,
                d.tangent_gap_rad.to_degrees(),
                d.frame_gap_rad.to_degrees()
            ));
            ui.small("Closure: ≤1 mm position and ≤0.001 rad tangent/frame. Intent alone never enables seam traversal.");
        }
        ui.collapsing("Connect endpoints", |ui| {
            ui.add(
                egui::DragValue::new(&mut s.expansion.completion_settings.preferred_length_m)
                    .speed(0.5)
                    .range(0.1..=5000.)
                    .prefix("Length m "),
            );
            ui.add(
                egui::DragValue::new(
                    &mut s.expansion.completion_settings.length_tolerance_fraction,
                )
                .speed(0.01)
                .range(0.01..=0.9)
                .prefix("Length tolerance fraction "),
            );
            ui.add(
                egui::DragValue::new(&mut s.expansion.completion_settings.minimum_radius_m)
                    .speed(0.1)
                    .range(0.1..=10000.)
                    .prefix("Minimum radius m "),
            );
            if ui
                .add_enabled(
                    s.expansion.completion_job.is_none(),
                    egui::Button::new("Preview connecting region"),
                )
                .clicked()
            {
                s.start_completion();
            }
            if s.expansion.completion_job.is_some() {
                ui.spinner();
                ui.small("Bounded completion worker running; Cancel retains the original layout.");
            }
            if let Some((_, _, c)) = &s.expansion.completion {
                ui.label(format!(
                    "Candidate gap {:.8} m · frame {:.8} rad",
                    c.closure.position_gap_m, c.closure.frame_gap_rad
                ));
            }
        });
    } else if let Err(e) = track.as_ref() {
        ui.label(e.to_string());
    }
    ui.separator();
    ui.strong("Track devices · longitudinal force commands");
    ui.horizontal(|ui| {
        for (kind, label) in [
            (DeviceKind::Launch, "Add launch"),
            (DeviceKind::Brake, "Add brake"),
        ] {
            if ui.button(label).clicked()
                && let Ok(track) = &track
            {
                let range = track
                    .sections
                    .iter()
                    .find(|r| r.id == s.selected)
                    .unwrap_or(&track.sections[0]);
                if let Some(id) = s
                    .expansion
                    .operations
                    .devices
                    .iter()
                    .map(|d| d.id)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                {
                    s.expansion.operations.devices.push(TrackDevice::new(
                        id,
                        kind,
                        range.start_distance_m,
                        range.end_distance_m,
                    ));
                    s.expansion.device = Some(id);
                }
            }
        }
    });
    for d in &s.expansion.operations.devices {
        if ui
            .selectable_label(s.expansion.device == Some(d.id), &d.name)
            .clicked()
        {
            s.expansion.device = Some(d.id);
            s.expansion.curve_device = d.acceleration_curve.as_ref().map(|_| d.id);
            s.expansion.curve_transition = 0;
            s.expansion.engineering = false;
        }
    }
    if let Some(index) = s
        .expansion
        .operations
        .devices
        .iter()
        .position(|d| Some(d.id) == s.expansion.device)
    {
        let circuit = track.as_ref().is_ok_and(|t| {
            s.expansion.operations.layout == LayoutMode::Circuit
                && topology::diagnose(t).is_ok_and(|d| d.valid)
        });
        let d = &mut s.expansion.operations.devices[index];
        ui.push_id(("device",d.id),|ui| {
            ui.text_edit_singleline(&mut d.name);ui.checkbox(&mut d.enabled,"Enabled");
            ui.add(egui::DragValue::new(&mut d.start_m).speed(0.1).prefix("Start station m "));
            ui.add(egui::DragValue::new(&mut d.end_m).speed(0.1).prefix("End station m "));
            let constant=d.profile.is_empty() && d.acceleration_curve.is_none();
            ui.add_enabled(constant,egui::DragValue::new(&mut d.acceleration_mps2).speed(0.1).range(0. ..=100.).prefix("Constant magnitude m/s² ")).on_hover_text("Active only for a constant command. A custom profile is edited in the Transition Editor.");
            ui.add(egui::DragValue::new(&mut d.maximum_force_n).speed(100.).range(0. ..=1e9).prefix("Total device force limit N "));
            if d.kind==DeviceKind::Launch {ui.checkbox(&mut d.reverse_launch,"Launch toward decreasing station");}
            ui.small("Brakes oppose actual velocity, including rollback. Limits apply across all occupied masses. Intervals are [start, end).");
            ui.small("Custom curves use the bottom Transition Editor; graph and precision fields share one profile.");
            if !d.profile.is_empty() || d.acceleration_curve.is_some() {
                if ui.button("Fit profile to interval").clicked() && let Ok(t)=&track && let Err(e)=d.fit_profile(t.length_m(),circuit) {s.expansion.message=Some(e.to_string());}
                if ui.button("Use constant command").clicked() {d.profile.clear();d.acceleration_curve=None;}
            }

        });
        if ui
            .button("Custom Acceleration Profile")
            .on_hover_text("Open this device in the bottom Transition Editor")
            .clicked()
            && let Ok(t) = &track
        {
            let mut plain = s.expansion.operations.devices[index].clone();
            plain.profile.clear();
            plain.acceleration_curve = None;
            match plain.length(t.length_m(), circuit) {
                Ok(span) => s.open_device_curve(plain.id, span),
                Err(e) => s.expansion.message = Some(e.to_string()),
            }
        }
        if ui.button("Delete device").clicked() {
            s.expansion.operations.devices.remove(index);
            s.expansion.device = None;
        }
    }
    ui.separator();
    physical(s, ui);
    expansion::actions(s, ui);
}
pub(super) fn physical(s: &mut Session, ui: &mut egui::Ui) {
    ui.collapsing("Physical simulation settings",|ui| {
        let p=&mut s.expansion.operations.physical;
        ui.add(egui::DragValue::new(&mut p.initial_speed_mps).speed(0.1).range(-100. ..=100.).prefix("Initial velocity m/s "));
        ui.add(egui::DragValue::new(&mut p.gravity_mps2).speed(0.01).range(0. ..=50.).prefix("Gravity m/s² "));
        ui.add(egui::DragValue::new(&mut p.rolling_resistance_coefficient).speed(0.0001).range(0. ..=0.2).prefix("Rolling coefficient "));
        ui.add(egui::DragValue::new(&mut p.air_density_kg_m3).speed(0.01).range(0. ..=10.).prefix("Air density kg/m³ "));
        ui.add(egui::DragValue::new(&mut p.drag_area_m2).speed(0.01).range(0. ..=100.).prefix("Train Cd × area m² "));
        ui.small("Resistance: Crr·m·g + ½ρ·CdA·v², opposing travel. Default resistance is zero. Fixed 1/240 s; independent of design speed.");
    });
}
