//! Replaceable M8.1 controls inside the owner-directed Configuration/Preview/Summary.
use super::{session::Session, train_assets};

pub(super) fn controls(s: &mut Session, ui: &mut egui::Ui, display: &qx_rendering::DisplayOptions) {
    ui.heading("Train assets · development controls");
    if ui.add_enabled(!s.assets.importing,egui::Button::new("Import GLB…")).on_hover_text("Static, embedded glTF 2.0 meshes; imported files are packaged beside the project on Save").clicked() {s.file_dialog(super::files::FileAction::ImportGlb);}
    if s.assets.busy() {
        ui.spinner();
        ui.small("Loading train assets…");
    }
    let choices: Vec<_> = s
        .editor
        .project()
        .trains
        .iter()
        .flat_map(|t| {
            t.components.iter().map(|c| {
                (
                    (t.id.clone(), c.id.clone()),
                    format!("{} / {} ({:?})", t.id, c.id, c.kind),
                )
            })
        })
        .collect();
    let before = s.assets.selected.clone();
    let editing = s.assets.draft_dirty(s.editor.project());
    if !choices
        .iter()
        .any(|(id, _)| Some(id) == s.assets.selected.as_ref())
    {
        let associated = s.editor.project().heartline.train_configuration_id.as_ref();
        s.assets.selected = choices
            .iter()
            .find(|(id, _)| Some(&id.0) == associated)
            .or_else(|| choices.first())
            .map(|(id, _)| id.clone());
    }
    ui.add_enabled_ui(!editing, |ui| {
        egui::ComboBox::from_id_salt("train asset component")
            .selected_text(
                choices
                    .iter()
                    .find(|(id, _)| Some(id) == s.assets.selected.as_ref())
                    .map_or("Select component", |(_, label)| label),
            )
            .show_ui(ui, |ui| {
                for (id, label) in &choices {
                    ui.selectable_value(&mut s.assets.selected, Some(id.clone()), label);
                }
            });
    });
    if before != s.assets.selected {
        s.assets.draft = None;
    }
    egui::ComboBox::from_id_salt("imported GLBs")
        .selected_text(if s.assets.chosen.is_empty() {
            "Select imported GLB"
        } else {
            s.assets.chosen.rsplit('/').next().unwrap_or("GLB")
        })
        .show_ui(ui, |ui| {
            for key in s.assets.library.keys() {
                ui.selectable_value(
                    &mut s.assets.chosen,
                    key.clone(),
                    key.rsplit('/').next().unwrap_or(key),
                );
            }
        });
    let current = s
        .assets
        .selected
        .as_ref()
        .and_then(|(t, c)| {
            s.editor
                .project()
                .trains
                .iter()
                .find(|v| &v.id == t)
                .and_then(|t| t.components.iter().find(|v| &v.id == c))
        })
        .and_then(|c| c.asset.clone());
    if let Some(a) = &current {
        ui.small(format!("Assigned: {}", a.path));
    } else {
        ui.small("Development reference geometry (no asset assigned)");
    }
    ui.add_enabled_ui(!editing, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    s.assets.selected.is_some() && s.assets.library.contains_key(&s.assets.chosen),
                    egui::Button::new("Assign / replace"),
                )
                .clicked()
            {
                let (t, c) = s.assets.selected.clone().unwrap();
                let mut a = current
                    .clone()
                    .unwrap_or_else(|| train_assets::identity(s.assets.chosen.clone()));
                a.path = s.assets.chosen.clone();
                if s.assign_train_asset(&t, &c, Some(a)) {
                    s.assets.draft = None;
                    s.assets.fit = true;
                }
            }
            if ui
                .add_enabled(current.is_some(), egui::Button::new("Remove assignment"))
                .clicked()
            {
                let (t, c) = s.assets.selected.clone().unwrap();
                s.assign_train_asset(&t, &c, None);
                s.assets.draft = None;
            }
        });
    });
    // Assign/replace/remove can change the committed asset in this very frame.
    // Seed the numeric draft from the new assignment, never the stale one above.
    let current = s
        .assets
        .selected
        .as_ref()
        .and_then(|(t, c)| {
            s.editor
                .project()
                .trains
                .iter()
                .find(|v| &v.id == t)
                .and_then(|t| t.components.iter().find(|v| &v.id == c))
        })
        .and_then(|c| c.asset.clone());
    if let Some(current) = current {
        if s.assets.draft.is_none() {
            s.assets.draft = Some(current);
        }
        let draft = s.assets.draft.as_mut().unwrap();
        ui.collapsing("Local correction (visual only)", |ui| {
            for (label, v) in [
                ("Translation (m)", &mut draft.translation_m),
                ("Pivot in model (m)", &mut draft.pivot_m),
                ("Scale XYZ", &mut draft.scale),
            ] {
                ui.label(label);
                ui.horizontal_wrapped(|ui| {
                    for (axis, n) in [("X", &mut v.x), ("Y", &mut v.y), ("Z", &mut v.z)] {
                        ui.add(
                            egui::DragValue::new(n)
                                .speed(0.01)
                                .prefix(format!("{axis} ")),
                        );
                    }
                });
            }
            ui.label("Rotation quaternion W / X / Y / Z");
            ui.horizontal_wrapped(|ui| {
                for n in [
                    &mut draft.rotation.w,
                    &mut draft.rotation.x,
                    &mut draft.rotation.y,
                    &mut draft.rotation.z,
                ] {
                    ui.add(egui::DragValue::new(n).speed(0.001));
                }
            });
            ui.small(
                "Apply normalizes the quaternion. No mass, inertia, anchors or constraints change.",
            );
        });
        if ui.button("Apply visual correction").clicked() {
            let mut a = s.assets.draft.clone().unwrap();
            match a.rotation.normalized() {
                Ok(q) => {
                    a.rotation = q;
                    let (t, c) = s.assets.selected.clone().unwrap();
                    if s.assign_train_asset(&t, &c, Some(a)) {
                        s.assets.draft = None;
                    }
                }
                Err(e) => s.status = format!("Invalid rotation: {e}"),
            }
        }
        if s.assets.draft_dirty(s.editor.project()) {
            ui.small("Unapplied visual correction. Apply or cancel before changing component/assignment.");
            if ui.button("Cancel visual correction").clicked() {
                s.assets.draft = None;
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                display.train_bounds.is_some(),
                egui::Button::new("Fit Train · F"),
            )
            .clicked()
        {
            s.assets.fit = true;
        }
        if ui
            .add_enabled(
                display.selected_car_bounds.is_some(),
                egui::Button::new("Focus Selected Car"),
            )
            .clicked()
        {
            s.assets.focus = true;
        }
    });
    ui.collapsing("Engineering references", |ui| {
        ui.checkbox(
            &mut s.assets.wireframe,
            "Engineering wireframe / axes / guides",
        );
        ui.checkbox(&mut s.assets.bogies, "Bogies and bogie markers");
        ui.checkbox(&mut s.assets.wheels, "Wheel-reference markers");
        ui.checkbox(&mut s.assets.joints, "Joint-anchor markers");
        ui.checkbox(&mut s.assets.couplers, "Coupler markers");
        if ui.button("Retry missing assets").clicked() {
            s.assets.errors.clear();
        }
    });
    ui.small("Kinematic inspection; physics playback is not implemented.");
    ui.separator();
}
pub(super) fn diagnostics(s: &Session, ui: &mut egui::Ui) {
    for (path, e) in &s.assets.errors {
        ui.colored_label(egui::Color32::LIGHT_YELLOW, format!("{path}: {e}"));
    }
    for (path, e) in &s.assets.library {
        ui.small(format!(
            "{} · {} meshes · {:.3} ms import",
            path.rsplit('/').next().unwrap_or(path),
            e.model.primitives.len(),
            e.model.import_ms
        ));
        for w in &e.model.warnings {
            ui.small(w);
        }
    }
}
