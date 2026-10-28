//! Transient playback worker. Fixed simulation steps never block egui or alter the document.
use super::session::Session;
use qx_document::{Project, train::Train};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    solver::{GeneratedTrack, GenerationOptions},
};
use qx_physics::{Diagnostics, FIXED_STEP_SECONDS, TrackSimulation, consist::ConsistInspection};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq)]
struct Source {
    train: Train,
    initial: InitialConditions,
    sections: Vec<Section>,
    options: GenerationOptions,
}
impl Source {
    fn new(project: &Project, options: GenerationOptions) -> Option<Self> {
        let id = project.heartline.train_configuration_id.as_ref()?;
        let mut train = project.trains.iter().find(|t| &t.id == id)?.clone();
        // Visual replacement, pivot and scale do not restart or modify the mechanics.
        for c in &mut train.components {
            c.asset = None;
        }
        Some(Self {
            train,
            initial: project.initial.clone(),
            sections: project.sections.clone(),
            options,
        })
    }
}
#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub poses: ConsistInspection,
    pub ticks: u64,
    pub running: bool,
    pub diagnostics: Option<Diagnostics>,
    pub speed_mps: Vec<f64>,
    pub lag_events: u64,
    pub error: Option<String>,
}
enum Request {
    Load {
        epoch: u64,
        train: Box<Train>,
        track: Arc<GeneratedTrack>,
        station: f64,
        speed: f64,
    },
    Action(u64, Action),
    Clear,
}
#[derive(Clone, Copy)]
pub(super) enum Action {
    Play,
    Pause,
    Step,
    Reset,
}
struct Worker {
    send: mpsc::Sender<Request>,
    latest: Latest,
}
type Latest = Arc<Mutex<Option<(u64, Result<Snapshot, String>)>>>;
impl Worker {
    fn new() -> Self {
        let (send, receive) = mpsc::channel();
        let latest = Arc::new(Mutex::new(None));
        let output = latest.clone();
        std::thread::spawn(move || {
            let dt = Duration::from_secs_f64(FIXED_STEP_SECONDS);
            let mut simulation: Option<TrackSimulation> = None;
            let mut epoch = 0;
            let mut running = false;
            let mut next = Instant::now();
            let mut diagnostics = None;
            let mut error = None;
            let mut lag_events = 0;
            loop {
                let timeout = if running {
                    next.saturating_duration_since(Instant::now())
                } else {
                    Duration::from_secs(60)
                };
                match receive.recv_timeout(timeout) {
                    Ok(Request::Load {
                        epoch: e,
                        train,
                        track,
                        station,
                        speed,
                    }) => {
                        epoch = e;
                        running = false;
                        diagnostics = None;
                        error = None;
                        lag_events = 0;
                        match TrackSimulation::new(*train, track, station, speed) {
                            Ok(s) => simulation = Some(s),
                            Err(e) => {
                                simulation = None;
                                if let Ok(mut slot) = output.lock() {
                                    *slot = Some((epoch, Err(e)));
                                }
                            }
                        }
                    }
                    Ok(Request::Action(e, action)) if e == epoch => {
                        if let Some(s) = &mut simulation {
                            match action {
                                Action::Play if error.is_none() => {
                                    running = true;
                                    next = Instant::now() + dt;
                                }
                                Action::Pause => running = false,
                                Action::Step => {
                                    running = false;
                                    if error.is_none() {
                                        match s.step() {
                                            Ok(d) => diagnostics = Some(d),
                                            Err(e) => error = Some(e),
                                        }
                                    }
                                }
                                Action::Reset => {
                                    s.reset();
                                    running = false;
                                    diagnostics = None;
                                    error = None;
                                    lag_events = 0;
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Request::Clear) => {
                        simulation = None;
                        running = false;
                    }
                    Ok(_) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if running && let Some(s) = &mut simulation {
                            // Bound catch-up work and discard excess wall-clock debt, never enlarge dt.
                            for _ in 0..8 {
                                if Instant::now() < next {
                                    break;
                                }
                                match s.step() {
                                    Ok(d) => diagnostics = Some(d),
                                    Err(e) => {
                                        error = Some(e);
                                        running = false;
                                        break;
                                    }
                                }
                                next += dt;
                            }
                            if running && Instant::now() >= next {
                                lag_events += 1;
                                next = Instant::now() + dt;
                            }
                        }
                    }
                }
                if let Some(s) = &simulation {
                    let value = s.snapshot().map(|poses| Snapshot {
                        poses,
                        ticks: s.ticks(),
                        running,
                        diagnostics: diagnostics.clone(),
                        speed_mps: s
                            .world()
                            .bodies
                            .iter()
                            .map(|b| b.linear_velocity_mps.length())
                            .collect(),
                        lag_events,
                        error: error.clone(),
                    });
                    // Only the newest snapshot is retained; the solver never runs under this lock.
                    if let Ok(mut slot) = output.lock() {
                        *slot = Some((epoch, value));
                    }
                }
            }
        });
        Self { send, latest }
    }
}
pub(super) struct State {
    worker: Option<Worker>,
    source: Option<Source>,
    epoch: u64,
    pub snapshot: Option<Snapshot>,
    pub error: Option<String>,
    pub pending: bool,
    pub initial_speed_mps: f64,
    pub fixture_count: usize,
    pub arrangement: qx_document::train::DevelopmentArrangement,
}
impl Default for State {
    fn default() -> Self {
        Self {
            worker: None,
            source: None,
            epoch: 0,
            snapshot: None,
            error: None,
            pending: false,
            initial_speed_mps: 0.,
            fixture_count: 3,
            arrangement: Default::default(),
        }
    }
}
impl State {
    pub fn active(&self) -> bool {
        self.source.is_some()
    }
    pub fn initialize(
        &mut self,
        project: &Project,
        options: GenerationOptions,
        track: &GeneratedTrack,
        station: f64,
    ) {
        self.discard();
        self.error = None;
        if project.heartline.authored_reference
            != qx_geometry::heartline::AuthoredReference::TrackCenterline
        {
            self.error = Some("playback requires the physical track centerline".into());
            return;
        }
        let Some(source) = Source::new(project, options) else {
            self.error = Some("associate a saved train first".into());
            return;
        };
        let worker = self.worker.get_or_insert_with(Worker::new);
        let request = Request::Load {
            epoch: self.epoch,
            train: Box::new(source.train.clone()),
            track: Arc::new(track.clone()),
            station,
            speed: self.initial_speed_mps,
        };
        if worker.send.send(request).is_err() {
            self.error = Some("simulation worker stopped".into());
            return;
        }
        self.source = Some(source);
        self.pending = true;
    }
    pub fn action(&mut self, action: Action) {
        if let Some(w) = &self.worker
            && w.send.send(Request::Action(self.epoch, action)).is_err()
        {
            self.error = Some("simulation worker stopped".into());
        }
    }
    pub fn discard(&mut self) {
        if let Some(w) = &self.worker {
            let _ = w.send.send(Request::Clear);
        }
        self.epoch = self.epoch.wrapping_add(1);
        self.source = None;
        self.snapshot = None;
        self.pending = false;
    }
    pub fn sync(&mut self, project: &Project, options: GenerationOptions) {
        if self.source.is_some()
            && (self.source != Source::new(project, options)
                || project.heartline.authored_reference
                    != qx_geometry::heartline::AuthoredReference::TrackCenterline)
        {
            self.discard();
            self.error = Some(
                "Mechanical definition, track or association changed. Initialize playback again."
                    .into(),
            );
        }
        if let Some(w) = &self.worker
            && let Ok(mut slot) = w.latest.lock()
            && let Some((epoch, value)) = slot.take()
            && epoch == self.epoch
        {
            self.pending = false;
            match value {
                Ok(s) => {
                    self.error = s.error.clone();
                    self.snapshot = Some(s);
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
}
pub(super) fn update(s: &mut Session, ctx: &egui::Context) {
    s.playback.sync(s.editor.project(), s.options);
    if s.playback.active()
        && (s.draft_dirty()
            || s.initial_draft != s.editor.project().initial
            || s.heartline_draft.authored_reference
                != s.editor.project().heartline.authored_reference)
    {
        s.playback.discard();
        s.playback.error =
            Some("Track draft changed. Apply or cancel it before initializing playback.".into());
    }
    if s.playback.pending || s.playback.active() {
        ctx.request_repaint_after(Duration::from_millis(16));
    }
    if s.workbench.active == super::workbench::Profile::Train
        && !ctx.egui_wants_keyboard_input()
        && !s.project_prompt_open()
        && s.file_dialog.is_none()
        && !s.playback.pending
        && s.playback.snapshot.is_some()
    {
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::Space,
            )
        }) {
            if s.playback.error.is_none() {
                s.playback.action(Action::Step);
            }
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Space)) {
            let running = s.playback.snapshot.as_ref().is_some_and(|p| p.running);
            if running || s.playback.error.is_none() {
                s.playback
                    .action(if running { Action::Pause } else { Action::Play });
            }
        }
    }
}
