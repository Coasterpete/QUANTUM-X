//! Desktop command shell; all actions dispatch existing document, view or layout services.
use super::{
    files::FileAction,
    icons::{self, Icon},
    session::Session,
    workbench::{Pane, Profile},
};
use qx_rendering::{DisplayOptions, ViewCommand, WindowCommand};

pub(super) type HeaderResult = (
    Option<WindowCommand>,
    Option<(Pane, bool)>,
    Option<ViewCommand>,
);
pub(super) fn status_height(session: &Session) -> f32 {
    session.graph_drag.as_ref().map_or_else(
        || {
            48. + if session.pending_action.is_some() {
                62.
            } else {
                0.
            } + if session.draft_error.is_some() {
                66.
            } else {
                0.
            } + if session.preferences.error.is_some() {
                32.
            } else {
                0.
            }
        },
        |drag| drag.status_height,
    )
}
pub(super) fn show(
    session: &mut Session,
    ctx: &egui::Context,
    display: &mut DisplayOptions,
) -> HeaderResult {
    let (mut window, mut floating, mut view) = (None, None, None);
    ctx.root_ui(|root| {
        egui::Panel::top("workbench commands").show(root, |ui| {
            ui.add_enabled_ui(session.file_dialog.is_none(), |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if icons::button(ui, Icon::New, "New project   Ctrl+N", true, "Create an untitled project").clicked() { session.open(None, false); ui.close(); }
                        if icons::button(ui, Icon::Open, "Open…   Ctrl+O", true, "Choose an existing project").clicked() { session.file_dialog(FileAction::Open); ui.close(); }
                        ui.separator();
                        if icons::button(ui, Icon::Save, "Save   Ctrl+S", true, "Save to current file, or choose a file for an untitled project").clicked() { session.save(); ui.close(); }
                        if icons::button(ui, Icon::SaveAs, "Save As…   Ctrl+Shift+S", true, "Save to another project file").clicked() { session.file_dialog(FileAction::SaveAs); ui.close(); }
                        ui.separator();
                        ui.label("Current file"); ui.monospace(if session.path.is_empty() { "Untitled · no file yet" } else { &session.path });
                    });
                    ui.menu_button("Edit", |ui| {
                        if icons::button(ui, Icon::Undo, "Undo   Ctrl+Z", session.editor.can_undo() && !session.has_drafts(), "Undo last document transaction").clicked() { session.undo(false); ui.close(); }
                        if icons::button(ui, Icon::Redo, "Redo   Ctrl+Y", session.editor.can_redo() && !session.has_drafts(), "Redo last document transaction").clicked() { session.undo(true); ui.close(); }
                        ui.separator();
                        if icons::button(ui, Icon::Apply, "Apply section   Ctrl+Enter", session.draft_dirty(), "Commit selected section draft").clicked() { session.apply_draft(); ui.close(); }
                        if icons::button(ui, Icon::Cancel, "Cancel section draft   Esc", session.draft_dirty(), "Restore committed section").clicked() { session.cancel_section(); ui.close(); }
                    });
                    ui.menu_button("View", |ui| {
                        if icons::button(ui, Icon::FitAll, "Fit All   F over viewport", true, "Fit all generated track references").clicked() { view = Some(ViewCommand::FitAll); ui.close(); }
                        if icons::button(ui, Icon::FitSelection, "Fit Selection", true, "Fit selected section").clicked() { view = Some(ViewCommand::FitSelected); ui.close(); }
                        for (label, command) in [("Perspective", ViewCommand::Perspective), ("Top", ViewCommand::Top), ("Side", ViewCommand::Side), ("Front", ViewCommand::Front), ("Toggle orthographic", ViewCommand::ToggleOrthographic)] {
                            if ui.button(label).clicked() { view = Some(command); ui.close(); }
                        }
                        ui.separator();
                        ui.checkbox(&mut display.grid, "Ground reference grid");
                        ui.checkbox(&mut display.frames, "Diagnostic orientation frames");
                        if ui.button("View and overlay settings").clicked() { session.workbench.layouts[0].dock(Pane::View); session.workbench.active = Profile::Track; ui.close(); }
                        if ui.button("Generation diagnostics").clicked() { session.workbench.layouts[0].dock(Pane::Diagnostics); session.workbench.active = Profile::Track; ui.close(); }
                    });
                    ui.menu_button("Preferences", |ui| {
                        if icons::button(ui, Icon::Preferences, "Display and UI scale…", true, "Window sizing, maximized state and independent UI scaling").clicked() {
                            session.window_draft = session.workbench.window.clone(); session.display_dialog = true; ui.close();
                        }
                    });
                    ui.menu_button("Window / Layout", |ui| {
                        ui.selectable_value(&mut session.workbench.active, Profile::Track, "Track Workspace   Ctrl+1");
                        ui.selectable_value(&mut session.workbench.active, Profile::Train, "Train Workspace   Ctrl+2");
                        ui.separator();
                        if ui.button("Reset current workspace layout").clicked() { session.workbench.reset_layout(); ui.close(); }
                        if ui.button("Reset Window/Layout").clicked() { session.workbench = Default::default(); session.preferences.allow_reset(); window = Some(WindowCommand::Reset); ui.close(); }
                        ui.separator();
                        let layout = &session.workbench.layouts[session.workbench.active.index()];
                        for pane in session.workbench.active.panes().iter().copied().filter(|p| !p.is_viewport()) {
                            let is_floating = layout.floating.iter().any(|f| f.pane == pane);
                            if ui.button(format!("{} {}", if is_floating { "Dock" } else { "Float" }, pane.title())).clicked() { floating = Some((pane, !is_floating)); ui.close(); }
                        }
                        ui.small("Drag tabs to dock/group panes; right-click a tab to float it inside this window.");
                    });
                });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if icons::button(ui, Icon::New, "New", true, "New project · Ctrl+N").clicked() { session.open(None, false); }
                    if icons::button(ui, Icon::Open, "Open", true, "Open project · Ctrl+O").clicked() { session.file_dialog(FileAction::Open); }
                    if icons::button(ui, Icon::Save, "Save", true, "Save project · Ctrl+S").clicked() { session.save(); }
                    ui.separator();
                    if icons::button(ui, Icon::Undo, "Undo", session.editor.can_undo() && !session.has_drafts(), "Undo · Ctrl+Z. Apply/Cancel pending drafts first.").clicked() { session.undo(false); }
                    if icons::button(ui, Icon::Redo, "Redo", session.editor.can_redo() && !session.has_drafts(), "Redo · Ctrl+Y. Apply/Cancel pending drafts first.").clicked() { session.undo(true); }
                    ui.separator();
                    ui.selectable_value(&mut session.workbench.active, Profile::Track, "Track · Ctrl+1");
                    ui.selectable_value(&mut session.workbench.active, Profile::Train, "Train · Ctrl+2");
                    ui.separator();
                    ui.add_sized([(ui.available_size_before_wrap().x - 110.).clamp(80., 400.), 20.], egui::Label::new(egui::RichText::new(&session.editor.project().metadata.name).strong()).truncate().halign(egui::Align::Min)).on_hover_text(if session.path.is_empty() { "Untitled project" } else { &session.path });
                    ui.small(if session.pending_edits() { "Unsaved edits" } else { "Saved" });
                });
            });
        });
        // Generation progress has a reserved line: publishing a preview cannot
        // resize the dock and move a graph handle under the held pointer.
        egui::Panel::bottom("project status").exact_size(status_height(session)).show(root, |ui| {
            super::workflow::resolution(session, ui);
            ui.add(egui::Label::new(&session.status).truncate()).on_hover_text(&session.status);
            let readout = session.graph_readout.take();
            if let Some(message) = &session.graph_feedback {
                ui.add(egui::Label::new(message).truncate()).on_hover_text(message);
            } else if let Some(message) = readout {
                ui.add(egui::Label::new(egui::RichText::new(&message).monospace()).truncate()).on_hover_text(message);
            } else if session.graph_drag.is_some() && let Some(error) = &session.draft_error {
                ui.add(egui::Label::new(error).truncate()).on_hover_text(error);
            } else if session.visible_revision != session.revision && session.draft_error.is_none() { ui.colored_label(egui::Color32::from_rgb(240, 195, 100), "Viewport awaiting current document/draft revision."); }
            if let Some(error) = &session.preferences.error { ui.colored_label(egui::Color32::from_rgb(240, 195, 100), error); }
        });
    });
    (window, floating, view)
}
