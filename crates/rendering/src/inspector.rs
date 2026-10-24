use crate::{
    InspectionScene,
    camera::Camera,
    gpu::Gpu,
    scene::{DisplayOptions, vertices},
};
use qx_geometry::math::Vec3;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
pub const HELP: &str = "QUANTUM-X DEVELOPMENT-ONLY geometry inspector (temporary controls)\n\
Left drag / arrows: orbit | Right/middle drag / WASD: pan | Wheel / +/-: zoom\n\
F/Home: fit all | Tab: next section | 0: clear selection | Enter: fit selection\n\
1: perspective | 2: top | 3: side (+Z) | 4: front (+X) | O: toggle orthographic\n\
G: reference grid | [/]: halve/double grid spacing | N: orientation frames\n\
K: curvature vectors (scaled diagnostic) | R: reload current project\n\
Drop a .qx.json project onto the window to open it | H: print this help | Esc: close\n\
White boundary crosses mark sections. Selected section is white and named in the title.\n\
This displays centerlines, not physical rails; design-speed loads are not vehicle physics.";
type Loader = Arc<dyn Fn(&Path) -> Result<InspectionScene, String> + Send + Sync>;
pub fn inspect(
    path: PathBuf,
    loader: impl Fn(&Path) -> Result<InspectionScene, String> + Send + Sync + 'static,
) -> Result<(), String> {
    let scene = loader(&path)?;
    validate_scene(&scene)?;
    let event_loop = EventLoop::new().map_err(|e| format!("native windowing unavailable: {e}"))?;
    println!("{HELP}");
    let mut app = Inspector {
        path,
        scene,
        loader: Arc::new(loader),
        window: None,
        gpu: None,
        camera: Camera::default(),
        display: DisplayOptions::default(),
        origin: Vec3::ZERO,
        cursor: None,
        drag: None,
        pending: None,
        status: "H: help | F: fit | Tab: select | R: reload".into(),
        fatal: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    app.fatal.map_or(Ok(()), Err)
}
fn validate_scene(scene: &InspectionScene) -> Result<(), String> {
    if scene.track.samples.len() < 2
        || scene.track.sections.is_empty()
        || scene.track.graphs.len() + 1 != scene.track.samples.len()
    {
        return Err("inspector requires generated samples and sections".into());
    }
    if scene
        .track
        .sections
        .iter()
        .any(|s| s.start_sample > s.end_sample || s.end_sample >= scene.track.samples.len())
    {
        return Err("invalid generated section range".into());
    }
    Ok(())
}
struct Inspector {
    path: PathBuf,
    scene: InspectionScene,
    loader: Loader,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    camera: Camera,
    display: DisplayOptions,
    origin: Vec3,
    cursor: Option<(f64, f64)>,
    drag: Option<MouseButton>,
    pending: Option<mpsc::Receiver<(PathBuf, Result<InspectionScene, String>)>>,
    status: String,
    fatal: Option<String>,
}
impl Inspector {
    fn aspect(&self) -> f64 {
        self.window.as_ref().map_or(1., |w| {
            let s = w.inner_size();
            s.width.max(1) as f64 / s.height.max(1) as f64
        })
    }
    fn fit(&mut self, selected: bool) {
        let (min, max) = self.scene.bounds(if selected {
            self.display.selected
        } else {
            None
        });
        self.camera.fit(min, max, self.aspect());
    }
    fn rebuild(&mut self) -> Result<(), String> {
        if let Some(gpu) = &mut self.gpu {
            gpu.upload(&vertices(&self.scene, self.origin, &self.display))?;
        }
        Ok(())
    }
    fn title(&self) {
        if let Some(w) = &self.window {
            let selected = self
                .display
                .selected
                .and_then(|i| self.scene.track.sections.get(i))
                .map(|s| {
                    self.scene
                        .labels
                        .iter()
                        .find(|l| l.id == s.id)
                        .map_or(format!("Section {}", s.id.0), |l| {
                            format!("{}: {} ({})", s.id.0, l.name, l.method)
                        })
                })
                .unwrap_or_else(|| "All sections".into());
            w.set_title(&format!(
                "QUANTUM-X [DEVELOPMENT INSPECTOR] | {} | {} | {} | grid {}m N:{} K:{} | {}",
                self.scene.name,
                selected,
                if self.camera.orthographic {
                    "Orthographic"
                } else {
                    "Perspective"
                },
                self.display.grid_spacing,
                self.display.frames,
                self.display.curvature,
                self.status
            ));
        }
    }
    fn reload(&mut self, path: PathBuf) {
        if self.pending.is_some() {
            self.status = "Load already in progress".into();
            return;
        }
        let loader = self.loader.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let scene = loader(&path).and_then(|s| {
                validate_scene(&s)?;
                Ok(s)
            });
            let _ = tx.send((path, scene));
        });
        self.pending = Some(rx);
        self.status = "Loading and regenerating project...".into();
    }
    fn key(&mut self, key: KeyCode, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let mut rebuild = false;
        match key {
            KeyCode::Escape => event_loop.exit(),
            KeyCode::KeyF | KeyCode::Home => self.fit(false),
            KeyCode::Enter => self.fit(true),
            KeyCode::Tab => {
                self.display.selected = Some(
                    self.display
                        .selected
                        .map_or(0, |i| (i + 1) % self.scene.track.sections.len()),
                );
                rebuild = true;
            }
            KeyCode::Digit0 => {
                self.display.selected = None;
                rebuild = true;
            }
            KeyCode::Digit1 => {
                self.camera.orthographic = false;
                self.camera.yaw = 0.8;
                self.camera.pitch = 0.5;
            }
            KeyCode::Digit2 => {
                self.camera.orthographic = true;
                self.camera.yaw = 0.;
                self.camera.pitch = std::f64::consts::FRAC_PI_2;
            }
            KeyCode::Digit3 => {
                self.camera.orthographic = true;
                self.camera.yaw = std::f64::consts::FRAC_PI_2;
                self.camera.pitch = 0.;
            }
            KeyCode::Digit4 => {
                self.camera.orthographic = true;
                self.camera.yaw = 0.;
                self.camera.pitch = 0.;
            }
            KeyCode::KeyO => self.camera.orthographic = !self.camera.orthographic,
            KeyCode::KeyG => {
                self.display.grid = !self.display.grid;
                rebuild = true;
            }
            KeyCode::KeyN => {
                self.display.frames = !self.display.frames;
                rebuild = true;
            }
            KeyCode::KeyK => {
                self.display.curvature = !self.display.curvature;
                rebuild = true;
            }
            KeyCode::BracketLeft => {
                self.display.grid_spacing = (self.display.grid_spacing * 0.5).max(0.01);
                rebuild = true;
            }
            KeyCode::BracketRight => {
                self.display.grid_spacing = (self.display.grid_spacing * 2.).min(1e6);
                rebuild = true;
            }
            KeyCode::KeyR => self.reload(self.path.clone()),
            KeyCode::KeyH => {
                println!("{HELP}");
                self.status =
                    "Drag: orbit/pan | Wheel: zoom | 1-4: views | G/N/K: diagnostics".into();
            }
            KeyCode::ArrowLeft => self.camera.orbit(-0.1, 0.),
            KeyCode::ArrowRight => self.camera.orbit(0.1, 0.),
            KeyCode::ArrowUp => self.camera.orbit(0., 0.1),
            KeyCode::ArrowDown => self.camera.orbit(0., -0.1),
            KeyCode::KeyA => self.camera.pan(-self.camera.distance * 0.03, 0.),
            KeyCode::KeyD => self.camera.pan(self.camera.distance * 0.03, 0.),
            KeyCode::KeyW => self.camera.pan(0., self.camera.distance * 0.03),
            KeyCode::KeyS => self.camera.pan(0., -self.camera.distance * 0.03),
            KeyCode::Equal | KeyCode::NumpadAdd => self.camera.zoom(-1.),
            KeyCode::Minus | KeyCode::NumpadSubtract => self.camera.zoom(1.),
            _ => {}
        }
        if rebuild {
            self.rebuild()?;
        }
        Ok(())
    }
    fn fail(&mut self, error: String, event_loop: &ActiveEventLoop) {
        eprintln!("Inspector: {error}");
        self.status = error.clone();
        self.title();
        self.fatal = Some(error);
        event_loop.exit();
    }
}
impl ApplicationHandler for Inspector {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("QUANTUM-X [DEVELOPMENT INSPECTOR]")
                            .with_inner_size(LogicalSize::new(1200., 800.)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let gpu = pollster::block_on(Gpu::new(window.clone()))?;
            self.window = Some(window);
            self.gpu = Some(gpu);
            let (min, max) = self.scene.bounds(None);
            self.origin = (min + max) * 0.5;
            self.fit(false);
            self.rebuild()?;
            self.title();
            self.window.as_ref().unwrap().request_redraw();
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(e, event_loop);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let result = (|| -> Result<(), String> {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    if let Some(g) = &mut self.gpu {
                        g.resize(size);
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    if let (Some(g), Some(w)) = (&mut self.gpu, &self.window) {
                        g.resize(w.inner_size());
                    }
                }
                WindowEvent::Occluded(false) => {}
                WindowEvent::DroppedFile(path) => self.reload(path),
                WindowEvent::Focused(false) => {
                    self.drag = None;
                    self.cursor = None;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    self.drag = if state == ElementState::Pressed {
                        Some(button)
                    } else {
                        None
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if let Some((x, y)) = self.cursor {
                        let dpi = self.window.as_ref().map_or(1., |w| w.scale_factor());
                        let dx = (position.x - x) / dpi;
                        let dy = (position.y - y) / dpi;
                        match self.drag {
                            Some(MouseButton::Left) => self.camera.orbit(-dx * 0.007, dy * 0.007),
                            Some(MouseButton::Right | MouseButton::Middle) => {
                                let scale = self.camera.distance * 0.0015;
                                self.camera.pan(-dx * scale, dy * scale);
                            }
                            _ => {}
                        }
                    }
                    self.cursor = Some((position.x, position.y));
                }
                WindowEvent::MouseWheel { delta, .. } => self.camera.zoom(match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y as f64,
                    MouseScrollDelta::PixelDelta(p) => -p.y / 60.,
                }),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed =>
                {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        self.key(code, event_loop)?;
                    }
                }
                WindowEvent::RedrawRequested => {
                    if let (Some(g), Some(w)) = (&mut self.gpu, &self.window) {
                        g.draw(w, &self.camera, self.origin)?;
                    }
                    return Ok(());
                }
                _ => {}
            }
            self.title();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(e, event_loop);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok((path, result)) => {
                    self.pending = None;
                    match result {
                        Ok(scene) => {
                            self.path = path;
                            self.scene = scene;
                            self.display.selected = None;
                            let (min, max) = self.scene.bounds(None);
                            self.origin = (min + max) * 0.5;
                            self.fit(false);
                            if let Err(e) = self.rebuild() {
                                self.fail(e, event_loop);
                                return;
                            }
                            self.status = format!(
                                "Loaded {} sections, {:.3}m",
                                self.scene.track.sections.len(),
                                self.scene.track.length_m()
                            );
                            println!("{}", self.status);
                        }
                        Err(e) => {
                            self.status = format!("Load failed: {e}");
                            eprintln!("{}; previous geometry retained", self.status);
                        }
                    }
                    self.title();
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.status = "Load worker failed; previous geometry retained".into();
                    self.title();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        event_loop.set_control_flow(if self.pending.is_some() {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(30))
        } else {
            ControlFlow::Wait
        });
    }
}
