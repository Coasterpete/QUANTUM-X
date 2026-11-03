//! Construction settings in the owner's existing Coaster context. Validation
//! examples are deliberately separate from the conceptual production catalog.
use super::session::Session;
use qx_style::construction::{HardwareShape, Profile, TrackConstruction};

/// Optional numeric construction editing; no asset IDs or source transforms.
/// All values edit the existing compositional document, with no family checks.
pub(super) fn assembly_controls(session: &mut Session, ui: &mut egui::Ui) {
    ui.small("Visual assembly dimensions in metres. Style 100 defaults are provisional development values. Track authoring and material finishes are preserved.");
    let c = &mut session.construction_draft;
    let rails: Vec<_> = c
        .continuous
        .iter()
        .enumerate()
        .filter(|(_, c)| c.role == qx_style::construction::Role::Running)
        .map(|(i, _)| i)
        .collect();
    if rails.len() == 2 {
        let (a, b) = (rails[0], rails[1]);
        let center = (c.continuous[a].offset_m[2] + c.continuous[b].offset_m[2]) * 0.5;
        let old = c.continuous[b].offset_m[2] - c.continuous[a].offset_m[2];
        let mut spacing = old.abs();
        ui.horizontal(|ui| {
            ui.label("Rail center spacing (m)");
            if ui
                .add(
                    egui::DragValue::new(&mut spacing)
                        .speed(0.01)
                        .range(0.001..=1000.)
                        .max_decimals(6),
                )
                .changed()
            {
                let half = spacing * 0.5 * if old < 0. { -1. } else { 1. };
                c.continuous[a].offset_m[2] = center - half;
                c.continuous[b].offset_m[2] = center + half;
            }
        });
        if let Profile::Circle { radius_m, .. } = c.continuous[a].profile
            && matches!(c.continuous[b].profile, Profile::Circle { .. })
        {
            let mut diameter = radius_m * 2.;
            ui.horizontal(|ui| {
                ui.label("Rail diameter (m)");
                if ui
                    .add(
                        egui::DragValue::new(&mut diameter)
                            .speed(0.001)
                            .range(0.00002..=1000.)
                            .max_decimals(6),
                    )
                    .changed()
                {
                    for i in rails {
                        if let Profile::Circle { radius_m, .. } = &mut c.continuous[i].profile {
                            *radius_m = diameter * 0.5;
                        }
                    }
                }
            });
        }
    }
    for component in &mut c.repeated {
        ui.push_id(&component.id, |ui| {
            let old = component.spacing_m;
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} spacing (m)",
                    super::material_ui::component_name(&component.id)
                ));
                if ui
                    .add(
                        egui::DragValue::new(&mut component.spacing_m)
                            .speed(0.01)
                            .range(0.00001..=1000.)
                            .max_decimals(6),
                    )
                    .changed()
                {
                    // Preserve the registered relative phase/margins when the
                    // user explicitly changes pitch; do not reset at sections.
                    component.start_m *= component.spacing_m / old;
                    component.end_margin_m *= component.spacing_m / old;
                }
            });
        });
    }
    let changed = c != &session.editor.project().construction;
    let valid = c.validate().is_ok();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(changed && valid, egui::Button::new("Apply geometry"))
            .clicked()
        {
            super::material_ui::apply(session);
        }
        if ui
            .add_enabled(changed, egui::Button::new("Cancel geometry"))
            .clicked()
        {
            super::material_ui::cancel(session);
        }
    });
}
pub(super) fn controls(session: &mut Session, ui: &mut egui::Ui) {
    ui.strong("Procedural construction");
    ui.small("Engine validation configurations · provisional dimensions. Independent of planned library styles below.");
    let c = &mut session.construction_draft;
    let slots: Vec<_> = c.materials.iter().map(|m| m.id.clone()).collect();
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
                let mut next = TrackConstruction::generic_dual();
                next.reuse_material_bindings(c);
                *c = next;
            }
            if ui
                .selectable_label(c.id == "validation-single", "Generic single-rail beam")
                .clicked()
            {
                let mut next = TrackConstruction::generic_single();
                next.reuse_material_bindings(c);
                *c = next;
            }
        });
    ui.small("Offsets: T forward / N rider up / L lateral (m). No authored centerline changes.");
    for component in &mut c.continuous {
        ui.push_id(&component.id, |ui| {
            ui.collapsing(&component.id, |ui| {
                ui.checkbox(&mut component.visible, "Visible");
                if session.development {
                    slot(ui, &mut component.material, &slots);
                }
                match &mut component.profile {
                    Profile::AssetSection { .. } => {
                        ui.small("Original source cross section; dimensions are preserved.");
                    }
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
                if session.development {
                    slot(ui, &mut repeat.material, &slots);
                }
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
    if session.development {
        ui.collapsing("Component materials · development slots", |ui| {
        if session.development && c.materials.len() < 64 && ui.button("Add material slot (development)").clicked() {
            let mut number = 1;
            while c.materials.iter().any(|m| m.id == format!("material-{number}")) { number += 1; }
            c.materials.push(qx_style::construction::Material { id: format!("material-{number}"), library: None,
                base_color: [0.35;3], metallic: 0., roughness: 0.9 });
        }
        for material in &mut c.materials {
            ui.push_id(&material.id, |ui| {
                ui.label(&material.id);
                if session.development {
                    ui.small("Development-only PBR assignment · Apply/Cancel and undo use the construction transaction.");
                    match &session.material_catalog {
                        Ok(library) => {
                            egui::ComboBox::from_id_salt("track library material")
                                .selected_text(material.library.as_ref().map_or("Scalar fallback", |b| &b.id))
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(material.library.is_none(), "Scalar fallback").clicked() { material.library = None; }
                                    for entry in &library.materials {
                                        if ui.selectable_label(material.library.as_ref().is_some_and(|b| b.id == entry.id), &entry.name).clicked() {
                                            material.library = Some(qx_style::materials::Binding::new(&entry.id));
                                        }
                                    }
                                });
                        },
                        Err(error) => { ui.label(error); },
                    }
                    if let Some(binding) = &mut material.library {
                        ui.horizontal_wrapped(|ui| {
                            for (label, value) in ["U scale", "V scale", "U offset", "V offset"].into_iter().zip(binding.uv_scale.iter_mut().chain(&mut binding.uv_offset)) {
                                ui.label(label); ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6));
                            }
                        });
                    }
                }
                if material.library.is_some() { ui.small("Library material assigned. Values below are the explicit missing-file fallback; they do not tint the maps."); }
                ui.color_edit_button_rgb(&mut material.base_color);
                ui.add(egui::Slider::new(&mut material.metallic, 0.0..=1.0).text("Metallic"));
                ui.add(egui::Slider::new(&mut material.roughness, 0.0..=1.0).text("Roughness"));
            });
        }
    });
    }
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
fn slot(ui: &mut egui::Ui, value: &mut String, slots: &[String]) {
    egui::ComboBox::from_id_salt("component material slot")
        .selected_text(value.as_str())
        .show_ui(ui, |ui| {
            for id in slots {
                ui.selectable_value(value, id.clone(), id);
            }
        });
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.005).max_decimals(8));
    });
}
