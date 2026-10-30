//! Destination content; all project actions retain the existing lifecycle services.
use super::startup::{App, LibraryTab, Page};
use qx_rendering::{EditorSurface, StartupSizing, WindowCommand};

pub(super) fn info(ui: &mut egui::Ui) {
    super::theme::heading(ui, "INFO", super::theme::GREEN);
    ui.heading("QUANTUM Dynamics");
    ui.label("Independent roller coaster design and simulation");
    ui.label(format!(
        "Version {} · Alpha development build",
        env!("CARGO_PKG_VERSION")
    ));
    ui.label("Design with ordered track sections, editable transition graphs and precision numerical controls. Each Alpha project contains one coaster.");
    ui.separator();
    ui.label("Track and Train Workspaces share the project document. Coaster Properties configures name, description, family and numbered style inside the Editor.");
    ui.label("Support construction, ride operations and clearance-envelope calculations are not implemented. Experimental physics and numerical diagnostics are available through optional development views.");
    ui.label("Overpass Mono typography · Lucide / Carbon icons");
}

pub(super) fn configuration(app: &mut App, ui: &mut egui::Ui) -> Option<WindowCommand> {
    super::theme::heading(ui, "SETUP", super::theme::ORANGE);
    ui.heading("Application configuration");
    ui.label("Display and window preferences. Project creation is a separate action.");
    let p = &mut app.configuration_draft;
    egui::ComboBox::from_id_salt("welcome startup size")
        .selected_text(format!("{:?}", p.startup))
        .show_ui(ui, |ui| {
            for mode in [
                StartupSizing::Automatic,
                StartupSizing::Custom,
                StartupSizing::Remember,
            ] {
                ui.selectable_value(&mut p.startup, mode, format!("{mode:?}"));
            }
        });
    ui.horizontal_wrapped(|ui| {
        ui.label("Window width / height");
        for value in &mut p.custom_size {
            ui.add(egui::DragValue::new(value).range(640..=16384));
        }
    });
    ui.checkbox(&mut p.maximized, "Maximized");
    ui.add(egui::Slider::new(&mut p.ui_scale, 0.75..=2.).text("UI scale"));
    ui.label("UI scale combines with Windows monitor DPI. Automatic uses the monitor work area; Remember restores the previous window size.");
    let mut apply = false;
    let mut cancel = false;
    ui.horizontal_wrapped(|ui| {
        apply = ui.button("Apply configuration").clicked();
        cancel = ui.button("Cancel").clicked();
    });
    if cancel {
        app.page = Page::Welcome;
    }
    if apply {
        app.configuration_draft.sanitize();
        let preferences = app.configuration_draft.clone();
        app.window_changed(preferences.clone());
        app.status = "Application configuration applied.".into();
        Some(WindowCommand::Apply(preferences))
    } else {
        None
    }
}

pub(super) fn recent(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Recent Projects");
    if app.recent.entries.is_empty() {
        ui.label("No recent projects. Projects appear here after a successful open or save.");
    }
    let entries = app.recent.entries.clone();
    for (index, entry) in entries.iter().enumerate() {
        ui.push_id(index, |ui| {
            ui.group(|ui| {
                ui.strong(&entry.name);
                ui.monospace(entry.path.to_string_lossy());
                let missing = app.missing.get(index).copied().unwrap_or(true);
                if missing {
                    ui.label("Missing or moved — locate the project or remove this entry.");
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!missing, egui::Button::new("Open"))
                        .clicked()
                    {
                        app.load_project(entry.path.clone(), None);
                    }
                    if ui.button("Locate…").clicked() {
                        app.pick(Some(index));
                    }
                    if ui.button("Remove from Recent Projects").clicked() {
                        app.recent.remove(index);
                        app.refresh_recents();
                    }
                });
            });
        });
        if app.page == Page::Editor || app.picker.is_some() {
            break;
        }
    }
    if ui.button("Refresh Recent Projects").clicked() {
        app.refresh_recents();
    }
}
pub(super) fn new_project(app: &mut App, ui: &mut egui::Ui) {
    super::theme::heading(ui, "New Project", super::theme::BLUE);
    ui.label("Choose a name to begin. Configure your coaster inside the Editor.");
    let name_width = (ui.available_width() - 140.).clamp(80., 420.);
    egui::Grid::new("project setup fields")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Project name");
            ui.add(
                egui::TextEdit::singleline(&mut app.setup.name)
                    .char_limit(256)
                    .desired_width(name_width),
            );
            ui.end_row();
            if let Some((project, path)) = &app.setup.template {
                ui.label("Template");
                ui.label(&project.metadata.name);
                ui.end_row();
                ui.label("Source");
                ui.monospace(path.to_string_lossy());
                ui.end_row();
            }
        });
    if app.setup.template.is_some() {
        ui.label(
            "Create makes a new unsaved copy with its own identity; the template is retained.",
        );
    } else {
        ui.label("Start with an editable 10 m straight and no assigned coaster style. Choose a family and numbered style later in Coaster Properties.");
    }
    ui.label("Create opens the Editor. Save / Save As chooses the project's .qx.json location. Cancel keeps your previous project.");
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !app.setup.name.trim().is_empty(),
                egui::Button::new("Create Project"),
            )
            .clicked()
        {
            app.create();
        }
        if ui.button("Cancel").clicked() {
            app.cancel_setup();
        }
    });
}
pub(super) fn library(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Library");
    ui.horizontal_wrapped(|ui| {
        for (tab, label) in [
            (LibraryTab::Samples, "Sample Projects"),
            (LibraryTab::Templates, "Templates"),
            (LibraryTab::Tracks, "Track Library"),
            (LibraryTab::Trains, "Train Library"),
        ] {
            ui.selectable_value(&mut app.library_tab, tab, label);
        }
        if ui.button("Refresh Library").clicked() {
            app.projects = super::project_library::Projects::discover(&app.library.root);
        }
    });
    for issue in &app.projects.issues {
        ui.label(issue);
    }
    match app.library_tab {
        LibraryTab::Samples | LibraryTab::Templates => {
            let template = app.library_tab == LibraryTab::Templates;
            let paths = if template {
                app.projects.templates.clone()
            } else {
                app.projects.samples.clone()
            };
            if template {
                ui.label("Installed .qx.json templates create new projects through setup.");
            } else {
                ui.label("Existing numerical development fixtures — not finished coaster designs. Opened samples can be saved as a separate project with Save As.");
            }
            if paths.is_empty() {
                ui.label(if template {
                    "No templates installed."
                } else {
                    "No sample projects installed."
                });
            }
            for path in paths {
                ui.horizontal_wrapped(|ui| {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                    if ui
                        .button(if template {
                            "Use Template…"
                        } else {
                            "Open Sample"
                        })
                        .clicked()
                    {
                        if template {
                            app.use_template(path.clone());
                        } else {
                            app.load_project(path.clone(), None);
                        }
                    }
                });
                if app.page != Page::Library {
                    break;
                }
            }
        }
        LibraryTab::Tracks | LibraryTab::Trains => {
            let kind = if app.library_tab == LibraryTab::Tracks {
                qx_style::library::LibraryKind::Track
            } else {
                qx_style::library::LibraryKind::Train
            };
            ui.label("Catalog inspection. Style application remains in Coaster Properties / Train Configuration after opening a project.");
            for entry in app.library.entries.iter().filter(|e| e.kind == kind) {
                ui.collapsing(&entry.style_id, |ui| {
                    ui.label(format!(
                        "{} · {}",
                        entry.status.label(),
                        if entry.content_ready {
                            "Content available"
                        } else {
                            "No installed content"
                        }
                    ));
                    for issue in &entry.issues {
                        ui.label(issue);
                    }
                });
            }
        }
    }
}
