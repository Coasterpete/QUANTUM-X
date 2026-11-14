//! `--gui-smoke-test simulator`: a scripted Editor → Simulator → Editor round
//! trip driven through the native hidden-window event loop, wgpu surface and
//! egui path. It uses the same commands as the UI (entry request, Play, Pause,
//! Reset, Return) and checks that the open document returns unchanged.
//! This is automated evidence, not manual graphical verification.
use super::{session::Session, simulator};
use qx_document::{
    Project,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::Section,
    math::{Quaternion, Vec3},
};
use std::time::Instant;

/// Ticks to simulate before pausing (0.2 s at 240 Hz).
const RUN_TICKS: u64 = 48;

/// DEVELOPMENT FIXTURE: a synthetic 100 m straight on a gentle grade with the
/// labeled 3-car development consist. Not a coaster design.
pub(super) fn fixture() -> Project {
    let mut p = super::session::blank_project();
    p.metadata.name = "Simulator smoke fixture (development)".into();
    p.sections[0] = Section::straight(1, "Synthetic smoke straight", 100.);
    p.initial.orientation =
        Quaternion::axis_angle(Vec3::Z, -0.2).expect("finite fixture orientation");
    p.trains.push(
        Train::development_consist("smoke".into(), 3, DevelopmentArrangement::Conventional)
            .expect("development consist"),
    );
    p.heartline.train_configuration_id = Some("smoke".into());
    p
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    WaitReady,
    Entering,
    Running,
    Pausing,
    Resetting,
    Done,
}

/// Editor state that must survive the round trip.
#[derive(PartialEq)]
pub(super) struct Baseline {
    project: Project,
    dirty: bool,
    undo: bool,
    redo: bool,
    path: String,
    selected: u64,
    profile: usize,
    /// Structural (see `simulator_tests::EditorState::layouts`).
    layouts: serde_json::Value,
}
impl Baseline {
    pub(super) fn capture(s: &Session) -> Self {
        Self {
            project: s.editor.project().clone(),
            dirty: s.dirty(),
            undo: s.editor.can_undo(),
            redo: s.editor.can_redo(),
            path: s.path.clone(),
            selected: s.selected.0,
            profile: s.workbench.active.index(),
            layouts: serde_json::to_value(&s.workbench.layouts).unwrap_or_default(),
        }
    }
}

pub(super) struct Script {
    step: Step,
    start: Instant,
    baseline: Option<Baseline>,
    events: Vec<(String, f64)>,
    simulator_frames: u64,
    paused_ticks: Option<u64>,
    paused_time_s: Option<f64>,
    reset_matches_initial: Option<bool>,
    initial: Option<qx_physics::consist::ConsistInspection>,
    editor_restored: Option<bool>,
    /// Latest readiness refusal while waiting to enter.
    blocked: Option<String>,
}
impl Script {
    pub fn new() -> Self {
        Self {
            step: Step::WaitReady,
            start: Instant::now(),
            baseline: None,
            events: vec![],
            simulator_frames: 0,
            paused_ticks: None,
            paused_time_s: None,
            reset_matches_initial: None,
            initial: None,
            editor_restored: None,
            blocked: None,
        }
    }
    fn mark(&mut self, event: &str) {
        let ms = self.start.elapsed().as_secs_f64() * 1000.;
        self.events.push((event.into(), (ms * 10.).round() / 10.));
    }
    /// Advances after each native draw. `Ok(true)` while work remains.
    pub fn advance(&mut self, s: &mut Session) -> Result<bool, String> {
        if s.simulator.active() {
            self.simulator_frames += 1;
        }
        // (ticks, running, simulated seconds) copied so commands can borrow `s`.
        let state = s
            .playback
            .snapshot
            .as_ref()
            .map(|p| (p.ticks, p.running, p.telemetry.elapsed_s));
        match self.step {
            Step::WaitReady => {
                let ready = simulator::readiness(s);
                self.blocked = ready.as_ref().err().cloned();
                // Once committed geometry is visible, a refusal is not transient.
                if let Err(e) = &ready
                    && s.visible_revision == s.revision
                    && !s.visible_preview
                    && !s.has_drafts()
                    && s.file_dialog.is_none()
                {
                    return Err(format!("entry refused: {e}"));
                }
                if ready.is_ok() {
                    self.baseline = Some(Baseline::capture(s));
                    s.simulator.entry_requested = true;
                    self.mark("entry_requested");
                    self.step = Step::Entering;
                }
            }
            Step::Entering => {
                if let Some(e) = &s.simulator.error {
                    return Err(format!("entry failed: {e}"));
                }
                if s.simulator.active() {
                    let (ticks, running, _) = state.ok_or("simulator active without a snapshot")?;
                    if ticks != 0 || running {
                        return Err("simulator did not enter paused at tick 0".into());
                    }
                    self.initial = s.playback.snapshot.as_ref().map(|p| p.poses.clone());
                    self.mark("simulator_active");
                    simulator::toggle_play(s);
                    self.step = Step::Running;
                }
            }
            Step::Running => {
                if let Some(e) = &s.playback.error {
                    return Err(format!("simulation error: {e}"));
                }
                if state.is_some_and(|(ticks, running, _)| running && ticks >= RUN_TICKS) {
                    self.mark("ran_fixed_steps");
                    simulator::toggle_play(s);
                    self.step = Step::Pausing;
                }
            }
            Step::Pausing => {
                if let Some((ticks, false, time_s)) = state {
                    self.paused_ticks = Some(ticks);
                    self.paused_time_s = Some(time_s);
                    self.mark("paused");
                    simulator::reset(s);
                    self.step = Step::Resetting;
                }
            }
            Step::Resetting => {
                if let Some((0, false, _)) = state {
                    let poses = s.playback.snapshot.as_ref().map(|p| &p.poses);
                    self.reset_matches_initial = Some(self.initial.as_ref() == poses);
                    self.mark("reset");
                    simulator::exit(s);
                    self.mark("returned_to_editor");
                    let restored = self.baseline.as_ref() == Some(&Baseline::capture(s));
                    self.editor_restored = Some(restored);
                    self.step = Step::Done;
                    if self.reset_matches_initial != Some(true) {
                        return Err("reset did not restore the initial poses".into());
                    }
                    if !restored || s.simulator.active() || s.playback.active() {
                        return Err("editor state was not restored exactly".into());
                    }
                }
            }
            Step::Done => return Ok(false),
        }
        Ok(true)
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "simulator_round_trip",
            "step": format!("{:?}", self.step),
            "events_ms": self.events,
            "simulator_frames_drawn": self.simulator_frames,
            "paused_ticks": self.paused_ticks,
            "paused_simulated_time_s": self.paused_time_s,
            "reset_matches_initial": self.reset_matches_initial,
            "editor_restored": self.editor_restored,
            "entry_blocked_by": self.blocked,
        })
    }
}

/// The scripted native Simulator run selected by `--gui-smoke-test`.
pub(super) enum Driver {
    RoundTrip(Box<Script>),
    Timing(Box<super::simulator_timing::TimingScript>),
}
impl Driver {
    pub fn advance(&mut self, s: &mut Session) -> Result<bool, String> {
        match self {
            Self::RoundTrip(script) => script.advance(s),
            Self::Timing(script) => script.advance(s),
        }
    }
    pub fn report(&self) -> serde_json::Value {
        match self {
            Self::RoundTrip(script) => script.report(),
            Self::Timing(script) => script.report(),
        }
    }
}
