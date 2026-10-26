//! Construction settings in the owner's existing Coaster context. Validation
//! examples are deliberately separate from the conceptual production catalog.
use super::session::Session;
use qx_style::construction::{HardwareShape, Profile, TrackConstruction};
pub(super) fn controls(session: &mut Session, ui: &mut egui::Ui) {
    ui.strong("Procedural construction");
    ui.small("Engine validation configurations · provisional dimensions. Independent of the concept catalog below.");
    let c = &mut session.construction_draft;
    egui::ComboBox::from_id_salt("validation construction")
        .selected_text(match c.strategy {
            qx_style::construction::Strategy::DualRail => "Generic dual-rail steel",
            qx_style::construction::Strategy::SingleRail => "Generic single-rail beam",
            _ => "Custom construction",
        })
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(c.id == "validation-dual", "Generic dual-rail steel")
                .clicked()
            {
                *c = TrackConstruction::generic_dual();
            }
            if ui
                .selectable_label(c.id == "validation-single", "Generic single-rail beam")
                .clicked()
            {
                *c = TrackConstruction::generic_single();
            }
        });
    ui.small("Offsets: T forward / N rider up / L lateral (m). No authored centerline changes.");
    for component in &mut c.continuous {
        ui.push_id(&component.id, |ui| {
            ui.collapsing(&component.id, |ui| {
                ui.checkbox(&mut component.visible, "Visible");
                match &mut component.profile {
                    Profile::Circle { radius_m, .. } => {
                        number(ui, "Rail radius (m)", radius_m);
                    }
                    Profile::Box { height_m, width_m } => {
                        number(ui, "Height (m)", height_m);
                        number(ui, "Width (m)", width_m);
                    }
                    Profile::Polygon { points } => {
                        for axis in 0..2 {
                            let min = points.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min);
                            let max = points
                                .iter()
                                .map(|p| p[axis])
                                .fold(f64::NEG_INFINITY, f64::max);
                            let old = max - min;
                            let mut extent = old;
                            number(
                                ui,
                                if axis == 0 {
                                    "Profile height (m)"
                                } else {
                                    "Profile width (m)"
                                },
                                &mut extent,
                            );
                            if extent != old && extent.is_finite() && extent > 1e-5 {
                                for point in points.iter_mut() {
                                    point[axis] *= extent / old;
                                }
                            }
                        }
                    }
                }
                for (axis, value) in ["T offset (m)", "N offset (m)", "L offset (m)"]
                    .into_iter()
                    .zip(&mut component.offset_m)
                {
                    number(ui, axis, value);
                }
                number(ui, "Profile rotation (rad)", &mut component.rotation_rad);
            });
        });
    }
    for repeat in &mut c.repeated {
        ui.push_id(&repeat.id, |ui| {
            ui.collapsing(&repeat.id, |ui| {
                ui.checkbox(&mut repeat.visible, "Visible");
                number(ui, "Spacing (m)", &mut repeat.spacing_m);
                number(ui, "Start station (m)", &mut repeat.start_m);
                number(ui, "End margin (m)", &mut repeat.end_margin_m);
                if let HardwareShape::Box { size_m } = &mut repeat.shape {
                    for (label, value) in ["Length T (m)", "Height N (m)", "Width L (m)"]
                        .into_iter()
                        .zip(size_m)
                    {
                        number(ui, label, value);
                    }
                }
                for (label, value) in ["T offset (m)", "N offset (m)", "L offset (m)"]
                    .into_iter()
                    .zip(&mut repeat.offset_m)
                {
                    number(ui, label, value);
                }
            });
        });
    }
    ui.collapsing("Component materials", |ui| {
        for material in &mut c.materials {
            ui.push_id(&material.id, |ui| {
                ui.label(&material.id);
                ui.color_edit_button_rgb(&mut material.base_color);
                ui.add(egui::Slider::new(&mut material.metallic, 0.0..=1.0).text("Metallic"));
                ui.add(egui::Slider::new(&mut material.roughness, 0.0..=1.0).text("Roughness"));
            });
        }
    });
    let changed = c != &session.editor.project().construction;
    if let Err(error) = c.validate() {
        ui.colored_label(egui::Color32::LIGHT_RED, error.to_string());
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(changed, egui::Button::new("Apply construction"))
            .clicked()
        {
            session.execute(vec![qx_document::command::Command::SetConstruction(
                session.construction_draft.clone(),
            )]);
        }
        if ui
            .add_enabled(changed, egui::Button::new("Cancel construction"))
            .clicked()
        {
            session.construction_draft = session.editor.project().construction.clone();
            session.refresh_view();
        }
    });
    if changed {
        ui.small("Construction draft · valid geometry previews. Apply to save; Cancel restores the committed construction.");
    }
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.005).max_decimals(8));
    });
}
