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
    commands.is_empty() || session.execute(commands)
}

pub(super) fn cancel(session: &mut Session) {
    session.metadata_draft = session.editor.project().metadata.clone();
    session.style_draft = session.editor.project().style.clone();
    session.style_family = session.style_draft.family_id.clone();
}

pub(super) fn show(session: &mut Session, ui: &mut egui::Ui) {
    ui.heading("Coaster Properties");
    let changed = session.metadata_draft != session.editor.project().metadata
        || session.style_draft != session.editor.project().style;
    // Validate only these fields while drawing. Full document validation belongs
    // at the existing transaction boundary, not every properties redraw.
    let validation = session.metadata_draft.validate().and_then(|()| {
        session
            .style_draft
            .validate(&session.catalog)
            .map_err(Into::into)
    });
    if let Err(error) = &validation {
        ui.colored_label(super::theme::ERROR, error.to_string());
    }
    let mut apply_requested = false;
    let mut cancel_requested = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(changed && validation.is_ok(), egui::Button::new("Apply"))
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
    if let Ok(style) = session
        .catalog
        .validate_selection(&session.style_draft)
        .cloned()
    {
        ui.strong(format!("{} - {}", style.name, style.status.label()));
        ui.label(&style.description);
        ui.small("Catalog assignment records design intent. Planned styles have no production models. Construction is configured separately below.");
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
    ui.small("Apply creates one undoable transaction. Style assignment preserves authored geometry, construction and train settings.");
    if ui.button("Open Train Workspace").clicked() {
        session.workbench.active = Profile::Train;
    }
    ui.separator();
    ui.collapsing("Track construction", |ui| {
        super::construction_ui::controls(session, ui);
    });
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
        "Production content unavailable. Empty folders are not completed assets."
    });
}
