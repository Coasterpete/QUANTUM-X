//! Approved workspace composition with replaceable development presentation.
use super::{
    graph, panels,
    session::Session,
    workbench::{Layout, Pane, Profile},
};
use egui_tiles::{Behavior, Tile, TileId, Tiles};
use qx_rendering::{DisplayOptions, InspectionScene, StartupSizing, ViewCommand, WindowCommand};

pub(super) fn show(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) -> (egui::Rect, Option<ViewCommand>, Option<WindowCommand>) {
    super::theme::install(ctx);
    ctx.set_zoom_factor(session.workbench.window.ui_scale);
    if session.file_dialog.is_none()
        && session.graph_drag.is_none()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Num1))
    {
        session.workbench.active = Profile::Track;
    }
    if session.file_dialog.is_none()
        && session.graph_drag.is_none()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Num2))
    {
        session.workbench.active = Profile::Train;
    }
    let (mut window_command, float_action, view_command) =
        super::commands::show(session, ctx, display);
    let profile = session.workbench.active;
    let mut layout = std::mem::replace(
        &mut session.workbench.layouts[profile.index()],
        Layout {
            tree: egui_tiles::Tree::empty(egui::Id::unique(("unused", profile.index()))),
            floating: vec![],
        },
    );
    if let Some((pane, float)) = float_action {
        if float {
            layout.float(pane);
        } else {
            layout.dock(pane);
        }
    }
    let mut behavior = PaneUi {
        session,
        scene,
        display,
        viewport: egui::Rect::NOTHING,
        command: view_command,
        float: None,
    };
    ctx.root_ui(|root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                layout.tree.ui(&mut behavior, ui);
            });
    });
    if let Some(pane) = behavior.float.take() {
        layout.float(pane);
    }
    let mut dock = None;
    let available = ctx.content_rect();
    for floating in &mut layout.floating {
        let mut open = true;
        let size = egui::vec2(floating.size[0], floating.size[1]).clamp(
            egui::vec2(180., 120.),
            available.size().max(egui::vec2(180., 120.)),
        );
        let position = egui::pos2(floating.position[0], floating.position[1])
            .clamp(available.min, (available.max - size).max(available.min));
        let window = egui::Window::new(floating.pane.title())
            .id(egui::Id::unique((
                "floating",
                profile.index(),
                floating.pane,
            )))
            .open(&mut open)
            .current_pos(position)
            .default_size(size)
            .min_size(egui::vec2(180., 120.))
            .max_size(available.size().max(egui::vec2(180., 120.)))
            .constrain_to(available)
            .show(ctx, |ui| {
                if ui.button("Dock pane").clicked() {
                    dock = Some(floating.pane);
                }
                if floating.pane == Pane::Transition {
                    behavior.content(ui, floating.pane);
                } else {
                    egui::ScrollArea::both()
                        .id_salt(("float scroll", floating.pane))
                        .show(ui, |ui| behavior.content(ui, floating.pane));
                }
            });
        if let Some(w) = window {
            floating.position = [w.response.rect.min.x, w.response.rect.min.y];
            floating.size = [w.response.rect.width(), w.response.rect.height()];
        }
        if !open {
            dock = Some(floating.pane);
        }
    }
    if let Some(pane) = dock {
        layout.dock(pane);
    }
    let result = (behavior.viewport, behavior.command);
    session.workbench.layouts[profile.index()] = layout;
    if session.display_dialog {
        display_preferences(session, ctx, &mut window_command);
    }
    (result.0, result.1, window_command)
}

struct PaneUi<'a> {
    session: &'a mut Session,
    scene: &'a InspectionScene,
    display: &'a mut DisplayOptions,
    viewport: egui::Rect,
    command: Option<ViewCommand>,
    float: Option<Pane>,
}
impl PaneUi<'_> {
    fn content(&mut self, ui: &mut egui::Ui, pane: Pane) {
        match pane {
            Pane::Track => panels::track(self.session, ui),
            Pane::Transition => graph::editor(self.session, ui, self.scene),
            Pane::Support => support(ui),
            Pane::View => self.view(ui),
            Pane::Diagnostics => self.diagnostics(ui),
            Pane::Configuration => {
                ui.heading("Nominal train configuration");
                ui.small("Document settings for reference geometry. Consist, mass, bogies and train assets are not implemented.");
                panels::heartline(self.session, ui);
                ui.separator();
                ui.collapsing("Reference preview controls", |ui| self.view(ui));
            }
            Pane::Summary => {
                let project = self.session.editor.project();
                ui.heading("Committed configuration");
                ui.label(format!("Project: {}", project.metadata.name));
                ui.monospace(format!(
                    "Nominal train ID: {}",
                    project
                        .heartline
                        .train_configuration_id
                        .as_deref()
                        .unwrap_or("Unassigned")
                ));
                ui.label(match project.heartline.authored_reference {
                    qx_geometry::heartline::AuthoredReference::TrackCenterline => {
                        "Authored reference: track centerline"
                    }
                    qx_geometry::heartline::AuthoredReference::RiderReference => {
                        "Authored reference: rider reference"
                    }
                });
                match project.heartline.rider_offset {
                    qx_geometry::heartline::OffsetProfile::Constant { offset_m } => {
                        offset_summary(ui, "Constant rider offset", offset_m)
                    }
                    qx_geometry::heartline::OffsetProfile::Quintic { start_m, end_m } => {
                        offset_summary(ui, "Quintic start offset", start_m);
                        offset_summary(ui, "Quintic end offset", end_m);
                    }
                }
                ui.monospace(format!(
                    "Design speed: {:.6} m/s",
                    project.initial.reference_speed_mps
                ));
                ui.small("Design speed controls track-generation kinematics; it is not simulated vehicle speed.");
                if self.session.heartline_draft != project.heartline {
                    ui.strong("Configuration draft pending Apply / Cancel");
                }
                ui.separator();
                ui.label("Train geometry, articulated dynamics and ride systems are unavailable.");
                ui.label("Preview displays the shared track/rider reference geometry only.");
            }
            Pane::Viewport | Pane::Preview => {
                let compact = ui.available_width() < 600. || ui.available_height() < 220.;
                ui.horizontal_wrapped(|ui| {
                    if !compact {
                        ui.small(if pane == Pane::Preview {
                            "Reference preview · train geometry unavailable"
                        } else {
                            "3D track references · no rail surfaces"
                        });
                    }
                    for (icon, name, command) in [
                        (super::icons::Icon::FitAll, "Fit All", ViewCommand::FitAll),
                        (
                            super::icons::Icon::FitSelection,
                            "Fit Selection",
                            ViewCommand::FitSelected,
                        ),
                    ] {
                        if super::icons::button(
                            ui,
                            icon,
                            if compact { "" } else { name },
                            true,
                            name,
                        )
                        .clicked()
                        {
                            self.command = Some(command);
                        }
                    }
                    if !compact
                        && super::icons::button(
                            ui,
                            super::icons::Icon::Perspective,
                            "Perspective",
                            true,
                            "Perspective camera",
                        )
                        .clicked()
                    {
                        self.command = Some(ViewCommand::Perspective);
                    }
                    ui.menu_button(if compact { "View" } else { "Navigate" }, |ui| {
                        ui.small(if pane == Pane::Preview {
                            "Reference geometry; train model unavailable"
                        } else {
                            "Track references; rail surfaces unavailable"
                        });
                        for (name, command) in [
                            ("Perspective", ViewCommand::Perspective),
                            ("Top", ViewCommand::Top),
                            ("Side", ViewCommand::Side),
                            ("Front", ViewCommand::Front),
                            ("Toggle orthographic", ViewCommand::ToggleOrthographic),
                        ] {
                            if ui.button(name).clicked() {
                                self.command = Some(command);
                                ui.close();
                            }
                        }
                        ui.separator();
                        ui.label("Click track: select section · left drag: orbit");
                        ui.label("Middle/right drag: pan");
                        ui.label("Wheel: zoom · F: Fit All");
                    });
                });
                self.viewport = ui.available_rect_before_wrap().intersect(ui.clip_rect());
                ui.allocate_rect(self.viewport, egui::Sense::hover());
                if pane == Pane::Viewport {
                    let label = if self.session.draft_error.is_some() {
                        "Previous valid geometry · draft has an error"
                    } else if self.session.visible_revision != self.session.revision {
                        "Generating current inputs · previous geometry visible"
                    } else if self.session.visible_preview {
                        "DRAFT PREVIEW · Apply to keep"
                    } else {
                        "Committed geometry"
                    };
                    ui.painter().text(
                        self.viewport.left_top() + egui::vec2(10., 8.),
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::monospace(11.),
                        egui::Color32::LIGHT_GRAY,
                    );
                    if let Some(range) = self
                        .scene
                        .track
                        .sections
                        .iter()
                        .find(|r| r.id == self.session.selected)
                    {
                        for (index, label) in
                            [(range.start_sample, "Start"), (range.end_sample, "End")]
                        {
                            let sample = &self.scene.track.samples[index];
                            let text = format!(
                                "{label}: ({:.3}, {:.3}, {:.3}) m",
                                sample.position_m.x, sample.position_m.y, sample.position_m.z
                            );
                            let y = if label == "Start" { -32. } else { -16. };
                            ui.painter().text(
                                self.viewport.left_bottom() + egui::vec2(10., y),
                                egui::Align2::LEFT_BOTTOM,
                                text,
                                egui::FontId::monospace(11.),
                                egui::Color32::LIGHT_GRAY,
                            );
                        }
                    }
                }
            }
        }
    }
    fn view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Viewport references");
        ui.horizontal_wrapped(|ui| {
            for (label, command) in [
                ("Fit all · F", ViewCommand::FitAll),
                ("Fit section", ViewCommand::FitSelected),
                ("Perspective", ViewCommand::Perspective),
                ("Top", ViewCommand::Top),
                ("Side", ViewCommand::Side),
                ("Front", ViewCommand::Front),
                ("Orthographic", ViewCommand::ToggleOrthographic),
            ] {
                if ui.button(label).clicked() {
                    self.command = Some(command);
                }
            }
        });
        ui.small(
            "Left drag: orbit. Middle/right drag: pan. Wheel: zoom. T/N/L = X/Y/Z at identity.",
        );
        ui.checkbox(&mut self.display.centerline, "Authored reference path");
        ui.checkbox(
            &mut self.display.reference,
            "Calculated track / rider reference",
        );
        ui.checkbox(&mut self.display.offsets, "Signed offset vectors");
        ui.checkbox(&mut self.display.points, "Reference-point crosses");
        ui.checkbox(&mut self.display.frames, "Orientation frames");
        ui.checkbox(&mut self.display.curvature, "Curvature vectors (scaled)");
        ui.checkbox(&mut self.display.loads, "Calculated-path loads (scaled)");
        ui.checkbox(&mut self.display.grid, "3D ground reference grid");
        ui.add(
            egui::DragValue::new(&mut self.display.grid_spacing)
                .range(0.01..=1e6)
                .prefix("Ground spacing m "),
        )
        .on_hover_text("World-space 3D reference only. Transition plot dots adapt independently.");
    }
    fn diagnostics(&mut self, ui: &mut egui::Ui) {
        ui.heading("Generation diagnostics");
        ui.monospace(format!(
            "Authored path: {:.6} m",
            self.scene.track.length_m()
        ));
        if let Some(reference) = &self.scene.reference {
            ui.monospace(format!("Calculated path: {:.6} m", reference.length_m));
            ui.small("Loads are design-speed kinematic estimates, not train telemetry.");
            for warning in &reference.warnings {
                ui.label(warning);
            }
        }
        for diagnostic in &self.scene.track.diagnostics {
            ui.label(format!(
                "Section {} / {}: {}",
                diagnostic.section_id.0, diagnostic.code, diagnostic.message
            ));
        }
        if self.scene.track.diagnostics.is_empty() {
            ui.label("No configured diagnostics triggered.");
        }
        ui.separator();
        ui.add(
            egui::DragValue::new(&mut self.session.options.max_step_m)
                .speed(0.01)
                .range(1e-6..=10.)
                .prefix("Max step m "),
        );
        if ui.button("Regenerate geometry").clicked() {
            self.session.regenerate();
        }
    }
}
impl Behavior<Pane> for PaneUi<'_> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }
    fn pane_ui(&mut self, ui: &mut egui::Ui, _: TileId, pane: &mut Pane) -> egui_tiles::UiResponse {
        ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
        if pane.is_viewport() {
            self.content(ui, *pane);
        } else {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(8.)
                .show(ui, |ui| {
                    if *pane == Pane::Transition {
                        self.content(ui, *pane);
                    } else {
                        egui::ScrollArea::both()
                            .id_salt(("pane scroll", *pane))
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.content(ui, *pane));
                    }
                });
        }
        egui_tiles::UiResponse::None
    }
    fn gap_width(&self, _: &egui::Style) -> f32 {
        7.
    }
    fn min_size(&self) -> f32 {
        160.
    }
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }
    fn on_tab_button(
        &mut self,
        tiles: &mut Tiles<Pane>,
        id: TileId,
        response: egui::Response,
    ) -> egui::Response {
        if let Some(Tile::Pane(pane)) = tiles.get(id)
            && !pane.is_viewport()
        {
            response.context_menu(|ui| {
                if ui.button("Float pane").clicked() {
                    self.float = Some(*pane);
                    ui.close();
                }
            });
        }
        response
    }
}
fn offset_summary(ui: &mut egui::Ui, label: &str, value: qx_geometry::math::Vec3) {
    ui.label(label);
    egui::Grid::new(label).num_columns(2).show(ui, |ui| {
        for (axis, value) in [
            ("T · forward", value.x),
            ("N · rider up", value.y),
            ("L · lateral", value.z),
        ] {
            ui.label(axis);
            ui.monospace(format!("{value:.6} m"));
            ui.end_row();
        }
    });
}
fn support(ui: &mut egui::Ui) {
    ui.heading("Support Settings");
    ui.label("This document has no support construction model yet.");
    ui.small("Support-specific settings will live here alongside tools referencing the same coaster document.");
    ui.collapsing("Support Appearance", |ui| {
        ui.label("No support structure or material is available to edit.");
        ui.small("Focused support subpanel; separate from application display preferences.");
    });
    ui.collapsing("Unavailable construction tools", |ui| {
        for text in [
            "Support prefab tools",
            "Foundation generator",
            "Rail connector generator",
            "Copy/paste support definitions",
            "Structure, node, member and attachment selection/editing",
        ] {
            ui.label(format!("{text}: unavailable"));
        }
    });
}
fn display_preferences(
    session: &mut Session,
    ctx: &egui::Context,
    command: &mut Option<WindowCommand>,
) {
    let mut open = session.display_dialog;
    egui::Window::new("Display preferences").open(&mut open).resizable(false).show(ctx, |ui| {
        let p = &mut session.window_draft;
        ui.label("Application preferences · logical client pixels");
        egui::ComboBox::from_id_salt("startup size").selected_text(format!("{:?}", p.startup)).show_ui(ui, |ui| {
            for mode in [StartupSizing::Automatic, StartupSizing::Custom, StartupSizing::Remember] { ui.selectable_value(&mut p.startup, mode, format!("{mode:?}")); }
        });
        ui.horizontal(|ui| {
            ui.label("Width"); ui.add(egui::DragValue::new(&mut p.custom_size[0]).range(960..=16384));
            ui.label("Height"); ui.add(egui::DragValue::new(&mut p.custom_size[1]).range(640..=16384));
        });
        ui.checkbox(&mut p.maximized, "Maximized");
        ui.add(egui::Slider::new(&mut p.ui_scale, 0.75..=2.).text("UI scale"))
            .on_hover_text("Scales UI text and controls independently of window dimensions and Windows monitor DPI.");
        ui.small("Automatic uses the monitor work area. Remember restores the last normal size. Custom uses the width/height above. Bounds are clamped to the available monitor.");
        if ui.button("Apply display preferences").clicked() {
            p.sanitize();
            session.workbench.window = p.clone();
            *command = Some(WindowCommand::Apply(p.clone()));
            session.display_dialog = false;
        }
    });
    session.display_dialog &= open;
}
