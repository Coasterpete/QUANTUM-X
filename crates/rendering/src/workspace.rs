//! Native egui composition over the existing wgpu line renderer. App document,
//! commands, generation workers and widget decisions stay outside this crate.
use crate::{
    DisplayOptions, InspectionScene,
    camera::Camera,
    gpu::{Gpu, GuiFrame},
    scene::vertices,
    window::{self, WindowCommand, WindowPreferences},
};
use qx_geometry::math::Vec3;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Clone, Copy)]
pub enum ViewCommand {
    FitAll,
    FitSelected,
    FitTrain,
    FocusSelectedCar,
    Perspective,
    Top,
    Side,
    Front,
    ToggleOrthographic,
}
pub struct SurfaceOutput {
    pub viewport: egui::Rect,
    pub view_command: Option<ViewCommand>,
    pub close: bool,
    pub window_command: Option<WindowCommand>,
    pub profile: usize,
    pub reset_navigation: bool,
}
pub trait EditorSurface {
    fn window_title(&self) -> String {
        format!("{} — Editor", crate::identity::product().name)
    }
    fn select_section(&mut self, _id: qx_geometry::authoring::SectionId) {}
    fn inspect_station(&mut self, _point: Option<(qx_geometry::authoring::SectionId, f64)>) {}
    fn window_preferences(&self) -> WindowPreferences {
        WindowPreferences::default()
    }
    fn window_changed(&mut self, _preferences: WindowPreferences) {}
    fn persist_preferences(&mut self, _force: bool) {}
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput;
    fn poll_scene(&mut self) -> Option<InspectionScene>;
    fn open_path(&mut self, path: &std::path::Path);
    /// Allow the app to present a recoverable unsaved-document prompt.
    fn request_close(&mut self) -> bool;
}
pub fn edit(scene: InspectionScene, surface: impl EditorSurface + 'static) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = Workspace {
        scene,
        surface: Box::new(surface),
        window: None,
        gpu: None,
        ctx: egui::Context::default(),
        input: None,
        camera: Camera::default(),
        display: DisplayOptions::default(),
        navigation: std::array::from_fn(|_| (Camera::default(), DisplayOptions::default())),
        profile: 0,
        origin: Vec3::ZERO,
        viewport: egui::Rect::NOTHING,
        cursor: None,
        drag: None,
        drag_start: None,
        needs_fit: [true; 2],
        fatal: None,
        exit_requested: false,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.fatal.map_or(Ok(()), Err)
}
struct Workspace {
    scene: InspectionScene,
    surface: Box<dyn EditorSurface>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    ctx: egui::Context,
    input: Option<egui_winit::State>,
    camera: Camera,
    display: DisplayOptions,
    navigation: [(Camera, DisplayOptions); 2],
    profile: usize,
    origin: Vec3,
    viewport: egui::Rect,
    cursor: Option<(f64, f64)>,
    drag: Option<MouseButton>,
    drag_start: Option<(f64, f64)>,
    needs_fit: [bool; 2],
    fatal: Option<String>,
    exit_requested: bool,
}
impl Workspace {
    fn fit(&mut self, selected: bool) {
        if !selected
            && self.profile == 1
            && let Some((min, max)) = self.display.train_bounds
        {
            self.camera.fit(
                min,
                max,
                (self.viewport.width() / self.viewport.height().max(1.)).max(0.1) as f64,
            );
            return;
        }
        let (min, max) = self.scene.display_bounds(
            if selected {
                self.display.selected
            } else {
                None
            },
            &self.display,
        );
        self.camera.fit(
            min,
            max,
            (self.viewport.width() / self.viewport.height().max(1.)).max(0.1) as f64,
        );
    }
    fn rebuild(&mut self) -> Result<(), String> {
        if let Some(gpu) = &mut self.gpu {
            gpu.upload_scene(&self.scene, self.origin, &self.display)?;
            gpu.upload(
                &vertices(&self.scene, self.origin, &self.display),
                crate::scene::grid_vertex_count(&self.display),
            )?;
        }
        Ok(())
    }
    fn draw(&mut self) -> Result<(), String> {
        let changed = if let Some(scene) = self.surface.poll_scene() {
            self.scene = scene;
            let (min, max) = self.scene.bounds(None);
            self.origin = (min + max) * 0.5;
            true
        } else {
            false
        };
        let window = self.window.as_ref().unwrap().clone();
        window.set_title(&self.surface.window_title());
        let input = self.input.as_mut().unwrap().take_egui_input(&window);
        let before = self.display.clone();
        let mut action = None;
        let mut window_command = None;
        let mut profile = self.profile;
        let mut reset_navigation = false;
        let output = self.ctx.run_pass(input, |ctx| {
            let result = self.surface.ui(ctx, &self.scene, &mut self.display);
            self.viewport = result.viewport;
            action = result.view_command;
            self.exit_requested = result.close;
            window_command = result.window_command;
            profile = result.profile.min(1);
            reset_navigation = result.reset_navigation;
        });
        self.input
            .as_mut()
            .unwrap()
            .handle_platform_output(&window, output.platform_output);
        if profile != self.profile {
            let instances = std::mem::take(&mut self.display.asset_instances);
            let overlays = std::mem::take(&mut self.display.overlay_lines);
            let bounds = (self.display.train_bounds, self.display.selected_car_bounds);
            self.navigation[self.profile] = (self.camera.clone(), before.clone());
            (self.camera, self.display) = self.navigation[profile].clone();
            self.display.asset_instances = instances;
            self.display.overlay_lines = overlays;
            (self.display.train_bounds, self.display.selected_car_bounds) = bounds;
            self.profile = profile;
            self.drag = None;
        }
        if reset_navigation {
            self.needs_fit = [true; 2];
        }
        if self.needs_fit[self.profile] && self.viewport.is_positive() {
            self.fit(false);
            self.needs_fit[self.profile] = false;
        }
        if let Some(command) = window_command {
            let preferences = match command {
                WindowCommand::Apply(p) => p,
                WindowCommand::Reset => WindowPreferences::default(),
            };
            window::apply(&window, &preferences);
            self.surface.window_changed(preferences);
            self.cursor = None;
            self.drag = None;
        }
        self.surface.persist_preferences(false);
        if let Some(command) = action {
            match command {
                ViewCommand::FitAll => self.fit(false),
                ViewCommand::FitSelected => self.fit(true),
                ViewCommand::FitTrain | ViewCommand::FocusSelectedCar => {
                    let bounds = if matches!(command, ViewCommand::FitTrain) {
                        self.display.train_bounds
                    } else {
                        self.display.selected_car_bounds
                    };
                    if let Some((min, max)) = bounds {
                        self.camera.fit(
                            min,
                            max,
                            self.viewport.width() as f64 / self.viewport.height().max(1.) as f64,
                        );
                    }
                }
                ViewCommand::Perspective => {
                    self.camera.yaw = 0.8;
                    self.camera.pitch = 0.5;
                    self.camera.orthographic = false;
                }
                ViewCommand::Top => {
                    self.camera.yaw = 0.;
                    self.camera.pitch = std::f64::consts::FRAC_PI_2;
                    self.camera.orthographic = true;
                }
                ViewCommand::Side => {
                    self.camera.yaw = std::f64::consts::FRAC_PI_2;
                    self.camera.pitch = 0.;
                    self.camera.orthographic = true;
                }
                ViewCommand::Front => {
                    self.camera.yaw = 0.;
                    self.camera.pitch = 0.;
                    self.camera.orthographic = true;
                }
                ViewCommand::ToggleOrthographic => {
                    self.camera.orthographic = !self.camera.orthographic
                }
            }
        }
        if changed || before != self.display {
            self.rebuild()?;
        }
        let jobs = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        self.gpu.as_mut().unwrap().draw_ui(
            &window,
            &self.camera,
            self.origin,
            Some(GuiFrame {
                jobs,
                textures: output.textures_delta,
                pixels_per_point: output.pixels_per_point,
                viewport: self.viewport,
            }),
        )
    }
}
impl ApplicationHandler for Workspace {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let preferences = self.surface.window_preferences();
            let mut monitors: Vec<_> = event_loop.available_monitors().collect();
            if let Some(primary) = event_loop.primary_monitor() {
                monitors.sort_by_key(|m| *m != primary);
            }
            let areas: Vec<_> = monitors.iter().map(window::work_area).collect();
            let placement = window::resolve(&preferences, &areas);
            let window = Arc::new(
                event_loop
                    .create_window(
                        crate::identity::window_attributes()?
                            .with_title(self.surface.window_title())
                            .with_inner_size(LogicalSize::new(placement.size[0], placement.size[1]))
                            .with_min_inner_size(LogicalSize::new(
                                placement.minimum[0],
                                placement.minimum[1],
                            ))
                            .with_position(winit::dpi::PhysicalPosition::new(
                                placement.position[0],
                                placement.position[1],
                            ))
                            .with_maximized(preferences.maximized),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let gpu = pollster::block_on(Gpu::new(window.clone()))?;
            self.input = Some(egui_winit::State::new(
                self.ctx.clone(),
                egui::ViewportId::ROOT,
                &window,
                Some(window.scale_factor() as f32),
                window.theme(),
                None,
            ));
            self.gpu = Some(gpu);
            self.window = Some(window);
            let (min, max) = self.scene.bounds(None);
            self.origin = (min + max) * 0.5;
            self.viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 500.));
            self.fit(false);
            self.navigation = std::array::from_fn(|_| (self.camera.clone(), self.display.clone()));
            self.navigation[1].1.reference = true;
            self.navigation[1].1.offsets = true;
            self.rebuild()?;
            Ok(())
        })();
        if let Err(e) = result {
            self.fatal = Some(e);
            event_loop.exit();
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = &self.window else {
            return;
        };
        let _response = self.input.as_mut().unwrap().on_window_event(window, &event);
        let capture_window = matches!(
            event,
            WindowEvent::Resized(_)
                | WindowEvent::Moved(_)
                | WindowEvent::ScaleFactorChanged { .. }
        );
        let result = match event {
            WindowEvent::CloseRequested => {
                if self.surface.request_close() {
                    event_loop.exit();
                }
                Ok(())
            }
            WindowEvent::Resized(size) => {
                self.gpu.as_mut().unwrap().resize(size);
                Ok(())
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.cursor = None;
                self.drag = None;
                self.gpu.as_mut().unwrap().resize(window.inner_size());
                Ok(())
            }
            WindowEvent::DroppedFile(path) => {
                self.surface.open_path(&path);
                Ok(())
            }
            WindowEvent::Focused(false) => {
                self.drag = None;
                self.cursor = None;
                Ok(())
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == ElementState::Released {
                    if self.drag == Some(MouseButton::Left)
                        && button == MouseButton::Left
                        && let (Some(start), Some(end)) = (self.drag_start, self.cursor)
                        && (end.0 - start.0).hypot(end.1 - start.1) < 4.
                        && pointer_in_view(&self.ctx, self.viewport, self.cursor)
                        && let Some((id, _)) = self.scene.pick(
                            &self.camera,
                            self.origin,
                            self.viewport,
                            egui::pos2(end.0 as f32, end.1 as f32),
                        )
                    {
                        self.surface.select_section(id);
                    }
                    self.drag = None;
                    self.drag_start = None;
                } else if pointer_in_view(&self.ctx, self.viewport, self.cursor) {
                    self.drag = Some(button);
                    self.drag_start = self.cursor;
                }
                Ok(())
            }
            WindowEvent::CursorMoved { position, .. } => {
                let dpi = self.ctx.pixels_per_point() as f64;
                let next = (position.x / dpi, position.y / dpi);
                if let Some((x, y)) = self.cursor {
                    let dx = next.0 - x;
                    let dy = next.1 - y;
                    match self.drag {
                        Some(MouseButton::Left)
                            if self
                                .drag_start
                                .is_some_and(|s| (next.0 - s.0).hypot(next.1 - s.1) >= 4.) =>
                        {
                            self.camera.orbit(-dx * 0.007, dy * 0.007)
                        }
                        Some(MouseButton::Right | MouseButton::Middle) => self.camera.pan(
                            -dx * self.camera.distance * 0.0015,
                            dy * self.camera.distance * 0.0015,
                        ),
                        _ => {}
                    }
                }
                self.cursor = Some(next);
                let hit = if self.drag.is_none()
                    && pointer_in_view(&self.ctx, self.viewport, self.cursor)
                {
                    self.scene.pick(
                        &self.camera,
                        self.origin,
                        self.viewport,
                        egui::pos2(next.0 as f32, next.1 as f32),
                    )
                } else {
                    None
                };
                self.surface.inspect_station(hit);
                Ok(())
            }
            WindowEvent::MouseWheel { delta, .. }
                if pointer_in_view(&self.ctx, self.viewport, self.cursor) =>
            {
                self.camera.zoom(match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y as f64,
                    MouseScrollDelta::PixelDelta(p) => -p.y / 60.,
                });
                Ok(())
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => Ok(()),
        };
        if capture_window {
            let mut preferences = self.surface.window_preferences();
            window::capture(self.window.as_ref().unwrap(), &mut preferences);
            self.surface.window_changed(preferences);
        }
        if let Err(error) = result {
            eprintln!("Authoring renderer: {error}");
            self.fatal = Some(error);
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit_requested {
            event_loop.exit();
            return;
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
        // Worker completion and egui animations use a bounded polling interval.
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            let mut preferences = self.surface.window_preferences();
            window::capture(window, &mut preferences);
            self.surface.window_changed(preferences);
        }
        self.surface.persist_preferences(true);
    }
}
fn pointer_in_view(ctx: &egui::Context, viewport: egui::Rect, cursor: Option<(f64, f64)>) -> bool {
    cursor.is_some_and(|(x, y)| {
        let p = egui::pos2(x as f32, y as f32);
        viewport.contains(p)
            && !ctx.egui_is_using_pointer()
            && ctx
                .layer_id_at(p)
                .is_none_or(|layer| layer.order == egui::Order::Background)
    })
}
