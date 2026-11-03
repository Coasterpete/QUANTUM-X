//! Noninteractive diagnostics for the existing integrated ride implementation.
use super::{
    ride_camera::Mode,
    session::{Session, scene},
    train_playback::Action,
    workbench::Profile,
};
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use serde_json::json;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(super) fn prepare(input: &Path) -> Result<(Session, InspectionScene), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    let mut project = qx_document::persistence::load(input, &catalog)?;
    if project.trains.is_empty() {
        // The explicit diagnostic may supply the existing one-car gray-box
        // fixture in its output copy; the source track and file remain intact.
        project
            .trains
            .push(qx_document::train::Train::development_consist(
                "ride-capture-development-fixture".into(),
                1,
                qx_document::train::DevelopmentArrangement::Conventional,
            )?);
    }
    if project.heartline.train_configuration_id.is_none() {
        project.heartline.train_configuration_id = project.trains.first().map(|t| t.id.clone());
    }
    if project.heartline.train_configuration_id.is_none() {
        return Err(
            "associate an existing train for this diagnostic; gray boxes are supported".into(),
        );
    }
    let captured = scene(&project, &catalog, Default::default())?;
    let mut s = Session::new(project, Some(input), catalog)?;
    s.assets.load_blocking(s.editor.project(), &s.path);
    s.workbench.active = Profile::Train;
    s.frame_requested = false;
    s.visible_revision = s.revision;
    s.train_station_m = Some(captured.track.length_m() * 0.5);
    s.ride.mode = Mode::Rider;
    Ok((s, captured))
}
pub(super) fn wait(
    s: &mut Session,
    predicate: impl Fn(&super::train_playback::Snapshot) -> bool,
) -> Result<(), Box<dyn Error>> {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        s.playback.sync(s.editor.project(), s.options);
        if let Some(e) = &s.playback.error {
            return Err(e.clone().into());
        }
        if s.playback.snapshot.as_ref().is_some_and(&predicate) {
            return Ok(());
        }
        if Instant::now() > end {
            return Err("playback worker timeout".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
struct Probe {
    session: Session,
    phase: usize,
    camera: Option<qx_rendering::CameraView>,
    instance_origin: Option<[f64; 3]>,
    origin: Option<Instant>,
    target: Option<String>,
    visual_correction: Option<qx_document::train::Asset>,
}
impl EditorSurface for Probe {
    fn diagnostic_tick(&mut self, t: f64) {
        if self.origin.is_none() {
            self.origin = Instant::now().checked_sub(Duration::from_secs_f64(t));
        }
        let phase = if t < 0.75 {
            0
        } else if t < 2.75 {
            1
        } else if t < 3. {
            2
        } else if t < 3.25 {
            3
        } else if t < 7. {
            4
        } else {
            5
        };
        if phase != self.phase {
            self.phase = phase;
            self.session.playback.action(match phase {
                1 | 4 => Action::Play,
                3 => Action::Reset,
                _ => Action::Pause,
            });
        }
    }
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        d: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let result = self.session.ui(ctx, scene, d);
        self.camera = d.camera_view;
        let target = self.session.ride.component.as_deref().or_else(|| {
            self.session
                .editor
                .project()
                .trains
                .iter()
                .find(|t| {
                    Some(&t.id)
                        == self
                            .session
                            .editor
                            .project()
                            .heartline
                            .train_configuration_id
                            .as_ref()
                })
                .and_then(|t| t.cars.first())
                .map(|c| c.body_id.as_str())
        });
        self.instance_origin = d
            .asset_instances
            .iter()
            .find(|i| Some(i.component_id.as_str()) == target)
            .map(|i| [i.transform[3][0], i.transform[3][1], i.transform[3][2]]);
        self.target = target.map(str::to_owned);
        self.visual_correction = self
            .session
            .editor
            .project()
            .trains
            .iter()
            .flat_map(|t| &t.components)
            .find(|c| Some(&c.id) == self.target.as_ref())
            .and_then(|c| c.asset.as_ref())
            .filter(|a| {
                super::train_assets::renderable(a)
                    && self.session.assets.library.contains_key(&a.path)
            })
            .cloned();
        result
    }
    fn frame_diagnostics(&self) -> serde_json::Value {
        let s = &self.session;
        let body = s
            .ride
            .presentation
            .as_ref()
            .and_then(|p| {
                p.components
                    .iter()
                    .find(|p| Some(&p.id) == self.target.as_ref())
            })
            .map(|p| json!({"position_m":p.pose.position_m,"rotation":p.pose.orientation}));
        let worker=s.playback.snapshot.as_ref().map(|p|json!({"tick":p.ticks,"running":p.running,"steps":p.timing.steps,"step_total_ms":p.timing.step_total_ms,"step_max_ms":p.timing.step_max_ms,"over_budget_steps":p.timing.over_budget_steps,"lateness_max_ms":p.timing.lateness_max_ms,"lag_events":p.lag_events,"error":p.error}));
        json!({"phase":self.phase,"component_id":self.target,"visual_correction":self.visual_correction,"sample_wall_s":s.ride.sample_time.zip(self.origin).map(|(t,o)|t.saturating_duration_since(o).as_secs_f64()),"pose_update_ms":s.ride.pose_update_ms,"camera_update_ms":s.ride.camera_update_ms,"presentation_tick":s.ride.presentation_tick,"alpha":s.ride.interpolation_alpha,"starved":s.ride.starved,"body":body,"instance_origin":self.instance_origin,"camera":self.camera.map(|c|json!({"eye_m":c.eye_m,"forward":c.forward,"up":c.up})),"worker":worker,"error":s.ride.error})
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        self.session.poll()
    }
    fn open_path(&mut self, _: &Path) {}
    fn request_close(&mut self) -> bool {
        true
    }
}
pub fn native(input: PathBuf, output: PathBuf, rate: &str) -> Result<(), Box<dyn Error>> {
    let original = std::fs::read(&input)?;
    let (mut s, captured) = prepare(&input)?;
    s.ride.component = s
        .editor
        .project()
        .trains
        .iter()
        .filter(|t| Some(&t.id) == s.editor.project().heartline.train_configuration_id.as_ref())
        .flat_map(|t| &t.components)
        .find(|c| c.kind == qx_document::train::ComponentKind::PassengerAssembly)
        .map(|c| c.id.clone());
    let station = s.train_station_m.unwrap();
    s.playback.initial_speed_mps = 1.;
    s.playback
        .initialize(s.editor.project(), s.options, &captured.track, station);
    wait(&mut s, |p| p.ticks == 0)?;
    let legacy_wait = rate == "legacy";
    let hz = if legacy_wait { 60 } else { rate.parse()? };
    qx_rendering::profile_editor(
        captured,
        Probe {
            session: s,
            phase: 0,
            camera: None,
            instance_origin: None,
            origin: None,
            target: None,
            visual_correction: None,
        },
        qx_rendering::NativeProfile {
            output: output.clone(),
            hz,
            seconds: 8.,
            legacy_wait,
        },
    )?;
    if std::fs::read(input)? != original {
        return Err("native diagnostic changed source bytes".into());
    }
    println!("Native hidden-surface profile saved: {}", output.display());
    Ok(())
}

/// Offline deterministic capture, not a claim about real-time rendering speed.
/// Native recording rasterizes each distinct frame from the same presentation
/// function as live playback, including a fractional fixed-step phase.
pub fn record(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("choose a new recording directory".into());
    }
    let original = std::fs::read(&input)?;
    let (mut s, captured) = prepare(&input)?;
    std::fs::create_dir_all(output.join("pov-frames"))?;
    s.save_to(&output.join("inspection.qx.json"));
    if let Some(e) = &s.save_failure {
        return Err(e.clone().into());
    }
    let saved = s.editor.project().clone();
    let train = saved
        .trains
        .iter()
        .find(|t| Some(&t.id) == saved.heartline.train_configuration_id.as_ref())
        .ok_or("train association unresolved")?
        .clone();
    let mut sim = qx_physics::TrackSimulation::new(
        train,
        std::sync::Arc::new(captured.track.clone()),
        s.train_station_m.unwrap(),
        2.,
    )?;
    let mut recorder = qx_rendering::ViewportRecorder::new(&captured, 1440, 960)?;
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    let epoch = Instant::now();
    let mut frames = vec![];
    let mut stop_error = None;
    let mut step_total_ms = 0.;
    let mut step_max_ms: f64 = 0.;
    'frames: for index in 0..360 {
        let target_tick = index as f64 * 4. + 0.5;
        let current_tick = target_tick.ceil() as u64;
        while sim.ticks() < current_tick {
            let start = Instant::now();
            if let Err(e) = sim.step() {
                stop_error = Some(e);
                break 'frames;
            }
            let ms = start.elapsed().as_secs_f64() * 1000.;
            step_total_ms += ms;
            step_max_ms = step_max_ms.max(ms);
        }
        s.playback.snapshot = Some(super::train_playback::Snapshot {
            poses: sim.snapshot()?,
            previous: sim.previous_snapshot().clone(),
            committed_at: epoch
                + Duration::from_secs_f64(current_tick as f64 * qx_physics::FIXED_STEP_SECONDS),
            ticks: current_tick,
            running: true,
            diagnostics: None,
            speed_mps: vec![],
            lag_events: 0,
            error: None,
            telemetry: Default::default(),
            timing: Default::default(),
            timeline: vec![],
        });
        s.ride.capture_time = Some(
            epoch
                + Duration::from_secs_f64(
                    (target_tick + super::train_playback::PRESENTATION_DELAY_TICKS)
                        * qx_physics::FIXED_STEP_SECONDS,
                ),
        );
        // Warm the real native Editor composition once; subsequent frames retain
        // egui state and all GPU resources instead of initializing eight passes/device per image.
        for _ in 0..if index == 0 { 8 } else { 1 } {
            let mut pass = ctx.run_pass(
                egui::RawInput {
                    time: Some(index as f64 / 60.),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 960.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    s.ui(ctx, &captured, &mut display);
                },
            );
            pass.textures_delta.clear();
        }
        if let Some(e) = &s.ride.error {
            return Err(e.clone().into());
        }
        let file = format!("pov-frames/{index:04}.png");
        let timing = recorder.frame(&captured, &display, &output.join(&file))?;
        let mut evidence = super::ride_scenario::record(&s, &display);
        evidence["file"] = json!(file);
        evidence["frame"] = json!(index);
        evidence["output_time_s"] = json!(index as f64 / 60.);
        evidence["committed_tick"] = json!(current_tick);
        evidence["presentation_tick"] = json!(s.ride.presentation_tick);
        evidence["alpha"] = json!(s.ride.interpolation_alpha);
        evidence["pose_update_ms"] = json!(s.ride.pose_update_ms);
        evidence["camera_update_ms"] = json!(s.ride.camera_update_ms);
        evidence["render"] = serde_json::to_value(timing)?;
        let camera = display.camera_view.unwrap();
        std::fs::write(
            output.join(&file).with_extension("capture.json"),
            serde_json::to_vec_pretty(
                &json!({"size":[1440,960],"viewport":[0,0,1440,960],"attached_camera":{"eye_m":camera.eye_m,"forward":camera.forward,"up":camera.up},"output_time_s":index as f64/60.}),
            )?,
        )?;
        frames.push(evidence);
        if index % 60 == 59 {
            println!(
                "POV: {} unique rasterized frames / {:.1} simulated seconds",
                index + 1,
                (index + 1) as f64 / 60.
            );
        }
    }
    if frames.len() < 60 {
        return Err(format!("track ended before one second of capture: {stop_error:?}").into());
    }
    if std::fs::read(&input)? != original || s.editor.project() != &saved {
        return Err("recording changed source or document".into());
    }
    let evidence = json!({"source":input,"offscreen":true,"viewport_only":true,"native_desktop_tested":false,"input_unchanged":true,"fps":60,"width":1440,"height":960,"dt_s":qx_physics::FIXED_STEP_SECONDS,"duration_s":frames.len() as f64/60.,"frame_count":frames.len(),"stop_error":stop_error,"adapter":recorder.adapter,"physics_step_total_ms":step_total_ms,"physics_step_max_ms":step_max_ms,"frames":frames});
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!(
        "Authentic 60 FPS / 1440x960 POV sequence: {}",
        output.display()
    );
    Ok(())
}
