//! Coaster identity/catalog assignment, separate from geometry and train mechanics.
use super::{
    session::Session,
    workbench::{Pane, Profile},
};
use qx_document::command::Command;
use qx_style::{StyleSelection, library::LibraryKind};

pub(super) fn open(session: &mut Session) {
    session.workbench.active = Profile::Track;
    session.workbench.layouts[0].dock(Pane::Track);
    session.track_tab = 1;
}

pub(super) fn apply(session: &mut Session) -> bool {
    let p = session.editor.project();
    let mut commands = Vec::new();
    if session.metadata_draft != p.metadata {
        commands.push(Command::SetMetadata(session.metadata_draft.clone()));
    }
    if session.style_draft != p.style {
        commands.push(Command::SetStyle(session.style_draft.clone()));
    }
    if session.style_construction_base.is_some() {
        if session.visible_revision != session.revision || session.draft_error.is_some() {
            session.status =
                "Wait for a valid construction preview before applying the style.".into();
            return false;
        }
        commands.push(Command::SetConstruction(session.construction_draft.clone()));
    }
    let applied = commands.is_empty() || session.execute(commands);
    if applied {
        session.style_construction_base = None;
    }
    applied
}

/// Selection changes visual construction only. The generation worker verifies
/// source bytes and the complete path before Apply becomes available.
pub(super) fn select_style(session: &mut Session, selection: StyleSelection) -> Result<(), String> {
    let construction = if selection.is_unassigned() {
        None
    } else {
        let style = session
            .catalog
            .validate_selection(&selection)
            .map_err(|e| e.to_string())?;
        if style.status == qx_style::DevelopmentStatus::Planned {
            None
        } else {
            let entry = session
                .library
                .entry(LibraryKind::Track, &style.id)
                .ok_or("Track style is not registered")?;
            if style.status == qx_style::DevelopmentStatus::Available && !entry.content_ready {
                return Err(format!(
                    "Track construction unavailable: {}",
                    entry.issues.join("; ")
                ));
            }
            Some(entry.manifest.as_ref().and_then(|m|m.construction.clone()).ok_or("Track construction is missing; restore this style's source assets and reload the project")?)
        }
    };
    if let Some(mut construction) = construction {
        let base = session
            .style_construction_base
            .clone()
            .unwrap_or_else(|| session.construction_draft.clone());
        construction
            .reuse_component_appearances(&session.construction_draft)
            .map_err(|e| e.to_string())?;
        session.style_construction_base = Some(base);
        session.construction_draft = construction;
    } else if let Some(base) = session.style_construction_base.take() {
        session.construction_draft = base;
    }
    session.style_draft = selection;
    session.refresh_view();
    Ok(())
}

pub(super) fn cancel(session: &mut Session) {
    if let Some(base) = session.style_construction_base.take() {
        session.construction_draft = base;
    }
    session.metadata_draft = session.editor.project().metadata.clone();
    session.style_draft = session.editor.project().style.clone();
    session.style_family = session.style_draft.family_id.clone();
    session.refresh_view();
}

pub(super) fn show(session: &mut Session, ui: &mut egui::Ui) {
    ui.heading("Coaster Properties");
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(
            &mut session.material_ui.appearance_tab,
            false,
            "Identity & style",
        );
        ui.selectable_value(
            &mut session.material_ui.appearance_tab,
            true,
            "Track appearance",
        );
    });
    if session.material_ui.appearance_tab {
        let changed = session.construction_draft != session.editor.project().construction;
        let valid = session.construction_draft.validate().is_ok();
        let mut apply = false;
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            apply = ui
                .add_enabled(
                    changed && valid,
                    egui::Button::new(if session.style_construction_base.is_some() {
                        "Apply style and appearance"
                    } else {
                        "Apply appearance"
                    }),
                )
                .clicked();
            cancel = ui
                .add_enabled(
                    changed,
                    egui::Button::new(if session.style_construction_base.is_some() {
                        "Cancel style"
                    } else {
                        "Cancel appearance"
                    }),
                )
                .clicked();
        });
        super::material_ui::show(session, ui);
        if apply {
            super::material_ui::apply(session);
        } else if cancel {
            super::material_ui::cancel(session);
        }
        return;
    }
    let changed = session.metadata_draft != session.editor.project().metadata
        || session.style_draft != session.editor.project().style
        || (session.style_construction_base.is_some()
            && session.construction_draft != session.editor.project().construction);
    // Validate only these fields while drawing. Full document validation belongs
    // at the existing transaction boundary, not every properties redraw.
    let validation = session.metadata_draft.validate().and_then(|()| {
        session
            .style_draft
            .validate(&session.catalog)
            .map_err(Into::into)
    });
    let construction_ready = session.style_construction_base.is_none()
        || (session.visible_revision == session.revision && session.draft_error.is_none());
    if !construction_ready {
        ui.small("Checking source geometry and updating construction… Apply is available after a valid preview.");
    }
    if let Err(error) = &validation {
        ui.colored_label(super::theme::ERROR, error.to_string());
    }
    let mut apply_requested = false;
    let mut cancel_requested = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                changed && validation.is_ok() && construction_ready,
                egui::Button::new("Apply"),
            )
            .clicked()
        {
            apply_requested = true;
        }
        if ui
            .add_enabled(changed, egui::Button::new("Cancel"))
            .clicked()
        {
            cancel_requested = true;
        }
    });

    ui.label("Coaster name")
        .on_hover_text("Also names this single-coaster project.");
    ui.add(
        egui::TextEdit::singleline(&mut session.metadata_draft.name).desired_width(f32::INFINITY),
    );
    ui.label("Description");
    ui.add(
        egui::TextEdit::multiline(&mut session.metadata_draft.description)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    ui.collapsing("Project author", |ui| {
        ui.text_edit_singleline(&mut session.metadata_draft.author);
    });
    ui.separator();
    ui.label("Coaster family");
    let family_name = session
        .catalog
        .families
        .iter()
        .find(|f| f.id == session.style_family)
        .map_or("Choose a family", |f| f.name.as_str());
    egui::ComboBox::from_id_salt("coaster family")
        .selected_text(family_name)
        .show_ui(ui, |ui| {
            for f in &session.catalog.families {
                ui.selectable_value(&mut session.style_family, f.id.clone(), &f.name);
            }
        });
    if let Some(family) = session
        .catalog
        .families
        .iter()
        .find(|f| f.id == session.style_family)
    {
        ui.small(&family.description);
    }
    ui.label("Numbered track style");
    let styles: Vec<_> = {
        let mut styles: Vec<_> = session
            .catalog
            .styles_for_family(&session.style_family)
            .collect();
        styles.sort_by_key(|s| s.number);
        styles
    };
    let selected = session
        .catalog
        .validate_selection(&session.style_draft)
        .ok();
    let previous_selection = session.style_draft.clone();
    egui::ComboBox::from_id_salt("coaster numbered style")
        .selected_text(selected.map_or("Unassigned", |s| s.name.as_str()))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut session.style_draft,
                StyleSelection::default(),
                "Unassigned",
            );
            for style in styles {
                ui.selectable_value(
                    &mut session.style_draft,
                    StyleSelection {
                        family_id: style.family_id.clone(),
                        style_id: style.id.clone(),
                    },
                    format!("{} - {}", style.name, style.status.label()),
                );
            }
        });
    if session.style_draft != previous_selection {
        let selection = session.style_draft.clone();
        session.style_draft = previous_selection;
        if let Err(error) = select_style(session, selection) {
            session.status = error.clone();
            session.draft_error = Some(error);
        }
    }
    if let Ok(style) = session
        .catalog
        .validate_selection(&session.style_draft)
        .cloned()
    {
        ui.strong(format!("{} - {}", style.name, style.status.label()));
        ui.label(&style.description);
        ui.small(if style.status == qx_style::DevelopmentStatus::Planned {
            "Planned style: records design intent; construction is not replaced."
        } else { "Registered construction previews in the viewport. Apply keeps the style and component appearance together." });
        if style.status == qx_style::DevelopmentStatus::InDevelopment {
            ui.colored_label(super::theme::ORANGE, "Construction in development. Review the unresolved details in the style description before applying.");
        }
        preview(session, &style, ui);
        ui.collapsing("Train compatibility", |ui| {
            ui.small("Catalog declarations describe intended compatibility; they do not validate vehicle dynamics.");
            for train in &session.catalog.train_styles {
                if let Ok(assessment) = session.catalog.compatibility(&style.id, &train.id) {
                    ui.label(format!("{}: {}", train.name, compatibility(assessment)));
                }
            }
            for train in &session.editor.project().trains {
                ui.label(format!("Configured {}: {}", train.name, compatibility(train.compatibility.assess(&style))));
            }
            if session.catalog.train_styles.is_empty() && session.editor.project().trains.is_empty() {
                ui.label("No compatible train information registered.");
            }
        });
    } else {
        ui.label("Unassigned - track and graph authoring remain available.");
        ui.small("Choose a final construction style when ready. Train compatibility is unknown until a style is assigned.");
    }
    ui.small("Apply creates one undoable transaction. Authored regions, transition profiles, reference geometry and train settings are preserved.");
    if ui.button("Open Train Workspace").clicked() {
        session.workbench.active = Profile::Train;
    }
    ui.separator();
    ui.collapsing("Advanced construction", |ui| {
        super::construction_ui::assembly_controls(session, ui);
    });
    if session.development {
        ui.collapsing("Advanced construction · development", |ui| {
            super::construction_ui::controls(session, ui);
        });
    }
    if session.development {
        ui.collapsing("Library diagnostics - development", |ui| {
            super::library_ui::tracks(session, ui);
        });
    }
    // Header actions run after text/combo inputs consume the entire native batch.
    if apply_requested {
        apply(session);
    } else if cancel_requested {
        cancel(session);
    }
}

fn compatibility(value: qx_style::compatibility::Compatibility) -> String {
    use qx_style::compatibility::Compatibility::*;
    match value {
        Declared => "Declared compatible".into(),
        Incompatible(reason) => format!("Incompatible: {reason}"),
        Unspecified => "Unknown".into(),
    }
}

fn preview(session: &mut Session, style: &qx_style::Style, ui: &mut egui::Ui) {
    let entry = session.library.entry(LibraryKind::Track, &style.id);
    let images = entry.and_then(|e| e.manifest.as_ref()).map(|m| &m.previews);
    let thumbnail = images.and_then(|p| p.thumbnail.as_deref());
    let large = images.and_then(|p| p.large.as_deref());
    let mut shown = false;
    for (label, manifest_path, legacy_path, height) in [
        (
            "Style thumbnail",
            thumbnail,
            style.thumbnail.as_deref(),
            80.,
        ),
        ("Style preview", large, style.preview.as_deref(), 240.),
    ] {
        let path = if let Some(path) = manifest_path {
            session
                .library
                .asset(LibraryKind::Track, &style.id, path)
                .ok()
        } else {
            legacy_path
                .and_then(|p| qx_style::library::resolve_existing(&session.library.root, p).ok())
        };
        if let Some(path) = path {
            ui.label(label);
            session.previews.show(ui, path, label, height);
            shown = true;
        }
    }
    if !shown {
        ui.label("Preview unavailable - awaiting style artwork.");
    }
    ui.small(if entry.is_some_and(|e| e.content_ready) {
        "Library content resolves; asset quality and mechanical validation are separate."
    } else {
        "Content readiness and unresolved assembly details are shown in the style description."
    });
}
