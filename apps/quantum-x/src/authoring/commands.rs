//! Desktop command shell; all actions dispatch existing document, view or layout services.
use super::{
    draft_workflow::UiAction,
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
            } + if session.save_failure.is_some() {
                24.
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
            ui.add_enabled_ui(session.file_dialog.is_none() && !session.project_prompt_open() && session.graph_drag.is_none(), |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button(egui::RichText::new("File").strong().color(super::theme::TRACK), |ui| {
                        if icons::button(ui, Icon::New, "New project…   Ctrl+N", true, "Set up a project before creating it").clicked() { session.ui_action = Some(UiAction::New); ui.close(); }
                        if icons::button(ui, Icon::Open, "Open…   Ctrl+O", true, "Choose an existing project").clicked() { session.ui_action = Some(UiAction::Open); ui.close(); }
                        ui.separator();
                        if icons::button(ui, Icon::Save, "Save   Ctrl+S", true, "Save to current file, or choose a file for an untitled project").clicked() { session.ui_action = Some(UiAction::Save); ui.close(); }
                        if icons::button(ui, Icon::SaveAs, "Save As…   Ctrl+Shift+S", true, "Save to another project file").clicked() { session.ui_action = Some(UiAction::SaveAs); ui.close(); }
                        ui.separator();
                        if ui.button("Return to Welcome Screen…").clicked() { session.ui_action = Some(UiAction::Welcome); ui.close(); }
                        ui.separator();
                        ui.label("Current file"); ui.monospace(if session.path.is_empty() { "Untitled · no file yet" } else { &session.path });
                    });
                    ui.menu_button(egui::RichText::new("Edit").strong().color(super::theme::GREEN), |ui| {
                        if ui.button("Coaster Properties").clicked() { super::coaster_properties::open(session); ui.close(); }
                        ui.separator();
                        if icons::button(ui, Icon::Undo, "Undo   Ctrl+Z", session.editor.can_undo() || session.has_drafts(), "Undo last document transaction").clicked() { session.ui_action = Some(UiAction::History(false)); ui.close(); }
                        if icons::button(ui, Icon::Redo, "Redo   Ctrl+Y", session.editor.can_redo() || session.has_drafts(), "Redo last document transaction").clicked() { session.ui_action = Some(UiAction::History(true)); ui.close(); }
                        ui.separator();
                        if icons::button(ui, Icon::Apply, "Apply section   Ctrl+Enter", session.draft_dirty(), "Commit selected section draft").clicked() { session.apply_draft(); ui.close(); }
                        if icons::button(ui, Icon::Cancel, "Cancel section draft   Esc", session.draft_dirty(), "Restore committed section").clicked() { session.cancel_section(); ui.close(); }
                    });
                    ui.menu_button(egui::RichText::new("View").strong().color(super::theme::VIOLET), |ui| {
                        ui.menu_button("Track display",|ui|{for mode in [qx_rendering::DisplayMode::Rendered,qx_rendering::DisplayMode::RenderedCenterline,qx_rendering::DisplayMode::Technical,qx_rendering::DisplayMode::Wireframe] {ui.selectable_value(&mut display.mode,mode,mode.label());}});
                        if icons::button(ui, Icon::FitAll, "Fit All   F over viewport", true, "Fit all generated track references").clicked() { view = Some(ViewCommand::FitAll); ui.close(); }
                        if icons::button(ui, Icon::FitSelection, "Fit Selection", true, "Fit selected section").clicked() { view = Some(ViewCommand::FitSelected); ui.close(); }
                        for (label, command) in [("Perspective", ViewCommand::Perspective), ("Top", ViewCommand::Top), ("Side", ViewCommand::Side), ("Front", ViewCommand::Front), ("Toggle orthographic", ViewCommand::ToggleOrthographic)] {
                            if ui.button(label).clicked() { view = Some(command); ui.close(); }
                        }
                        ui.separator();
                        ui.checkbox(&mut display.grid, "Ground reference grid");
                        if ui.checkbox(&mut session.development, "Development tools").on_hover_text("Show experimental train tools, test fixtures and solver/import diagnostics. Session only.").changed() { ui.close(); }
                        if ui.button("View and overlay settings").clicked() { session.workbench.layouts[0].dock(Pane::View); session.workbench.active = Profile::Track; ui.close(); }
                        if ui.button("Generation diagnostics").clicked() { session.workbench.layouts[0].dock(Pane::Diagnostics); session.workbench.active = Profile::Track; ui.close(); }
                    });
                    ui.menu_button("Info", |ui| {
                        let product = qx_rendering::identity::product();
                        ui.strong(&product.name);
                        ui.label(format!("Version {} · development build", env!("CARGO_PKG_VERSION")));
                        ui.label("Independent roller coaster design and simulation");
                        ui.small(format!("Development repository: {}", product.repository));
                        ui.small("Current workbench presentation is provisional; catalog entries do not certify dynamics.");
                    });
                    ui.menu_button(egui::RichText::new("Preferences").strong().color(super::theme::RED), |ui| {
                        if icons::button(ui, Icon::Preferences, "Display and UI scale…", true, "Window sizing, maximized state and independent UI scaling").clicked() {
                            session.window_draft = session.workbench.window.clone(); session.display_dialog = true; ui.close();
                        }
                    });
                    ui.menu_button(if ui.available_width() < 300. { "Layout" } else { "Window / Layout" }, |ui| {
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
                    let compact = ui.available_width() < 1100.;
                    let tiny = ui.available_width() < 700.;
                    if !compact { ui.label(egui::RichText::new("COMMAND").strong().color(super::theme::GREEN)); }
                    if icons::button(ui, Icon::New, if tiny { "" } else { "New" }, true, "New project · Ctrl+N").clicked() { session.ui_action = Some(UiAction::New); }
                    if icons::button(ui, Icon::Open, if tiny { "" } else { "Open" }, true, "Open project · Ctrl+O").clicked() { session.ui_action = Some(UiAction::Open); }
                    if icons::button(ui, Icon::Save, if tiny { "" } else { "Save" }, true, "Save project · Ctrl+S").clicked() { session.ui_action = Some(UiAction::Save); }
                    ui.separator();
                    if icons::button(ui, Icon::Undo, if compact { "" } else { "Undo" }, session.editor.can_undo() || session.has_drafts(), "Undo · Ctrl+Z. Apply/Cancel pending drafts first.").clicked() { session.ui_action = Some(UiAction::History(false)); }
                    if icons::button(ui, Icon::Redo, if compact { "" } else { "Redo" }, session.editor.can_redo() || session.has_drafts(), "Redo · Ctrl+Y. Apply/Cancel pending drafts first.").clicked() { session.ui_action = Some(UiAction::History(true)); }
                    ui.separator();
                    ui.selectable_value(&mut session.workbench.active, Profile::Track, egui::RichText::new(if compact { "Track" } else { "Track · Ctrl+1" }).color(super::theme::TRACK)).on_hover_text("Track Workspace · Ctrl+1");
                    ui.selectable_value(&mut session.workbench.active, Profile::Train, egui::RichText::new(if compact { "Train" } else { "Train · Ctrl+2" }).color(super::theme::TRAIN)).on_hover_text("Train Workspace · Ctrl+2");
                    ui.separator();
                    if !compact { ui.add_sized([(ui.available_size_before_wrap().x - 140.).clamp(80., 320.), 28.], egui::Label::new(egui::RichText::new(&session.editor.project().metadata.name).strong()).truncate().halign(egui::Align::Min)).on_hover_text(if session.path.is_empty() { "Untitled project" } else { &session.path }); }
                    if !tiny { ui.small(session.save_state()).on_hover_text(session.save_failure.as_deref().unwrap_or("Drafts are previews; only committed changes are written to disk.")); }
                    if (!compact || session.has_drafts()) && ui.add_enabled(session.has_drafts(), egui::Button::new("Drafts…")).on_hover_text(session.draft_names().join("\n")).clicked() {
                        session.request_drafts(super::draft_workflow::DraftRequest::Review);
                    }
                });
            });
        });
        // Generation progress has a reserved line: publishing a preview cannot
        // resize the dock and move a graph handle under the held pointer.
        egui::Panel::bottom("project status").exact_size(status_height(session)).show(root, |ui| {
            super::workflow::resolution(session, ui);
            if let Some(error) = &session.save_failure {
                ui.add(egui::Label::new(egui::RichText::new(error).color(egui::Color32::LIGHT_RED)).truncate()).on_hover_text(error);
            }
            let status = if ui.available_width() < 700. { format!("{} · {}", session.save_state(), session.status) } else { session.status.clone() };
            ui.add(egui::Label::new(&status).truncate()).on_hover_text(&status);
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
