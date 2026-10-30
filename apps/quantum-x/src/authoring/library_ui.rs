//! Replaceable catalog diagnostics inside existing panes, not the final browser.
use super::session::Session;
use qx_style::{
    Catalog,
    library::{Library, LibraryKind},
};
use std::path::PathBuf;

pub(crate) fn assets_root() -> PathBuf {
    if let Some(root) = std::env::var_os("QX_ASSETS_ROOT") {
        return root.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        let adjacent = exe.parent().unwrap().join("assets");
        let public = exe.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .eq_ignore_ascii_case(&qx_rendering::identity::product().windows_executable)
        });
        if public || adjacent.is_dir() || exe.parent().unwrap().join("product.json").is_file() {
            return adjacent;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}
pub(crate) fn load_catalog() -> (Catalog, Option<String>) {
    match Library::open(&assets_root()) {
        Ok(library) => (library.catalog, None),
        Err(e) => (
            Catalog::built_in().expect("tested built-in catalog"),
            Some(format!(
                "Library index unavailable: {e}. Built-in planned metadata retained."
            )),
        ),
    }
}
pub(crate) fn assets_root_and_error() -> (PathBuf, Option<String>) {
    (assets_root(), load_catalog().1)
}

fn notice(session: &Session, ui: &mut egui::Ui) {
    ui.small("Library inspection · temporary development controls. Planned entries cannot be applied. Folder names imply no mechanics or compatibility.");
    if let Some(error) = &session.library_error {
        ui.label(error);
    }
    ui.small(format!("Assets root: {}", session.library.root.display()));
}

pub(super) fn tracks(session: &mut Session, ui: &mut egui::Ui) {
    notice(session, ui);
    ui.small("Apply visual style explicitly replaces procedural construction in one undoable transaction. Resolve style/construction drafts first; authored track and train settings are preserved.");
    let mut choose = None;
    for family in &session.catalog.families {
        ui.collapsing(&family.name, |ui| {
            let mut styles: Vec<_> = session.catalog.styles_for_family(&family.id).collect();
            styles.sort_by_key(|s| (s.number, &s.id));
            for style in styles {
                ui.push_id(&style.id, |ui| {
                    ui.label(format!("{} · {}", style.name, style.status.label()));
                    ui.small(&style.description);
                    if let Some(directory) = &style.directory {
                        ui.monospace(directory);
                    }
                    if let Some(entry) = session.library.entry(LibraryKind::Track, &style.id) {
                        for issue in &entry.issues {
                            ui.small(issue);
                        }
                        if ui
                            .add_enabled(
                                entry.content_ready
                                    && session.construction_draft
                                        == session.editor.project().construction
                                    && session.style_draft == session.editor.project().style
                                    && (session.editor.project().style.style_id != style.id
                                        || entry
                                            .manifest
                                            .as_ref()
                                            .and_then(|m| m.construction.as_ref())
                                            != Some(&session.editor.project().construction)),
                                egui::Button::new("Apply visual style"),
                            )
                            .clicked()
                        {
                            choose = entry
                                .manifest
                                .as_ref()
                                .and_then(|m| m.construction.clone())
                                .map(|c| {
                                    (
                                        qx_style::StyleSelection {
                                            family_id: family.id.clone(),
                                            style_id: style.id.clone(),
                                        },
                                        c,
                                    )
                                });
                        }
                    }
                    let pairs: Vec<_> = session
                        .catalog
                        .train_styles
                        .iter()
                        .filter_map(|train| {
                            match session.catalog.compatibility(&style.id, &train.id) {
                                Ok(qx_style::compatibility::Compatibility::Declared) => {
                                    Some(train.name.as_str())
                                }
                                _ => None,
                            }
                        })
                        .collect();
                    ui.small(if pairs.is_empty() {
                        "Compatible trains: unspecified".into()
                    } else {
                        format!("Declared trains: {} (metadata only)", pairs.join(", "))
                    });
                });
            }
        });
    }
    if let Some((selection, construction)) = choose {
        session.execute(vec![
            qx_document::command::Command::SetStyle(selection),
            qx_document::command::Command::SetConstruction(construction),
        ]);
    }
}

pub(super) fn trains(session: &mut Session, ui: &mut egui::Ui) {
    ui.collapsing("Train Library · development inspection", |ui| {
        notice(session, ui);
        for family in &session.catalog.train_families {
            ui.collapsing(&family.name, |ui| {
                if let Some(directory) = &family.directory { ui.monospace(directory); }
                let mut styles: Vec<_> = session.catalog.train_styles.iter().filter(|s| s.family_id == family.id).collect();
                styles.sort_by_key(|s| (s.number, &s.id));
                if styles.is_empty() { ui.small("Planned family; no style or mechanical definition registered."); }
                for style in styles {
                    ui.label(format!("{} · {}", style.name, style.status.label()));
                    ui.monospace(&style.directory);
                    if let Some(entry) = session.library.entry(LibraryKind::Train, &style.id) {
                        for issue in &entry.issues { ui.small(issue); }
                        ui.small(if entry.content_ready { "Content resolves; mechanical validation is required through qx-document before use." } else { "Unavailable; no vehicle created." });
                    }
                    if session.editor.project().style.is_unassigned() {
                        ui.small("Selected track compatibility: unknown (style unassigned)");
                    } else if let Ok(assessment) = session.catalog.compatibility(&session.editor.project().style.style_id, &style.id) {
                        ui.small(format!("Selected track compatibility: {assessment:?} (metadata only)"));
                    }
                }
            });
        }
    });
}
