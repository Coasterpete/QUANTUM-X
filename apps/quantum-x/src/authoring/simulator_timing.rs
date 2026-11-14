//! `--gui-smoke-test simulator-timing <project.qx.json> <play-seconds>` (M4K.1).
//!
//! Drives the real Simulator composition in the native hidden-window event loop
//! (production worker, egui, wgpu) with the same commands as the UI, and
//! records worker scheduling, frame freshness and worker CPU. Run it twice,
//! with and without `QX_PLAYBACK_WAIT=legacy`, for a before/after comparison.
//!
//! Sequence: enter → Play for `play-seconds` of wall time → Pause → hold paused
//! → Reset (must equal the initial poses) → Return to Editor (editor state must
//! be unchanged) → re-enter and close the Simulator **while running**, several
//! times → replay the same number of ticks offline with `TrackSimulation` and
//! require bit-identical poses. A simulation error (for example an open track
//! end) ends the run early; Reset must then recover it.
//!
//! The hidden loop draws on a ~5 ms poll, not display vsync, so frame
//! freshness is evidence about worker pacing, not a display benchmark.
use super::{playback_clock::Histogram, session::Session, simulator, simulator_smoke::Baseline};
use qx_physics::{FIXED_STEP_SECONDS, TrackSimulation, consist::ConsistInspection};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const PAUSED_HOLD: Duration = Duration::from_secs(3);
const CLOSE_WHILE_RUNNING_CYCLES: u32 = 3;
const CYCLE_TICKS: u64 = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    WaitReady,
    Entering,
    Running,
    Pausing,
    PausedHold,
    Resetting,
    CycleEnter,
    CycleRunning,
    Done,
}

pub(super) struct TimingScript {
    play: Duration,
    phase: Phase,
    baseline: Option<Baseline>,
    station_m: Option<f64>,
    initial: Option<ConsistInspection>,
    play_at: Option<Instant>,
    cpu_at_play: Option<f64>,
    frames: u64,
    fresh: u64,
    starved: u64,
    last_tick: u64,
    last_frame: Option<Instant>,
    frame_interval: Histogram,
    running: Option<Value>,
    run_error: Option<String>,
    paused_poses: Option<(u64, ConsistInspection)>,
    pause_at: Option<Instant>,
    cpu_at_pause: Option<f64>,
    paused_ticks_stable: bool,
    paused: Option<Value>,
    reset_matches_initial: Option<bool>,
    editor_restored: Vec<bool>,
    cycles_done: u32,
    determinism: Option<Value>,
}

impl TimingScript {
    pub fn new(play_seconds: f64) -> Self {
        Self {
            play: Duration::from_secs_f64(play_seconds),
            phase: Phase::WaitReady,
            baseline: None,
            station_m: None,
            initial: None,
            play_at: None,
            cpu_at_play: None,
            frames: 0,
            fresh: 0,
            starved: 0,
            last_tick: 0,
            last_frame: None,
            frame_interval: Histogram::default(),
            running: None,
            run_error: None,
            paused_poses: None,
            pause_at: None,
            cpu_at_pause: None,
            paused_ticks_stable: true,
            paused: None,
            reset_matches_initial: None,
            editor_restored: vec![],
            cycles_done: 0,
            determinism: None,
        }
    }

    fn request_entry(&mut self, s: &mut Session) -> Result<(), String> {
        let ready = simulator::readiness(s);
        if let Err(e) = &ready
            && s.visible_revision == s.revision
            && !s.visible_preview
            && !s.has_drafts()
            && s.file_dialog.is_none()
        {
            return Err(format!("entry refused: {e}"));
        }
        if ready.is_ok() {
            if self.baseline.is_none() {
                self.baseline = Some(Baseline::capture(s));
            }
            s.simulator.entry_requested = true;
            self.phase = if self.phase == Phase::WaitReady {
                Phase::Entering
            } else {
                Phase::CycleRunning
            };
        }
        Ok(())
    }

    fn check_restored(&mut self, s: &Session) -> Result<(), String> {
        let restored = self.baseline.as_ref() == Some(&Baseline::capture(s))
            && !s.simulator.active()
            && !s.playback.active();
        self.editor_restored.push(restored);
        if restored {
            Ok(())
        } else {
            Err("editor state was not restored exactly after leaving the Simulator".into())
        }
    }

    /// Advances after each native draw. `Ok(true)` while work remains.
    pub fn advance(&mut self, s: &mut Session) -> Result<bool, String> {
        let snap = s.playback.snapshot.as_ref();
        let state = snap.map(|p| (p.ticks, p.running, p.timing.thread_cpu_ms));
        match self.phase {
            Phase::WaitReady => self.request_entry(s)?,
            Phase::Entering => {
                if let Some(e) = &s.simulator.error {
                    return Err(format!("entry failed: {e}"));
                }
                if s.simulator.active() {
                    let (ticks, running, cpu) = state.ok_or("simulator without snapshot")?;
                    if ticks != 0 || running {
                        return Err("simulator did not enter paused at tick 0".into());
                    }
                    self.initial = s.playback.snapshot.as_ref().map(|p| p.poses.clone());
                    self.cpu_at_play = cpu;
                    simulator::toggle_play(s);
                    self.play_at = Some(Instant::now());
                    self.phase = Phase::Running;
                }
            }
            Phase::Running => {
                let now = Instant::now();
                if let Some((ticks, true, _)) = state {
                    self.frames += 1;
                    self.fresh += u64::from(ticks > self.last_tick);
                    self.starved += u64::from(s.ride.starved);
                    self.last_tick = ticks;
                    if let Some(previous) = self.last_frame {
                        self.frame_interval
                            .record(now.duration_since(previous).as_secs_f64() * 1000.);
                    }
                    self.last_frame = Some(now);
                }
                if let Some(e) = s.playback.error.clone() {
                    // Error recovery path: stop measuring, keep the stopped state.
                    self.run_error = Some(e);
                    self.capture_running(s);
                    self.capture_paused_poses(s);
                    simulator::reset(s);
                    self.phase = Phase::Resetting;
                } else if self
                    .play_at
                    .is_some_and(|t| now.duration_since(t) >= self.play)
                {
                    self.capture_running(s);
                    simulator::toggle_play(s);
                    self.phase = Phase::Pausing;
                }
            }
            Phase::Pausing => {
                if let Some((_, false, cpu)) = state {
                    self.capture_paused_poses(s);
                    self.pause_at = Some(Instant::now());
                    self.cpu_at_pause = cpu;
                    self.phase = Phase::PausedHold;
                }
            }
            Phase::PausedHold => {
                let paused_tick = self.paused_poses.as_ref().map(|p| p.0);
                if state.map(|p| p.0) != paused_tick || state.is_some_and(|p| p.1) {
                    self.paused_ticks_stable = false;
                }
                if self.pause_at.is_some_and(|t| t.elapsed() >= PAUSED_HOLD) {
                    // Reset is also the probe that makes the blocked worker
                    // publish its thread CPU time for the paused interval.
                    simulator::reset(s);
                    self.phase = Phase::Resetting;
                }
            }
            Phase::Resetting => {
                if let Some((0, false, cpu)) = state {
                    let held = self.pause_at.map(|t| t.elapsed().as_secs_f64());
                    let cpu_ms = cpu.zip(self.cpu_at_pause).map(|(b, a)| b - a);
                    self.paused = Some(json!({
                        "hold_s": held,
                        "ticks_unchanged": self.paused_ticks_stable,
                        "worker_cpu_ms": cpu_ms,
                        "worker_cpu_percent_of_one_core": cpu_ms.zip(held).map(|(c, h)| c / (h * 10.)),
                    }));
                    let poses = s.playback.snapshot.as_ref().map(|p| &p.poses);
                    self.reset_matches_initial = Some(self.initial.as_ref() == poses);
                    if self.reset_matches_initial != Some(true) {
                        return Err("reset did not restore the initial poses".into());
                    }
                    if s.playback.error.is_some() {
                        return Err("reset did not clear the simulation error".into());
                    }
                    simulator::exit(s);
                    self.check_restored(s)?;
                    self.phase = Phase::CycleEnter;
                }
            }
            Phase::CycleEnter => {
                if self.cycles_done >= CLOSE_WHILE_RUNNING_CYCLES {
                    self.determinism = Some(self.replay_offline(s)?);
                    self.phase = Phase::Done;
                } else {
                    self.request_entry(s)?;
                }
            }
            Phase::CycleRunning => {
                if let Some(e) = &s.simulator.error {
                    return Err(format!("re-entry failed: {e}"));
                }
                if s.simulator.active() {
                    match state {
                        Some((0, false, _)) => simulator::toggle_play(s),
                        Some((ticks, true, _)) if ticks >= CYCLE_TICKS => {
                            // Close the Simulator while the worker is running.
                            simulator::exit(s);
                            self.check_restored(s)?;
                            self.cycles_done += 1;
                            self.phase = Phase::CycleEnter;
                        }
                        _ => {}
                    }
                }
            }
            Phase::Done => return Ok(false),
        }
        if self.phase == Phase::Entering && self.station_m.is_none() {
            // The same station rule `simulator::request_entry` applies.
            self.station_m = s.train_station_m;
        }
        Ok(true)
    }

    fn capture_paused_poses(&mut self, s: &Session) {
        self.paused_poses = s
            .playback
            .snapshot
            .as_ref()
            .map(|p| (p.ticks, p.poses.clone()));
    }

    fn capture_running(&mut self, s: &Session) {
        let Some(p) = &s.playback.snapshot else {
            return;
        };
        let t = &p.timing;
        let wall = t.segment_wall_s;
        let cpu_ms = t.thread_cpu_ms.zip(self.cpu_at_play).map(|(b, a)| b - a);
        let telemetry = &p.telemetry;
        self.running = Some(json!({
            "mechanism": t.mechanism.map(|m| m.label()),
            "wall_s": wall,
            "simulated_s": t.segment_simulated_s,
            "ticks": p.ticks,
            "drift_ms": t.segment_drift_ms(),
            "simulated_to_wall_ratio": t.segment_simulated_s / wall.max(f64::EPSILON),
            "steps_per_second": t.steps as f64 / wall.max(f64::EPSILON),
            "wakes": t.wakes,
            "wake_interval": t.wake_interval.summary(),
            "wake_interval_below_5ms_fraction": t.wake_interval.fraction_below(5.),
            "wake_lateness": t.wake_lateness.summary(),
            "steps_per_wake_0_to_8": t.steps_per_wake,
            "catch_up_limit_events": p.lag_events,
            "catch_up_limit_events_per_second": p.lag_events as f64 / wall.max(f64::EPSILON),
            "dropped_wall_ms": t.dropped_wall_ms,
            "step_duration": t.step_duration.summary(),
            "step_mean_ms": t.step_total_ms / (t.steps.max(1) as f64),
            "over_budget_steps": t.over_budget_steps,
            "worker_cpu_ms": cpu_ms,
            "worker_cpu_percent_of_one_core": cpu_ms.map(|c| c / (wall.max(f64::EPSILON) * 10.)),
            "telemetry": {
                "speed_mps": telemetry.speed_mps,
                "distance_traveled_m": telemetry.distance_traveled_m,
                "longitudinal_acceleration_mps2": telemetry.longitudinal_acceleration_mps2,
                "applied_device_force_n": telemetry.applied_device_force_n,
            },
        }));
        let frames = self.frames.max(1) as f64;
        if let Some(r) = self.running.as_mut() {
            r["frames"] = json!({
                "running_frames": self.frames,
                "fresh_physics_percent": 100. * self.fresh as f64 / frames,
                "starved_presentation_percent": 100. * self.starved as f64 / frames,
                "frame_interval": self.frame_interval.summary(),
            });
        }
    }

    /// Replays the paused tick count with the production constructor the worker
    /// uses and requires bit-identical poses: wait timing must not change state.
    fn replay_offline(&self, s: &Session) -> Result<Value, String> {
        let (ticks, live) = self
            .paused_poses
            .clone()
            .ok_or("no paused poses recorded")?;
        let project = s.editor.project();
        let id = project
            .heartline
            .train_configuration_id
            .as_ref()
            .ok_or("no associated train")?;
        let mut train = project
            .trains
            .iter()
            .find(|t| &t.id == id)
            .ok_or("associated train missing")?
            .clone();
        for c in &mut train.components {
            c.asset = None;
        }
        let scene = super::session::scene(project, &s.catalog, s.options)?;
        let track = scene.train_track()?.clone();
        let station = self.station_m.unwrap_or(track.length_m() * 0.5);
        let operations = project.operations.clone();
        let track = std::sync::Arc::new(track);
        let mut sim = if s.playback.experimental_velocity {
            TrackSimulation::with_experimental_velocity_initialization(
                train, track, station, operations,
            )
        } else {
            TrackSimulation::with_operations(train, track, station, operations)
        }?;
        let start = Instant::now();
        let mut offline_error = None;
        while sim.ticks() < ticks {
            if let Err(e) = sim.step() {
                offline_error = Some(e);
                break;
            }
        }
        let identical = sim.ticks() == ticks && sim.snapshot()? == live;
        if !identical {
            return Err(format!(
                "live worker state at tick {ticks} differs from offline replay (offline tick {}, error {offline_error:?})",
                sim.ticks()
            ));
        }
        Ok(json!({
            "ticks": ticks,
            "simulated_s": ticks as f64 * FIXED_STEP_SECONDS,
            "bit_identical_poses": identical,
            "offline_stopped_with": offline_error,
            "offline_replay_ms": start.elapsed().as_secs_f64() * 1000.,
        }))
    }

    pub fn report(&self) -> Value {
        json!({
            "name": "simulator_timing",
            "phase": format!("{:?}", self.phase),
            "play_seconds_requested": self.play.as_secs_f64(),
            "running": self.running,
            "run_error": self.run_error,
            "paused": self.paused,
            "reset_matches_initial": self.reset_matches_initial,
            "editor_restored": self.editor_restored,
            "closed_while_running_cycles": self.cycles_done,
            "determinism": self.determinism,
        })
    }
}
