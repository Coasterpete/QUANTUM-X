mod graph;
mod panels;
mod session;
mod spline;
#[cfg(test)]
mod tests;
use qx_geometry::solver::GenerationOptions;
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use session::{Session, blank_project, scene};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

pub fn run(path: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    let project = match &path {
        Some(path) => qx_document::persistence::load(path, &catalog)?,
        None => blank_project(),
    };
    let initial_scene = scene(&project, &catalog, GenerationOptions::default())?;
    let session = Session::new(project, path.as_deref(), catalog)?;
    qx_rendering::edit(initial_scene, session).map_err(Into::into)
}
pub fn render(path: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    let project = qx_document::persistence::load(&path, &catalog)?;
    let initial_scene = scene(&project, &catalog, GenerationOptions::default())?;
    let mut session = Session::new(project, Some(&path), catalog)?;
    println!(
        "{}",
        qx_rendering::render_offscreen(&initial_scene, &mut session, &output, 1440, 960)?
    );
    Ok(())
}
impl EditorSurface for Session {
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        panels::workspace(self, ctx, scene, display)
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        self.poll()
    }
    fn open_path(&mut self, path: &Path) {
        self.open(Some(path.to_path_buf()), false);
    }
    fn request_close(&mut self) -> bool {
        if self.pending_edits() {
            self.close_requested = true;
            self.status =
                "Unsaved edits: save or explicitly discard using the close prompt.".into();
            false
        } else {
            true
        }
    }
}
