pub(crate) mod box_spined_scenario;
#[cfg(test)]
mod box_spined_tests;
mod coaster_properties;
mod color_ui;
mod commands;
mod construction_ui;
#[cfg(test)]
mod correction_tests;
mod device_curve_ui;
mod draft_workflow;
mod engineering_ui;
mod expansion;
pub(crate) mod expansion_scenario;
#[cfg(test)]
mod expansion_tests;
mod files;
mod graph;
mod graph_canvas;
mod graph_curve;
mod graph_domain;
#[cfg(test)]
mod graph_drag_tests;
mod graph_gesture;
mod graph_precision;
#[cfg(test)]
mod graph_precision_tests;
mod graph_regions;
mod graph_view;
mod icons;
mod layout_ui;
#[cfg(test)]
mod library_tests;
pub(crate) mod library_ui;
pub(crate) mod m4j_scenario;
#[cfg(test)]
mod m5_tests;
#[cfg(test)]
mod m6_tests;
mod m7_live;
mod m7_scenario;
#[cfg(test)]
mod m7_tests;
mod m81_scenario;
#[cfg(test)]
mod m81_tests;
mod m82_scenario;
#[cfg(test)]
mod m82_tests;
pub(crate) mod material_editor_scenario;
pub(crate) mod material_scenario;
#[cfg(test)]
mod material_tests;
mod material_ui;
mod operations_ui;
mod panels;
mod plot;
mod previews;
#[cfg(test)]
mod save_tests;
mod scenario;
pub(crate) mod selection_scenario;
mod support_ui;
pub fn track_scenario(output: PathBuf) -> Result<(), Box<dyn Error>> {
    m7_scenario::run(output)
}
mod project_library;
mod recent;
mod ride_camera;
pub(crate) mod ride_quality;
pub(crate) mod ride_scenario;
mod session;
mod snap;
#[cfg(test)]
mod snap_tests;
mod snap_ui;
mod spline;
pub(crate) mod startup;
mod startup_pages;
#[cfg(test)]
mod startup_tests;
mod startup_ui;
#[cfg(test)]
mod tests;
mod theme;
mod timeline;
mod train_asset_ui;
mod train_assets;
mod train_playback;
mod train_playback_ui;
mod train_preview;
pub(crate) mod transition_scenario;
pub(crate) mod ui_reference_scenario;
mod vehicle_ui;
mod workbench;
mod workflow;
#[cfg(test)]
mod workflow_corrections_tests;
pub(crate) mod workflow_scenario;
use qx_geometry::solver::GenerationOptions;
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use session::{Session, scene};
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
pub fn train_asset_scenario(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    m81_scenario::run(input, output)
}
pub fn train_playback_scenario(output: PathBuf) -> Result<(), Box<dyn Error>> {
    m82_scenario::run(output)
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
    session
        .assets
        .load_blocking(session.editor.project(), &session.path);
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
            return Err("render selection refers to an absent region ID".into());
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
            return Err("selected region does not support this authored channel".into());
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
        self.expansion.structure = None;
        self.select(id);
    }
    fn select_support(&mut self, structure: u64, member: u64) {
        self.expansion.structure = Some(structure);
        self.expansion.member = Some(member);
        self.expansion.node = None;
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
