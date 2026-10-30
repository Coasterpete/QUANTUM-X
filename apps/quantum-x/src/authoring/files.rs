//! Native dialogs are single-flight workers; document replacement remains guarded by Session.
use super::session::Session;
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Copy)]
pub(super) enum FileAction {
    Open,
    SaveAs,
    ImportGlb,
}
pub(super) struct Dialog {
    pub action: FileAction,
    pub result: mpsc::Receiver<Option<PathBuf>>,
}
/// Shared native project picker for the welcome shell and the editor. Only the
/// caller decides when a validated result may replace a document.
pub(super) fn project_picker(current: PathBuf) -> Result<mpsc::Receiver<Option<PathBuf>>, String> {
    #[cfg(windows)]
    {
        let (send, result) = mpsc::channel();
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new()
                .add_filter("QUANTUM Dynamics project (*.qx.json)", &["qx.json"])
                .add_filter("JSON project (*.json)", &["json"])
                .set_title("Open QUANTUM Dynamics project");
            if let Some(parent) = current.parent().filter(|p| !p.as_os_str().is_empty()) {
                dialog = dialog.set_directory(parent);
            }
            let _ = send.send(dialog.pick_file());
        });
        Ok(result)
    }
    #[cfg(not(windows))]
    {
        let _ = current;
        Err("Native project dialogs currently require Windows.".into())
    }
}
impl Session {
    pub fn file_dialog(&mut self, action: FileAction) {
        if self.file_dialog.is_some() {
            return;
        }
        if matches!(action, FileAction::SaveAs) && self.has_drafts() {
            self.request_drafts(super::draft_workflow::DraftRequest::SaveAs);
            return;
        }
        if matches!(action, FileAction::Open) && self.pending_edits() && !self.open_dialog_confirmed
        {
            self.open_dialog_requested = true;
            return;
        }
        if matches!(action, FileAction::Open) {
            match project_picker(PathBuf::from(&self.path)) {
                Ok(result) => self.file_dialog = Some(Dialog { action, result }),
                Err(error) => {
                    self.open_dialog_confirmed = false;
                    self.status = error;
                }
            }
            return;
        }
        #[cfg(windows)]
        {
            let current = PathBuf::from(&self.path);
            let (send, result) = mpsc::channel();
            self.file_dialog = Some(Dialog { action, result });
            std::thread::spawn(move || {
                let mut dialog = if matches!(action, FileAction::ImportGlb) {
                    rfd::FileDialog::new().add_filter("Binary glTF 2.0 (*.glb)", &["glb"])
                } else {
                    rfd::FileDialog::new()
                        .add_filter("QUANTUM Dynamics project (*.qx.json)", &["qx.json"])
                        .add_filter("JSON project (*.json)", &["json"])
                };
                if let Some(parent) = current.parent().filter(|p| !p.as_os_str().is_empty()) {
                    dialog = dialog.set_directory(parent);
                }
                let path = match action {
                    FileAction::ImportGlb => {
                        dialog.set_title("Import static train GLB").pick_file()
                    }
                    FileAction::Open => dialog
                        .set_title("Open QUANTUM Dynamics project")
                        .pick_file(),
                    FileAction::SaveAs => dialog
                        .set_title("Save QUANTUM Dynamics project as")
                        .set_file_name(
                            current
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Untitled.qx.json"),
                        )
                        .save_file(),
                };
                let _ = send.send(path);
            });
        }
        #[cfg(not(windows))]
        {
            self.status = "Native project dialogs currently require Windows. Use edit <project> on this platform.".into();
        }
    }
    pub fn poll_file_dialog(&mut self, ctx: &egui::Context) {
        let result = self.file_dialog.as_ref().map(|d| d.result.try_recv());
        match result {
            Some(Ok(path)) => {
                let action = self.file_dialog.take().unwrap().action;
                let confirmed = std::mem::take(&mut self.open_dialog_confirmed);
                match path {
                    Some(path) => match action {
                        FileAction::Open => self.open(Some(path), confirmed),
                        FileAction::SaveAs => self.save_to(&path),
                        FileAction::ImportGlb => self.assets.start_import(path),
                    },
                    None => {
                        self.startup_after_save = false;
                        self.status =
                            "File dialog closed without selecting a file; project retained.".into()
                    }
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.file_dialog = None;
                self.open_dialog_confirmed = false;
                self.startup_after_save = false;
                self.status =
                    "File dialog stopped; project retained. Try the command again.".into();
            }
            _ => {}
        }
        if self.file_dialog.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
