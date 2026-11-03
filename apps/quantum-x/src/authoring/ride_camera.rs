//! Transient ride view and geometric playback. The same presentation snapshot
//! feeds cameras and GLB/gray-box instances, independently of project persistence.
use super::session::Session;
use qx_document::train::Train;
use qx_geometry::math::{Frame, Quaternion, Vec3};
use qx_physics::{Pose, consist::ConsistInspection};
use qx_rendering::{CameraView, DisplayOptions, InspectionScene};
use std::time::Instant;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Free,
    Rider,
    Front,
    Rear,
    Chase,
    Track,
}
impl Mode {
    const ALL: [Self; 6] = [
        Self::Free,
        Self::Rider,
        Self::Front,
        Self::Rear,
        Self::Chase,
        Self::Track,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Free => "Free Camera",
            Self::Rider => "Rider POV",
            Self::Front => "Front-Car POV",
            Self::Rear => "Rear-Car POV",
            Self::Chase => "Chase Camera",
            Self::Track => "Track Preview",
        }
    }
}
struct Cached {
    train: Train,
    revision: (u64, u64, bool),
    station: f64,
    poses: Result<ConsistInspection, String>,
}
struct Latched {
    frame: u64,
    revision: u64,
    at: Instant,
    sample: super::train_playback::PresentationSample,
}
pub(super) struct State {
    pub mode: Mode,
    pub component: Option<String>,
    pub offset_m: Vec3,
    pub chase_offset_m: Vec3,
    pub rotation_rad: Vec3,
    pub stabilized: bool,
    pub preview_playing: bool,
    pub preview_speed_mps: f64,
    pub error: Option<String>,
    pub presentation: Option<ConsistInspection>,
    pub notice: Option<String>,
    cache: Option<Cached>,
    clock: Option<Instant>,
    frame: Option<u64>,
    profile: Option<usize>,
    saved_modes: [Mode; 2],
    /// Explicit time is used by deterministic capture only; native frames latch
    /// Instant once, after UI command processing, for both vehicle and camera.
    pub capture_time: Option<Instant>,
    pub sample_time: Option<Instant>,
    pub pose_update_ms: f64,
    pub camera_update_ms: f64,
    pub presentation_tick: f64,
    pub interpolation_alpha: f64,
    pub starved: bool,
    physical_sample: Option<Latched>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            mode: Mode::Free,
            component: None,
            offset_m: Vec3::new(0., 1.2, 0.),
            chase_offset_m: Vec3::new(-8., 3., 4.),
            rotation_rad: Vec3::ZERO,
            stabilized: false,
            preview_playing: false,
            preview_speed_mps: 5.,
            error: None,
            presentation: None,
            notice: None,
            cache: None,
            clock: None,
            frame: None,
            profile: None,
            saved_modes: [Mode::Free; 2],
            capture_time: None,
            sample_time: None,
            pose_update_ms: 0.,
            camera_update_ms: 0.,
            presentation_tick: 0.,
            interpolation_alpha: 1.,
            starved: false,
            physical_sample: None,
        }
    }
}

pub(super) fn gray_box_controls(s: &mut Session, ui: &mut egui::Ui) {
    ui.collapsing("Gray-box train · development placeholder",|ui| {
        ui.small("Neutral matte components for articulation and cameras. No finished model is required.");
        ui.add(egui::DragValue::new(&mut s.playback.fixture_count).prefix("Cars ").range(1..=256));
        let ready=s.heartline_draft==s.editor.project().heartline;
        if ui.add_enabled(ready,egui::Button::new("Add and associate gray-box train")).on_hover_text("One undoable transaction. Uses the existing conventional development mechanism; no layout or track geometry changes.").clicked() {add_gray_box(s);}
        if !ready {ui.small("Apply or cancel reference settings before associating a train.");}
    });
}
fn add_gray_box(s: &mut Session) {
    use qx_document::{command::Command, train::DevelopmentArrangement};
    let mut trains = s.editor.project().trains.clone();
    let mut n = 1;
    while trains.iter().any(|t| t.id == format!("train-{n}")) {
        n += 1;
    }
    let id = format!("train-{n}");
    match Train::development_consist(
        id.clone(),
        s.playback.fixture_count,
        DevelopmentArrangement::Conventional,
    ) {
        Ok(train) => {
            trains.push(train);
            let mut reference = s.editor.project().heartline.clone();
            reference.train_configuration_id = Some(id);
            s.execute(vec![
                Command::SetTrains(trains),
                Command::SetHeartline(reference),
            ]);
        }
        Err(e) => s.status = e.to_string(),
    }
}

/// Pure attachment math, including independently rotating passenger assemblies.
pub(super) fn attached(
    pose: Pose,
    offset: Vec3,
    rotation: Vec3,
    chase: bool,
    stabilized: bool,
) -> Result<CameraView, String> {
    let body = Frame::new(pose.orientation).map_err(|e| e.to_string())?;
    let q = Quaternion::axis_angle(Vec3::Y, rotation.y)
        .and_then(|q| {
            Ok(q.compose(Quaternion::axis_angle(Vec3::Z, rotation.z)?)
                .compose(Quaternion::axis_angle(Vec3::X, rotation.x)?))
        })
        .map_err(|e| e.to_string())?;
    let basis = Frame::new(pose.orientation.compose(q)).map_err(|e| e.to_string())?;
    let eye_m = pose.position_m + body.local_to_world(offset);
    let forward = if chase {
        (pose.position_m + body.tangent() * 2. - eye_m)
            .normalized()
            .map_err(|e| e.to_string())?
    } else {
        basis.tangent()
    };
    let mut up = if chase && stabilized {
        Vec3::Y
    } else {
        basis.normal()
    };
    if up.cross(forward).length() < 1e-6 {
        // Horizon is undefined looking vertically; retain the body's lateral
        // reference there rather than normalizing a zero vector or flipping it.
        up = body.lateral().cross(forward);
        if up.length() < 1e-6 {
            up = Vec3::Z;
        }
    }
    CameraView { eye_m, forward, up }.validate()
}

pub(super) fn controls(s: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    ui.label("Ride camera · F6 cycles modes");
    egui::ComboBox::from_id_salt("ride camera mode")
        .selected_text(s.ride.mode.label())
        .show_ui(ui, |ui| {
            for mode in Mode::ALL {
                ui.selectable_value(&mut s.ride.mode, mode, mode.label());
            }
        });
    let train = s
        .editor
        .project()
        .heartline
        .train_configuration_id
        .as_ref()
        .and_then(|id| s.editor.project().trains.iter().find(|t| &t.id == id));
    if matches!(s.ride.mode, Mode::Rider | Mode::Chase) {
        let candidates: Vec<_> = train
            .into_iter()
            .flat_map(|t| &t.components)
            .filter(|c| {
                matches!(
                    c.kind,
                    qx_document::train::ComponentKind::CarBody
                        | qx_document::train::ComponentKind::PassengerAssembly
                )
            })
            .map(|c| c.id.clone())
            .collect();
        egui::ComboBox::from_id_salt("camera component ID")
            .selected_text(s.ride.component.as_deref().unwrap_or("First car body"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.ride.component, None, "First car body");
                for id in candidates {
                    ui.selectable_value(&mut s.ride.component, Some(id.clone()), id);
                }
            });
    }
    if s.ride.mode != Mode::Free {
        ui.collapsing("Camera local positioning (metres)", |ui| {
            let offset = if s.ride.mode == Mode::Chase {
                &mut s.ride.chase_offset_m
            } else {
                &mut s.ride.offset_m
            };
            for (label, value) in [
                ("Forward X", &mut offset.x),
                ("Up Y", &mut offset.y),
                ("Lateral Z", &mut offset.z),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.add(
                        egui::DragValue::new(value)
                            .speed(0.01)
                            .range(-1000. ..=1000.),
                    );
                });
            }
            if s.ride.mode != Mode::Chase {
                for (label, value) in [
                    ("Roll (rad)", &mut s.ride.rotation_rad.x),
                    ("Yaw (rad)", &mut s.ride.rotation_rad.y),
                    ("Pitch (rad)", &mut s.ride.rotation_rad.z),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(
                            egui::DragValue::new(value)
                                .speed(0.01)
                                .range(-std::f64::consts::TAU..=std::f64::consts::TAU),
                        );
                    });
                }
            }
        });
        if s.ride.mode == Mode::Chase {
            ui.checkbox(&mut s.ride.stabilized, "Stabilize chase horizon");
        }
    }
    ui.separator();
    ui.label("Geometric inspection / constant-speed preview");
    ui.small(
        "Preview follows geometry; it is not a physics simulation. Open endpoints stop playback.",
    );
    ui.add_enabled_ui(!s.playback.active(), |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if s.ride.preview_playing {
                    "Pause preview"
                } else {
                    "Play preview"
                })
                .clicked()
            {
                s.ride.preview_playing = !s.ride.preview_playing;
                s.ride.clock = Some(Instant::now());
                s.ride.notice = None;
            }
            ui.add(
                egui::DragValue::new(&mut s.ride.preview_speed_mps)
                    .prefix("m/s ")
                    .speed(0.1)
                    .range(-100. ..=100.),
            );
        });
    });
    if s.development {
        super::train_playback_ui::controls(s, ui, scene);
    } else {
        if let Some(error) = &s.playback.error {
            ui.colored_label(super::theme::YELLOW, error);
        }
        if let Some(p) = &s.playback.snapshot {
            ui.label(format!(
                "Experimental physics: {} · {:.3} s",
                if p.running { "Running" } else { "Paused" },
                p.ticks as f64 * qx_physics::FIXED_STEP_SECONDS
            ));
        }
        ui.collapsing("Experimental physics playback", |ui| {
            super::train_playback_ui::controls(s, ui, scene)
        });
    }
    if let Some(error) = &s.ride.error {
        ui.colored_label(super::theme::YELLOW, error);
    }
    if let Some(notice) = &s.ride.notice {
        ui.label(notice);
    }
}

pub(super) fn present(
    s: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) {
    display.asset_instances.clear();
    display.overlay_lines.clear();
    display.train_bounds = None;
    display.selected_car_bounds = None;
    let profile = s.workbench.active.index();
    if let Some(previous) = s.ride.profile
        && previous != profile
    {
        s.ride.saved_modes[previous] = s.ride.mode;
        s.ride.mode = s.ride.saved_modes[profile];
    }
    s.ride.profile = Some(profile);
    display.camera_view = None;
    s.ride.error = None;
    if s.workbench.active != super::workbench::Profile::Train
        && s.ride.mode == Mode::Free
        && !s.playback.active()
        && !s.ride.preview_playing
    {
        s.ride.presentation = None;
        return;
    }
    let track = match scene.train_track() {
        Ok(track) => track,
        Err(e) => {
            s.ride.presentation = None;
            s.ride.preview_playing = false;
            s.ride.error = Some(e);
            return;
        }
    };
    let update_start = Instant::now();
    let frame = ctx.cumulative_frame_nr();
    let now = if s.ride.frame == Some(frame) {
        s.ride.sample_time.unwrap_or(update_start)
    } else {
        s.ride.capture_time.unwrap_or(update_start)
    };
    s.ride.sample_time = Some(now);
    if s.ride.frame != Some(frame) {
        s.ride.frame = Some(frame);
        let elapsed = s
            .ride
            .clock
            .replace(now)
            .map_or(0., |t| now.saturating_duration_since(t).as_secs_f64());
        if s.playback.active() {
            s.ride.preview_playing = false;
        }
        if s.ride.preview_playing {
            let station = s.train_station_m.unwrap_or(track.length_m() * 0.5)
                + s.ride.preview_speed_mps * elapsed.min(0.25);
            if !(0. ..=track.length_m()).contains(&station) {
                s.ride.preview_playing = false;
                s.ride.notice = Some(
                    "Preview reached an open track endpoint; choose another inspection station."
                        .into(),
                );
            } else {
                s.train_station_m = Some(station);
            }
        }
    }
    let station = s.train_station_m.unwrap_or(track.length_m() * 0.5);
    display.inspection_distance_m = Some(station);
    let project = s.editor.project();
    let train = project
        .heartline
        .train_configuration_id
        .as_ref()
        .and_then(|id| project.trains.iter().find(|t| &t.id == id));
    s.ride.presentation = None;
    if let Some(train) = train {
        let physical = if s.playback.snapshot.is_some() {
            if let Some(latched) = &s.ride.physical_sample
                && latched.frame == frame
                && latched.revision == s.revision
                && latched.at == now
            {
                Some(latched.sample.clone())
            } else {
                let sample = s.playback.presentation_sample(now);
                s.ride.physical_sample = sample.clone().map(|sample| Latched {
                    frame,
                    revision: s.revision,
                    at: now,
                    sample,
                });
                sample
            }
        } else {
            None
        };
        let poses = if let Some(p) = physical {
            s.ride.presentation_tick = p.tick;
            s.ride.interpolation_alpha = p.alpha;
            s.ride.starved = p.starved;
            Ok(p.poses)
        } else {
            let mut mechanical = train.clone();
            for c in &mut mechanical.components {
                c.asset = None;
            }
            let revision = (s.revision, s.visible_revision, s.visible_preview);
            if !s.ride.cache.as_ref().is_some_and(|c| {
                c.train == mechanical && c.revision == revision && c.station == station
            }) {
                s.ride.cache = Some(Cached {
                    train: mechanical,
                    revision,
                    station,
                    poses: qx_physics::consist::inspect(train, track, station),
                });
            }
            s.ride.cache.as_ref().unwrap().poses.clone()
        };
        match poses {
            Ok(poses) => {
                display.inspection_distance_m = Some(if s.playback.active() {
                    poses.components.first().map_or(station, |p| p.station_m)
                } else {
                    station
                });
                super::train_preview::populate(train, track, &poses, &mut s.assets, display);
                s.ride.presentation = Some(poses);
                if !s.ride.preview_playing {
                    s.ride.error = None;
                }
            }
            Err(e) => {
                s.ride.preview_playing = false;
                s.ride.error = Some(format!(
                    "Train cannot be placed at {station:.3} m: {e}. Scrub to a station with room for the entire consist; inspect component offsets and joint dimensions if closure still fails."
                ));
                if s.development
                    && let Ok(seed) = qx_physics::consist::diagnostic_seed(train, track, station)
                {
                    super::train_preview::populate(train, track, &seed, &mut s.assets, display);
                    s.ride.error = Some(format!(
                        "{} · DEVELOPMENT DIAGNOSTIC: unclosed seed shown; physics and ride attachment unavailable at this station.",
                        s.ride.error.as_deref().unwrap_or("Assembly failed")
                    ));
                }
            }
        }
    }
    s.ride.pose_update_ms = update_start.elapsed().as_secs_f64() * 1000.;
    if s.ride.preview_playing {
        ctx.request_repaint();
    }
    if s.ride.error.is_some() && s.ride.mode != Mode::Track {
        return;
    }
    let camera_start = Instant::now();
    let view = if s.ride.mode == Mode::Free {
        return;
    } else if s.ride.mode == Mode::Track {
        scene
            .track
            .sample_at_distance(station)
            .map_err(|e| e.to_string())
            .and_then(|p| {
                attached(
                    Pose {
                        position_m: p.position_m,
                        orientation: p.frame.orientation(),
                    },
                    s.ride.offset_m,
                    s.ride.rotation_rad,
                    false,
                    false,
                )
            })
    } else {
        camera_pose(&s.ride, train).and_then(|pose| {
            attached(
                pose,
                if s.ride.mode == Mode::Chase {
                    s.ride.chase_offset_m
                } else {
                    s.ride.offset_m
                },
                s.ride.rotation_rad,
                s.ride.mode == Mode::Chase,
                s.ride.stabilized,
            )
        })
    };
    match view {
        Ok(view) => display.camera_view = Some(view),
        Err(e) => s.ride.error = Some(e),
    }
    s.ride.camera_update_ms = camera_start.elapsed().as_secs_f64() * 1000.;
    if s.ride.preview_playing || s.playback.snapshot.as_ref().is_some_and(|p| p.running) {
        ctx.request_repaint();
    }
}
fn camera_pose(state: &State, train: Option<&Train>) -> Result<Pose, String> {
    let train = train.ok_or("Associate a train in Configuration to use a ride camera.")?;
    let id = match state.mode {
        Mode::Rear => train.cars.last().map(|c| c.body_id.as_str()),
        Mode::Front => train.cars.first().map(|c| c.body_id.as_str()),
        _ => state
            .component
            .as_deref()
            .or_else(|| train.cars.first().map(|c| c.body_id.as_str())),
    }
    .ok_or("Train has no car body for this camera.")?;
    state.presentation.as_ref().and_then(|p|p.components.iter().find(|p|p.id==id)).map(|p|p.pose)
        .ok_or_else(||format!("Camera attachment {id} is unavailable; choose a valid component and inspection station."))
}

#[cfg(test)]
#[path = "ride_camera_tests.rs"]
mod tests;
