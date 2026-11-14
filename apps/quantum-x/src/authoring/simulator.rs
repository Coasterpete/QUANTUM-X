//! M4K Simulator Mode: a transient application mode over the one authoritative
//! [`Session`]. It owns no document, physics engine or worker of its own.
//!
//! * The authored project, undo/redo history, drafts, selection, graph view and
//!   docking layout stay in the live `Session` and are never written here.
//! * Physics runs on the existing `train_playback` worker (fixed 1/240 s steps,
//!   epoch-tagged requests, reset snapshot). Entering prepares a fresh
//!   simulation; leaving retires it. There is never a second worker or clock.
//! * Train meshes and attached cameras come from the existing
//!   `ride_camera::present` path, so one latched pose sample feeds both.
//!
//! Phases: `Editor → Preparing → Simulator → Editor`. Entry fails closed: any
//! readiness or initialization error leaves the application in Editor with an
//! actionable message and no partly initialized Simulator.
use super::{ride_camera::Mode as Camera, session::Session, train_playback::Action};
use qx_rendering::InspectionScene;

/// Renderer navigation bank used while the Simulator composition is shown.
/// Banks 0/1 remain the Track/Train editor banks and are restored on return.
pub(super) const NAVIGATION_BANK: usize = 2;
const _: () = assert!(NAVIGATION_BANK < qx_rendering::NAVIGATION_BANKS);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Editor,
    /// Entry requested; waiting for the worker to acknowledge a valid initial
    /// state for the current epoch. The editor composition stays visible.
    Preparing,
    Simulator,
}

/// Editor ride-camera state captured on entry and restored on return.
#[derive(Clone, Copy, Debug)]
struct EditorCamera {
    mode: Camera,
}

/// Camera modes offered in the Simulator. Every one follows the simulated
/// physical pose packet; the geometric Track Preview is deliberately excluded.
pub(super) const CAMERAS: [Camera; 5] = [
    Camera::Free,
    Camera::Rider,
    Camera::Front,
    Camera::Rear,
    Camera::Chase,
];

#[derive(Default)]
pub(super) struct State {
    phase: Phase,
    /// Set by the command bar (which has no scene); handled in the workspace pass.
    pub entry_requested: bool,
    /// Last entry/preparation/runtime failure, shown in the Editor after return.
    pub error: Option<String>,
    /// Authored revision the active simulation was prepared from.
    revision: Option<u64>,
    editor_camera: Option<EditorCamera>,
    /// Simulator camera choice, kept for the next entry in this session only.
    camera: Camera,
}
impl State {
    /// True while the Simulator composition owns the viewport.
    pub fn active(&self) -> bool {
        self.phase == Phase::Simulator
    }
    pub fn preparing(&self) -> bool {
        self.phase == Phase::Preparing
    }
    /// Editor viewport interaction (selection, inspection hover) is ignored
    /// while preparing or simulating so editor state is preserved exactly.
    pub fn blocks_editor_input(&self) -> bool {
        self.phase != Phase::Editor
    }
}

/// Entry preconditions that need no generated scene. Ordered so the first
/// failure names the step the user must take.
pub(super) fn readiness(s: &Session) -> Result<(), String> {
    if s.file_dialog.is_some() || s.project_prompt_open() || s.graph_drag.is_some() {
        return Err("Finish the open dialog or graph edit first.".into());
    }
    if s.has_drafts() {
        return Err(format!(
            "Apply or cancel pending drafts before simulating: {}",
            s.draft_names().join("; ")
        ));
    }
    if s.draft_error.is_some() {
        return Err("Resolve the track generation error before simulating.".into());
    }
    if s.visible_revision != s.revision || s.visible_preview {
        return Err("Track generation for the committed document is still running.".into());
    }
    let project = s.editor.project();
    let Some(id) = &project.heartline.train_configuration_id else {
        return Err(
            "No train is associated. Choose one in Train Workspace › Rider reference.".into(),
        );
    };
    if !project.trains.iter().any(|t| &t.id == id) {
        // The ID may name a nominal reference configuration, not a saved train.
        return Err(format!(
            "Rider reference '{id}' is not a saved train. Associate a saved train in \
             Train Workspace › Rider reference to simulate."
        ));
    }
    Ok(())
}

/// Requests Simulator entry from the committed document. Returns false and
/// records an actionable error when entry is not possible.
pub(super) fn request_entry(s: &mut Session, scene: &InspectionScene) -> bool {
    if s.simulator.phase != Phase::Editor {
        return false;
    }
    s.simulator.error = None;
    if let Err(e) = readiness(s) {
        s.simulator.error = Some(e);
        return false;
    }
    let track = match scene.train_track() {
        Ok(track) => track,
        Err(e) => {
            s.simulator.error = Some(format!("Physical track reference unavailable: {e}"));
            return false;
        }
    };
    s.simulator.editor_camera = Some(EditorCamera { mode: s.ride.mode });
    // Geometric preview is an editor inspection tool, never Simulator motion.
    s.ride.preview_playing = false;
    // One worker, one session: any editor development playback is retired and
    // a fresh simulation is prepared from the current committed inputs.
    s.playback.discard();
    s.playback.error = None;
    s.playback.initial_speed_mps = s.editor.project().operations.physical.initial_speed_mps;
    let station = s.train_station_m.unwrap_or(track.length_m() * 0.5);
    s.playback
        .initialize(s.editor.project(), s.options, track, station);
    if !s.playback.pending {
        let e = s
            .playback
            .error
            .take()
            .unwrap_or_else(|| "Simulation could not be prepared.".into());
        fail(s, e);
        return false;
    }
    s.simulator.revision = Some(s.revision);
    s.simulator.phase = Phase::Preparing;
    true
}

/// Advances preparation and watches for invalidation. Call once per frame after
/// `playback.sync`.
pub(super) fn poll(s: &mut Session) {
    match s.simulator.phase {
        Phase::Editor => {}
        Phase::Preparing => {
            if s.simulator.revision != Some(s.revision) {
                fail(
                    s,
                    "The document changed during preparation; entry canceled.".into(),
                );
            } else if !s.playback.pending {
                if s.playback.snapshot.is_some() && s.playback.error.is_none() {
                    s.simulator.phase = Phase::Simulator;
                    s.ride.mode = s.simulator.camera;
                } else {
                    let e =
                        s.playback.error.take().unwrap_or_else(|| {
                            "Simulation worker returned no initial state.".into()
                        });
                    fail(s, format!("Simulator could not start: {e}"));
                }
            }
        }
        Phase::Simulator => {
            // `playback.sync` retires the session if mechanics/track inputs
            // changed. Never present a Simulator without a live simulation.
            if !s.playback.active() || s.simulator.revision != Some(s.revision) {
                let e = s.playback.error.take().unwrap_or_else(|| {
                    "Simulation inputs changed; the simulation was retired.".into()
                });
                exit(s);
                s.simulator.error = Some(e);
            }
        }
    }
}

/// Cancels preparation, or leaves an active Simulator. Always safe to call.
pub(super) fn exit(s: &mut Session) {
    if s.simulator.phase == Phase::Simulator && CAMERAS.contains(&s.ride.mode) {
        s.simulator.camera = s.ride.mode;
    }
    if s.simulator.phase != Phase::Editor {
        // `discard` clears the worker (stopping any stepping) and advances the
        // epoch, so late packets from this simulation are ignored.
        s.playback.discard();
        s.playback.error = None;
    }
    if let Some(camera) = s.simulator.editor_camera.take() {
        s.ride.mode = camera.mode;
    }
    s.simulator.phase = Phase::Editor;
    s.simulator.revision = None;
    s.simulator.entry_requested = false;
}

fn fail(s: &mut Session, error: String) {
    exit(s);
    s.simulator.error = Some(error);
}

/// Simulator keyboard commands. Play/Pause keeps the existing playback
/// shortcut; F6 cycles physical cameras only.
pub(super) fn shortcuts(s: &mut Session, ctx: &egui::Context) {
    if ctx.egui_wants_keyboard_input() || s.playback.pending {
        return;
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6)) {
        let i = CAMERAS.iter().position(|c| *c == s.ride.mode).unwrap_or(0);
        s.ride.mode = CAMERAS[(i + 1) % CAMERAS.len()];
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Space)) {
        toggle_play(s);
    }
}

pub(super) fn running(s: &Session) -> bool {
    s.playback.snapshot.as_ref().is_some_and(|p| p.running)
}

pub(super) fn toggle_play(s: &mut Session) {
    let running = running(s);
    if running {
        s.playback.action(Action::Pause);
    } else if s.playback.error.is_none() && s.playback.snapshot.is_some() {
        s.playback.action(Action::Play);
    }
}

pub(super) fn reset(s: &mut Session) {
    if s.playback.snapshot.is_some() {
        s.playback.action(Action::Reset);
    }
}
