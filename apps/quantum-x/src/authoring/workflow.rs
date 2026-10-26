//! Explicit draft resolution is shared by every section navigation surface.
use super::session::Session;
use qx_geometry::authoring::SectionId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PendingAction {
    Select(SectionId),
    Insert { kind: usize, before: bool },
    Move { later: bool },
    Remove,
}
impl PendingAction {
    pub fn label(self) -> String {
        match self {
            Self::Select(id) => format!("select section {}", id.0),
            Self::Insert { before, .. } => format!(
                "insert {} this section",
                if before { "before" } else { "after" }
            ),
            Self::Move { later } => {
                format!("move section {}", if later { "later" } else { "earlier" })
            }
            Self::Remove => "remove this section".into(),
        }
    }
}
impl Session {
    pub fn defer_section_action(&mut self, action: PendingAction) -> bool {
        if self.draft_dirty() || self.graph_drag.is_some() {
            self.pending_action = Some(action);
            self.status = format!(
                "Apply or cancel the draft on section {} before you {}.",
                self.selected.0,
                action.label()
            );
            true
        } else {
            false
        }
    }
    pub fn cancel_section(&mut self) {
        self.graph_drag = None;
        self.graph_feedback = None;
        self.pending_action = None;
        self.sync_draft();
        self.draft_error = None;
        self.refresh_view();
        self.status = if self.has_drafts() {
            "Section draft canceled; other settings remain staged."
        } else {
            "Section draft canceled; committed geometry restored."
        }
        .into();
    }
    pub fn resolve_pending(&mut self, apply: bool) {
        if self.graph_drag.is_some() {
            return;
        }
        let Some(action) = self.pending_action else {
            return;
        };
        if apply {
            if !self.apply_draft() {
                return;
            }
        } else {
            self.cancel_section();
        }
        self.pending_action = None;
        match action {
            PendingAction::Select(id) => self.select(id),
            PendingAction::Insert { kind, before } => {
                super::panels::insert_section(self, kind, before)
            }
            PendingAction::Move { later } => super::timeline::move_selected(self, later),
            PendingAction::Remove => self.remove_selected(),
        }
    }
    pub fn remove_selected(&mut self) {
        if !self.defer_section_action(PendingAction::Remove) {
            self.execute(vec![qx_document::command::Command::RemoveSection {
                id: self.selected,
            }]);
        }
    }
}
pub(super) fn resolution(session: &mut Session, ui: &mut egui::Ui) {
    if let Some(action) = session.pending_action {
        ui.strong(format!(
            "Draft on section {} — before you {}:",
            session.selected.0,
            action.label()
        ));
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(session.graph_drag.is_none(), |ui| {
                if ui.button("Apply & continue").clicked() {
                    session.resolve_pending(true);
                }
                if ui.button("Cancel draft & continue").clicked() {
                    session.resolve_pending(false);
                }
                if ui.button("Keep editing").clicked() {
                    session.pending_action = None;
                }
            });
        });
    }
    if session.graph_drag.is_none()
        && let Some(error) = &session.draft_error
    {
        ui.colored_label(egui::Color32::from_rgb(240, 195, 100), error);
    }
}
