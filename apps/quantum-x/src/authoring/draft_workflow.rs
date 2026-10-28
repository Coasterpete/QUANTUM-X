//! Draft resolution at file/history boundaries. No draft is implicitly discarded.
use super::{files::FileAction, session::Session, workbench::Pane};
use qx_document::command::Command;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DraftRequest {
    Review,
    Save,
    SaveAs,
    SaveTo(PathBuf),
    History { redo: bool },
}

#[derive(Clone, Copy)]
pub(super) enum UiAction {
    Save,
    SaveAs,
    Open,
    New,
    History(bool),
}

impl Session {
    pub fn run_ui_action(&mut self) {
        if let Some(action) = self.ui_action.take() {
            match action {
                UiAction::Save => self.save(),
                UiAction::SaveAs => self.file_dialog(FileAction::SaveAs),
                UiAction::Open => self.file_dialog(FileAction::Open),
                UiAction::New => self.open(None, false),
                UiAction::History(redo) => self.undo(redo),
            }
        }
    }

    pub fn project_prompt_open(&self) -> bool {
        self.draft_request.is_some()
            || self.confirm_replace.is_some()
            || self.open_dialog_requested
            || self.close_requested
    }
    pub fn request_drafts(&mut self, request: DraftRequest) {
        self.draft_request = Some(request);
        self.resolution_error = None;
    }

    pub fn draft_names(&self) -> Vec<String> {
        let p = self.editor.project();
        let mut names = Vec::new();
        if self.draft_dirty() {
            names.push(format!(
                "Section/graph {} — {} (Track Workspace / Transition Editor)",
                self.selected.0, self.draft.name
            ));
        }
        if self.construction_draft != p.construction {
            names.push(
                "Track construction — strategy, dimensions, materials (Track Workspace / Coaster)"
                    .into(),
            );
        }
        if self.initial_draft != p.initial {
            names.push("Initial conditions (Track Workspace / Project)".into());
        }
        if self.metadata_draft != p.metadata {
            names.push("Project metadata (Track Workspace / Project)".into());
        }
        if self.heartline_draft != p.heartline {
            names.push(
                "Reference settings (Track Workspace / Reference; Train Workspace / Configuration)"
                    .into(),
            );
        }
        if self.assets.draft_dirty(p) {
            names.push("Train visual correction (Train Workspace / Configuration)".into());
        }
        names
    }

    pub fn pane_title(&self, pane: Pane) -> String {
        let pending = match pane {
            Pane::Track => self.has_drafts(),
            Pane::Transition => self.draft_dirty(),
            Pane::Configuration => {
                self.heartline_draft != self.editor.project().heartline
                    || self.assets.draft_dirty(self.editor.project())
            }
            _ => false,
        };
        format!(
            "{}{}",
            pane.title(),
            if pending { " · unapplied edits" } else { "" }
        )
    }

    pub fn save_state(&self) -> &'static str {
        if self.save_failure.is_some() {
            "Save failed"
        } else if self.has_drafts() && self.dirty() {
            "Unapplied drafts + unsaved changes"
        } else if self.has_drafts() {
            "Unapplied drafts"
        } else if self.dirty() {
            "Unsaved changes"
        } else {
            "Saved"
        }
    }

    /// Validate the complete candidate before the single existing transaction.
    /// A bad draft leaves every draft, the document and both history stacks intact.
    pub fn apply_all_drafts(&mut self) -> bool {
        if self.graph_drag.is_some() {
            self.resolution_error =
                Some("Finish the active graph gesture before applying drafts.".into());
            return false;
        }
        if !self.has_drafts() {
            return true;
        }
        let mut candidate = self.preview_project();
        if self.assets.draft_dirty(self.editor.project())
            && let Some(a) = &self.assets.draft
            && (!super::train_assets::renderable(a) || !self.assets.library.contains_key(&a.path))
        {
            self.resolution_error=Some("Train visual correction exceeds renderer limits or its GLB is not loaded. Revise it or import the missing asset. No drafts applied.".into());
            return false;
        }
        candidate.metadata = self.metadata_draft.clone();
        let validation = candidate
            .validate(&self.catalog)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                super::session::scene(&candidate, &self.catalog, self.options).map(|_| ())
            });
        if let Err(error) = validation {
            self.resolution_error = Some(format!(
                "Draft validation failed: {error}. No drafts applied; no file written. Revise the listed panels or keep editing."
            ));
            return false;
        }
        let p = self.editor.project();
        let mut commands = Vec::new();
        if self.draft_dirty() {
            commands.push(Command::ReplaceSection {
                section: self.draft.clone(),
            });
        }
        if candidate.construction != p.construction {
            commands.push(Command::SetConstruction(candidate.construction));
        }
        if candidate.initial != p.initial {
            commands.push(Command::SetInitial(candidate.initial));
        }
        if candidate.metadata != p.metadata {
            commands.push(Command::SetMetadata(candidate.metadata));
        }
        if candidate.heartline != p.heartline {
            commands.push(Command::SetHeartline(candidate.heartline));
        }
        if candidate.trains != p.trains {
            commands.push(Command::SetTrains(candidate.trains));
        }
        if !self.execute(commands) {
            self.resolution_error = Some(self.status.clone());
            return false;
        }
        self.pending_action = None;
        self.assets.draft = None;
        self.resolution_error = None;
        true
    }

    pub fn cancel_all_drafts(&mut self) {
        self.assets.draft = None;
        self.graph_drag = None;
        self.sync_draft();
        self.construction_draft = self.editor.project().construction.clone();
        self.initial_draft = self.editor.project().initial.clone();
        self.metadata_draft = self.editor.project().metadata.clone();
        self.heartline_draft = self.editor.project().heartline.clone();
        self.pending_action = None;
        self.resolution_error = None;
        self.graph_feedback = None;
        self.draft_error = None;
        self.refresh_view();
    }

    pub fn resolve_drafts(&mut self, apply: bool) {
        let Some(request) = self.draft_request.clone() else {
            return;
        };
        if self.graph_drag.is_some() {
            return;
        }
        if apply {
            if !self.apply_all_drafts() {
                return;
            }
        } else {
            // Save never offers a discard-and-save path.
            if !matches!(request, DraftRequest::History { .. } | DraftRequest::Review) {
                return;
            }
            self.cancel_all_drafts();
        }
        self.draft_request = None;
        match request {
            DraftRequest::Save => self.save(),
            DraftRequest::SaveAs => self.file_dialog(FileAction::SaveAs),
            DraftRequest::SaveTo(path) => self.save_to(&path),
            DraftRequest::History { redo } if !apply => self.undo(redo),
            DraftRequest::History { .. } => {
                self.status = "Drafts applied as one transaction. Choose Undo to undo that transaction. Applying changes clears Redo history.".into();
            }
            _ => {}
        }
    }
}

pub(super) fn dialog(session: &mut Session, ctx: &egui::Context) {
    let Some(request) = session.draft_request.clone() else {
        return;
    };
    let saving = matches!(
        request,
        DraftRequest::Save | DraftRequest::SaveAs | DraftRequest::SaveTo(_)
    );
    let history = matches!(request, DraftRequest::History { .. });
    egui::Modal::new(egui::Id::unique("resolve project drafts")).show(ctx, |ui| {
        ui.set_max_width(590.);
        ui.heading(if saving { "Unapplied edits before saving" } else { "Resolve unapplied edits" });
        ui.label("These panels contain changes that are not yet in the committed project:");
        for name in session.draft_names() { ui.label(format!("• {name}")); }
        if history {
            ui.label("Undo/Redo operates on committed transactions. Apply drafts as one transaction (then choose history again), or explicitly cancel them and continue. Applying drafts clears Redo history.");
        } else if saving {
            ui.label("Validate and apply all drafts as one transaction, then save the complete project. If any draft is invalid, nothing is applied or saved.");
        }
        if let Some(error) = &session.resolution_error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
        ui.add_enabled_ui(session.graph_drag.is_none(), |ui| {
            if ui.button(if saving { "Apply All & Save" } else { "Apply all drafts" }).clicked() { session.resolve_drafts(true); }
            if !saving && ui.button(if history { "Cancel drafts & continue history" } else { "Cancel all drafts" }).clicked() { session.resolve_drafts(false); }
            if ui.button("Keep editing / Cancel").clicked() {
                session.draft_request = None;
                session.resolution_error = None;
            }
        });
    });
}
