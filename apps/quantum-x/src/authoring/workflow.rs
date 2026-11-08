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
            Self::Select(id) => format!("select region {}", id.0),
            Self::Insert { before, .. } => format!(
                "insert {} this region",
                if before { "before" } else { "after" }
            ),
            Self::Move { later } => {
                format!("move region {}", if later { "later" } else { "earlier" })
            }
            Self::Remove => "remove this region".into(),
        }
    }
}
impl Session {
    pub fn region_extent_status(&self) -> String {
        let committed = self
            .editor
            .project()
            .sections
            .iter()
            .find(|r| r.id == self.selected);
        let exact = |r: &qx_geometry::authoring::Section| match r.domain {
            qx_geometry::authoring::Domain::Distance => format!("{} m", r.extent),
            qx_geometry::authoring::Domain::Time => format!("{} s", r.extent),
            _ => "normalized curve".into(),
        };
        format!(
            "Preview{}: {} · Committed: {}",
            if self.draft_dirty() {
                " (unapplied draft)"
            } else {
                ""
            },
            exact(&self.draft),
            committed.map_or_else(|| "unavailable".into(), exact)
        )
    }
    pub fn defer_section_action(&mut self, action: PendingAction) -> bool {
        if self.draft_dirty() || self.graph_drag.is_some() {
            self.pending_action = Some(action);
            self.status = format!(
                "Apply or cancel the draft on region {} before you {}.",
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
            "Region draft canceled; other settings remain staged."
        } else {
            "Region draft canceled; committed geometry restored."
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
    pub fn keep_editing(&mut self) {
        self.pending_action = None;
        self.status = "Region draft retained; keep editing.".into();
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
    if session.graph_drag.is_none()
        && let Some(error) = &session.draft_error
    {
        ui.colored_label(egui::Color32::from_rgb(240, 195, 100), error);
    }
}

/// Resolve navigation at a focused boundary, independent of dock/status layout.
pub(super) fn resolution_dialog(session: &mut Session, ctx: &egui::Context) {
    let focus_id = egui::Id::unique("region draft dialog focus");
    let Some(action) = session.pending_action else {
        ctx.data_mut(|d| d.remove::<bool>(focus_id));
        return;
    };
    let first = ctx.data_mut(|d| {
        let first = !d.get_temp::<bool>(focus_id).unwrap_or(false);
        d.insert_temp(focus_id, true);
        first
    });
    // egui treats Enter as a focused-button click even with Ctrl held. The
    // editor's Apply shortcut must not activate a confirmation choice.
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter));
    let response = egui::Modal::new(egui::Id::unique("resolve region draft")).show(ctx, |ui| {
        ui.set_max_width(480.);
        ui.heading("Unapplied Region draft");
        ui.label(format!(
            "Region {} has unapplied edits. Before you {}:",
            session.selected.0,
            action.label()
        ));
        ui.label("Apply commits this Region draft; Discard Draft discards only this Region draft. Either continues the requested action. Other staged settings are retained.");
        ui.label(session.region_extent_status());
        if let Some(error) = &session.draft_error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.add_enabled_ui(session.graph_drag.is_none(), |ui| {
            if ui.button("Apply & Continue").clicked() {
                session.resolve_pending(true);
            }
            if ui.button("Discard Draft & Continue").clicked() {
                session.resolve_pending(false);
            }
        });
        let keep = ui.button("Keep Editing");
        if first {
            keep.request_focus();
        }
        if keep.clicked() {
            session.keep_editing();
        }
    });
    // Escape and clicking the backdrop are non-destructive dismissals.
    if session.pending_action.is_some() && response.should_close() {
        session.keep_editing();
    }
    if session.pending_action.is_none() {
        ctx.data_mut(|d| d.remove::<bool>(focus_id));
    }
}
