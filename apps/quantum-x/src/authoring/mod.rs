mod commands;
mod construction_ui;
#[cfg(test)]
mod correction_tests;
mod draft_workflow;
mod files;
mod graph;
mod graph_canvas;
#[cfg(test)]
mod graph_drag_tests;
mod graph_gesture;
mod graph_view;
mod icons;
mod layout_ui;
#[cfg(test)]
mod m5_tests;
#[cfg(test)]
mod m6_tests;
mod m7_live;
mod m7_scenario;
#[cfg(test)]
mod m7_tests;
mod panels;
mod plot;
#[cfg(test)]
mod save_tests;
mod scenario;
pub fn track_scenario(output: PathBuf) -> Result<(), Box<dyn Error>> {
    m7_scenario::run(output)
}
mod session;
mod spline;
#[cfg(test)]
mod tests;
mod theme;
mod timeline;
mod workbench;
mod workflow;
use qx_geometry::solver::GenerationOptions;
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use session::{Session, blank_project, scene};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
pub fn inspection_scene(
    p: &qx_document::Project,
    c: &qx_style::Catalog,
) -> Result<InspectionScene, String> {
    scene(p, c, Default::default())
}
pub fn authoring_scenario(output: PathBuf) -> Result<(), Box<dyn Error>> {
    scenario::run(output)
}

pub fn run(path: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    let project = match &path {
        Some(path) => qx_document::persistence::load(path, &catalog)?,
        None => blank_project(),
    };
    let initial_scene = scene(&project, &catalog, GenerationOptions::default())?;
    let mut session = Session::new(project, path.as_deref(), catalog)?;
    (session.workbench, session.preferences) = workbench::PreferenceStore::load();
    qx_rendering::edit(initial_scene, session).map_err(Into::into)
}
pub fn render(
    path: PathBuf,
    output: PathBuf,
    profile: &str,
    width: u32,
    height: u32,
    selection: Option<(u64, &str)>,
) -> Result<(), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    let project = qx_document::persistence::load(&path, &catalog)?;
    let initial_scene = scene(&project, &catalog, GenerationOptions::default())?;
    let mut session = Session::new(project, Some(&path), catalog)?;
    session.workbench.active = match profile {
        "track" => workbench::Profile::Track,
        "train" => workbench::Profile::Train,
        _ => return Err("render workspace must be track or train".into()),
    };
    if let Some((id, channel)) = selection {
        use qx_geometry::authoring::{ChannelKey, SectionId};
        if !session
            .editor
            .project()
            .sections
            .iter()
            .any(|s| s.id.0 == id)
        {
            return Err("render selection refers to an absent section ID".into());
        }
        session.select(SectionId(id));
        session.channel = match channel {
            "roll" => ChannelKey::Roll,
            "normal" => ChannelKey::Normal,
            "lateral" => ChannelKey::Lateral,
            "pitch" => ChannelKey::Pitch,
            "yaw" => ChannelKey::Yaw,
            _ => return Err("render channel must be roll, normal, lateral, pitch or yaw".into()),
        };
        if !qx_document::editing::channel_keys(&session.draft).contains(&session.channel) {
            return Err("selected section does not support this authored channel".into());
        }
        graph_canvas::fit(&mut session);
    }
    println!(
        "{}",
        qx_rendering::render_offscreen(&initial_scene, &mut session, &output, width, height)?
    );
    Ok(())
}
impl EditorSurface for Session {
    fn select_section(&mut self, id: qx_geometry::authoring::SectionId) {
        self.select(id);
    }
    fn inspect_station(&mut self, point: Option<(qx_geometry::authoring::SectionId, f64)>) {
        self.viewport_inspection = point;
    }
    fn window_preferences(&self) -> qx_rendering::WindowPreferences {
        self.workbench.window.clone()
    }
    fn window_changed(&mut self, prefs: qx_rendering::WindowPreferences) {
        self.workbench.window = prefs;
    }
    fn persist_preferences(&mut self, force: bool) {
        self.preferences.save(&self.workbench, force);
    }
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
