use super::{Inputs, Loaded, load};
use qx_rendering::{
    DisplayMode, DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput, ViewCommand,
};
use std::{path::Path, sync::mpsc};
pub(super) struct DiagnosticSurface {
    inputs: Inputs,
    pub loaded: Option<Loaded>,
    pending: Option<InspectionScene>,
    worker: Option<mpsc::Receiver<Result<Loaded, String>>>,
    status: String,
    pub show_alignment: bool,
    markers: bool,
    pub closeup: bool,
}
impl DiagnosticSurface {
    pub fn new(inputs: Inputs, loaded: Loaded) -> Self {
        Self {
            inputs,
            loaded: Some(loaded),
            pending: None,
            worker: None,
            status: String::new(),
            show_alignment: false,
            markers: true,
            closeup: false,
        }
    }
    fn reload(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let inputs = self.inputs.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(load(&inputs));
        });
        self.worker = Some(rx);
        self.status = "Importing read-only files…".into();
    }
    pub(super) fn clear(&mut self) {
        self.worker = None;
        self.loaded = None;
        self.pending = Some(InspectionScene::empty());
        self.status = "Diagnostic geometry cleared. Native documents are untouched.".into();
    }
}
impl EditorSurface for DiagnosticSurface {
    fn window_title(&self) -> String {
        "QUANTUM Dynamics — DEVELOPMENT ONLY CSV/XML inspection".into()
    }
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let mut command = None;
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            command = Some(ViewCommand::FitAll);
            self.closeup = false;
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R)) {
            self.reload();
        }
        let mut viewport = egui::Rect::NOTHING;
        ctx.root_ui(|root| {
        egui::Panel::left("diagnostic-import").default_size(330.).resizable(true).show(root,|ui|{
            ui.heading("CSV/XML diagnostic");
            ui.strong("DEVELOPMENT ONLY · READ ONLY");
            ui.label("No Regions, terrain or vehicle simulation.");
            ui.horizontal(|ui|{
                if ui.button("Fit all (F)").clicked() {command=Some(ViewCommand::FitAll);self.closeup=false;}
                if ui.add_enabled(self.worker.is_none(),egui::Button::new("Reload (Ctrl+R)")).clicked(){self.reload();}
                if ui.button("Clear").clicked(){self.clear();}
            });
            ui.horizontal(|ui|{
                for (name,view) in [("Perspective",ViewCommand::Perspective),("Top",ViewCommand::Top),("Side",ViewCommand::Side)] {
                    if ui.button(name).clicked(){command=Some(view);self.closeup=false;}
                }
            });
            ui.label("Left drag: orbit · right/middle drag: pan · wheel: zoom");
            ui.checkbox(&mut display.running,"Running rails");ui.checkbox(&mut display.structure,"Track structure");
            ui.checkbox(&mut display.hardware,"Cross ties / hardware");ui.checkbox(&mut display.supports,"Imported support beams");
            ui.checkbox(&mut display.centerline,"Exported spline");ui.checkbox(&mut display.frames,"Orientation frames");
            ui.checkbox(&mut self.markers,"Footer / connector position markers");
            ui.checkbox(&mut self.show_alignment,"Connector-to-path distance lines");
            ui.label("Amber: footer / index-0 distance · green: connector · red: missing track reference");
            if !self.status.is_empty(){ui.label(&self.status);}
            egui::ScrollArea::vertical().show(ui,|ui|{
                if let Some(l)=&self.loaded {
                    ui.separator();ui.label(format!("{} samples · {:.3} m polyline",l.track.samples.len(),l.track.polyline_length_m));
                    ui.label(format!("Elevation {:.3} … {:.3} m",l.track.bounds.min.y,l.track.bounds.max.y));
                    ui.label(format!("Endpoint gap {:.3} m · no closure added",l.track.endpoint_gap_m));
                    ui.label(format!("Construction: {} (QUANTUM geometry)",self.inputs.construction));
                    if let Some(s)=&l.supports {
                        ui.label(format!("{} nodes · {} beams · {} footers",s.nodes.len(),s.beams.len(),s.footers.len()));
                        if let Some(r)=&l.alignment {
                            ui.label(format!("Main-track nearest distance RMS {:.4} m; max {:.4} m",r.rms_nearest_distance_m,r.max_nearest_distance_m));
                            ui.label(format!("{} connectors reference an absent track",r.unavailable_track_connectors));
                        }
                        ui.label("Distances include attachment offsets. Connector orientation is unavailable; nothing is snapped.");
                        ui.collapsing("Unsupported features",|ui|{for (feature,count) in &s.unsupported {ui.label(format!("{count} × {feature}"));}});
                    }
                    ui.collapsing("Current material slots",|ui|{if let Some(mesh)=&scene.mesh {for m in &mesh.materials {
                        ui.label(format!("{} · roughness {:.2} · metal {:.2}",m.id,m.roughness,m.metallic));
                        if let Some(b)=&m.library {ui.label(format!("Library: {}",b.id));}
                    }}});
                    ui.collapsing("Available material library",|ui|{for m in &l.materials{ui.label(m);}});
                    for warning in &scene.materials.warnings {ui.label(warning);}
                }
            });
        });
        viewport=egui::CentralPanel::default().frame(egui::Frame::NONE).show(root,|ui|ui.max_rect()).inner;
        });
        display.grid = false;
        display.boundaries = false;
        display.curvature = false;
        display.mode = DisplayMode::RenderedCenterline;
        display.overlay_lines.clear();
        display.train_bounds = None;
        if let Some(l) = &self.loaded {
            if self.markers {
                display.overlay_lines.extend_from_slice(&l.markers);
            }
            if self.show_alignment {
                display.overlay_lines.extend_from_slice(&l.alignment_lines);
            }
            if self.closeup {
                let p = l
                    .alignment
                    .as_ref()
                    .and_then(|r| r.residuals.iter().find(|r| r.track_index == 0))
                    .map_or(l.track.samples[l.track.samples.len() / 2].position_m, |r| {
                        r.connector_position_m
                    });
                let extent = qx_geometry::math::Vec3::new(12., 12., 12.);
                display.train_bounds = Some((p - extent, p + extent));
            }
        }
        SurfaceOutput {
            viewport,
            view_command: command,
            close: ctx.input(|i| i.key_pressed(egui::Key::Escape)),
            window_command: None,
            profile: 0,
            reset_navigation: false,
        }
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        if let Some(rx) = &self.worker {
            match rx.try_recv() {
                Ok(result) => {
                    self.worker = None;
                    match result {
                        Ok(l) => {
                            self.pending = Some(l.scene.clone());
                            self.loaded = Some(l);
                            self.status = "Read-only import reloaded. F fits all bounds.".into();
                        }
                        Err(e) => self.status = e,
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    self.status = "Import worker stopped; previous scene retained.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        self.pending.take()
    }
    fn open_path(&mut self, _: &Path) {
        self.status="Use diagnostic-import CLI to choose a CSV/XML pair. Native project drops do not change this diagnostic scene.".into();
    }
    fn request_close(&mut self) -> bool {
        true
    }
}
