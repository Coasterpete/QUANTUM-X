//! Transient playback worker. Fixed simulation steps never block egui or alter the document.
use super::session::Session;
use qx_document::{Project, train::Train};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    solver::{GeneratedTrack, GenerationOptions},
};
use qx_physics::{Diagnostics, FIXED_STEP_SECONDS, TrackSimulation, consist::ConsistInspection};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, PartialEq)]
struct Source {
    train: Train,
    initial: InitialConditions,
    sections: Vec<Section>,
    options: GenerationOptions,
    operations: qx_document::operations::Operations,
    heartline: qx_document::HeartlineSettings,
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
            operations: project.operations.clone(),
            heartline: project.heartline.clone(),
        })
    }
}
#[derive(Clone, Debug)]
pub(super) struct WorkerTiming {
    pub steps: u64,
    pub step_total_ms: f64,
    pub step_max_ms: f64,
    pub over_budget_steps: u64,
    pub lateness_max_ms: f64,
}
impl Default for WorkerTiming {
    fn default() -> Self {
        Self {
            steps: 0,
            step_total_ms: 0.,
            step_max_ms: 0.,
            over_budget_steps: 0,
            lateness_max_ms: 0.,
        }
    }
}
impl WorkerTiming {
    fn record(&mut self, start: Instant, deadline: Instant) {
        let end = Instant::now();
        let ms = end.duration_since(start).as_secs_f64() * 1000.;
        self.steps += 1;
        self.step_total_ms += ms;
        self.step_max_ms = self.step_max_ms.max(ms);
        self.over_budget_steps += u64::from(ms > FIXED_STEP_SECONDS * 1000.);
        self.lateness_max_ms = self
            .lateness_max_ms
            .max(end.saturating_duration_since(deadline).as_secs_f64() * 1000.);
    }
}
#[derive(Clone, Debug)]
pub(super) struct TimedPose {
    pub at: Instant,
    pub tick: u64,
    pub poses: Arc<ConsistInspection>,
}
pub(super) const PRESENTATION_DELAY_TICKS: f64 = 2.;
fn remember(timeline: &mut VecDeque<TimedPose>, s: &TrackSimulation, at: Instant) {
    if let Ok(poses) = s.snapshot() {
        timeline.push_back(TimedPose {
            at,
            tick: s.ticks(),
            poses: Arc::new(poses),
        });
        while timeline.len() > 8 {
            timeline.pop_front();
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub poses: ConsistInspection,
    pub previous: ConsistInspection,
    pub committed_at: Instant,
    pub ticks: u64,
    pub running: bool,
    pub diagnostics: Option<Diagnostics>,
    pub speed_mps: Vec<f64>,
    pub lag_events: u64,
    pub error: Option<String>,
    pub telemetry: qx_physics::devices::TrainTelemetry,
    pub timing: WorkerTiming,
    pub timeline: Vec<TimedPose>,
}
enum Request {
    Load {
        epoch: u64,
        train: Box<Train>,
        track: Arc<GeneratedTrack>,
        station: f64,
        operations: Box<qx_document::operations::Operations>,
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
            let mut committed_at = Instant::now();
            let mut timing = WorkerTiming::default();
            let mut collapse_pair = true;
            let mut timeline = VecDeque::new();
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
                        operations,
                    }) => {
                        epoch = e;
                        running = false;
                        diagnostics = None;
                        error = None;
                        lag_events = 0;
                        timing = WorkerTiming::default();
                        collapse_pair = true;
                        committed_at = Instant::now();
                        match TrackSimulation::with_operations(*train, track, station, *operations)
                        {
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
                                    if !running {
                                        committed_at = Instant::now();
                                        collapse_pair = true;
                                        next = committed_at + dt;
                                        running = true;
                                    }
                                }
                                Action::Pause => running = false,
                                Action::Step => {
                                    running = false;
                                    if error.is_none() {
                                        let start = Instant::now();
                                        match s.step() {
                                            Ok(d) => {
                                                diagnostics = Some(d);
                                                committed_at = Instant::now();
                                                remember(&mut timeline, s, committed_at);
                                                timing.record(start, start);
                                                collapse_pair = false;
                                            }
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
                                    timing = WorkerTiming::default();
                                    collapse_pair = true;
                                    committed_at = Instant::now();
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
                                let start = Instant::now();
                                match s.step() {
                                    Ok(d) => {
                                        diagnostics = Some(d);
                                        // Timestamp is the fixed-step deadline, not variable solver
                                        // completion time. Its pair spans exactly one simulation dt.
                                        committed_at = next;
                                        remember(&mut timeline, s, committed_at);
                                        timing.record(start, next);
                                        collapse_pair = false;
                                    }
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
                                committed_at = Instant::now();
                                collapse_pair = true;
                                next = committed_at + dt;
                            }
                        }
                    }
                }
                if let Some(s) = &simulation {
                    if collapse_pair {
                        timeline.clear();
                        remember(&mut timeline, s, committed_at);
                    }
                    let value = s.snapshot().map(|poses| Snapshot {
                        previous: if collapse_pair {
                            poses.clone()
                        } else {
                            s.previous_snapshot().clone()
                        },
                        poses,
                        committed_at,
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
                        telemetry: s.telemetry().clone(),
                        timing: timing.clone(),
                        timeline: timeline.iter().cloned().collect(),
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
    #[cfg(test)]
    pub fn presentation(&self, now: Instant) -> Option<ConsistInspection> {
        self.presentation_sample(now).map(|p| p.poses)
    }
    pub fn presentation_sample(&self, now: Instant) -> Option<PresentationSample> {
        let s = self.snapshot.as_ref()?;
        if !s.running {
            return Some(PresentationSample {
                poses: s.poses.clone(),
                tick: s.ticks as f64,
                alpha: 1.,
                starved: false,
            });
        }
        let target = now
            .checked_sub(Duration::from_secs_f64(
                PRESENTATION_DELAY_TICKS * FIXED_STEP_SECONDS,
            ))
            .unwrap_or(now);
        if let Some(first) = s.timeline.first() {
            if target <= first.at {
                return Some(PresentationSample {
                    poses: first.poses.as_ref().clone(),
                    tick: first.tick as f64,
                    alpha: 0.,
                    starved: false,
                });
            }
            for pair in s.timeline.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if target <= b.at {
                    let span = b.at.duration_since(a.at).as_secs_f64();
                    if span <= 0. {
                        return None;
                    }
                    let alpha = (target.duration_since(a.at).as_secs_f64() / span).clamp(0., 1.);
                    return Some(PresentationSample {
                        poses: qx_physics::consist::interpolate(&a.poses, &b.poses, alpha).ok()?,
                        tick: a.tick as f64 + (b.tick - a.tick) as f64 * alpha,
                        alpha,
                        starved: false,
                    });
                }
            }
            let last = s.timeline.last()?;
            return Some(PresentationSample {
                poses: last.poses.as_ref().clone(),
                tick: last.tick as f64,
                alpha: 1.,
                starved: true,
            });
        }
        // Deterministic offline packets carry just one committed pair.
        let previous_at = s
            .committed_at
            .checked_sub(Duration::from_secs_f64(FIXED_STEP_SECONDS))
            .unwrap_or(s.committed_at);
        let alpha = (target.saturating_duration_since(previous_at).as_secs_f64()
            / FIXED_STEP_SECONDS)
            .clamp(0., 1.);
        Some(PresentationSample {
            poses: qx_physics::consist::interpolate(&s.previous, &s.poses, alpha).ok()?,
            tick: s.ticks.saturating_sub(1) as f64 + alpha,
            alpha,
            starved: target > s.committed_at,
        })
    }

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
            operations: Box::new({
                let mut op = source.operations.clone();
                op.physical.initial_speed_mps = self.initial_speed_mps;
                op
            }),
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
        if self.source.is_some() && self.source != Source::new(project, options) {
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
#[derive(Clone)]
pub(super) struct PresentationSample {
    pub poses: ConsistInspection,
    pub tick: f64,
    pub alpha: f64,
    pub starved: bool,
}
pub(super) fn update(s: &mut Session, ctx: &egui::Context) {
    if s.workbench.active == super::workbench::Profile::Train
        && !ctx.egui_wants_keyboard_input()
        && !s.project_prompt_open()
        && s.file_dialog.is_none()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6))
    {
        use super::ride_camera::Mode;
        s.ride.mode = match s.ride.mode {
            Mode::Free => Mode::Rider,
            Mode::Rider => Mode::Front,
            Mode::Front => Mode::Rear,
            Mode::Rear => Mode::Chase,
            Mode::Chase => Mode::Track,
            Mode::Track => Mode::Free,
        };
    }
    s.playback.sync(s.editor.project(), s.options);
    if s.playback.active()
        && (s.draft_dirty()
            || s.initial_draft != s.editor.project().initial
            || s.heartline_draft != s.editor.project().heartline)
    {
        s.playback.discard();
        s.playback.error =
            Some("Track draft changed. Apply or cancel it before initializing playback.".into());
    }
    if s.playback.pending || s.playback.active() {
        if s.playback.snapshot.as_ref().is_some_and(|p| p.running) {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
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
