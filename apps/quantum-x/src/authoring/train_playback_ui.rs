//! Replaceable developer controls inside the owner's existing train workspace.
use super::{session::Session, train_playback::Action};

pub(super) fn controls(s: &mut Session, ui: &mut egui::Ui, scene: &qx_rendering::InspectionScene) {
    ui.heading("Rigid-body playback · development controls");
    if s.playback.experimental_velocity {
        ui.small("M4J experimental velocity initialization · lead running-gear speed contract");
    }
    if let Some(r) = s
        .playback
        .snapshot
        .as_ref()
        .and_then(|p| p.velocity_initialization.as_ref())
    {
        let component = s
            .editor
            .project()
            .heartline
            .train_configuration_id
            .as_ref()
            .and_then(|id| s.editor.project().trains.iter().find(|t| &t.id == id))
            .and_then(|t| t.components.get(r.reference.body))
            .map_or("unknown", |c| c.id.as_str());
        ui.small(format!(
            "Lead {component} guide {}: {:.9} m/s; residual {:.2e}; {} sweeps",
            r.reference_guide, r.achieved_speed_mps, r.max_residual, r.sweeps
        ));
    }
    let active = s.playback.active();
    super::operations_ui::physical(s, ui);
    if s.expansion.operations != s.editor.project().operations {
        super::expansion::actions(s, ui);
    }
    s.playback.initial_speed_mps = s.editor.project().operations.physical.initial_speed_mps;
    let ready = s.visible_revision == s.revision
        && !s.visible_preview
        && s.draft_error.is_none()
        && !s.expansion.dirty(s.editor.project());
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                ready && !s.playback.pending,
                egui::Button::new("Initialize playback"),
            )
            .clicked()
        {
            match scene.train_track() {
                Ok(track) => {
                    let station = s.train_station_m.unwrap_or(track.length_m() * 0.5);
                    s.playback
                        .initialize(s.editor.project(), s.options, track, station);
                }
                Err(e) => s.playback.error = Some(e),
            }
        }
        if ui
            .add_enabled(active, egui::Button::new("Return to inspection"))
            .clicked()
        {
            s.playback.discard();
        }
    });
    let running = s.playback.snapshot.as_ref().is_some_and(|p| p.running);
    let initialized = s.playback.snapshot.is_some() && !s.playback.pending;
    ui.horizontal_wrapped(|ui| {
        for (label, action, enabled) in [
            (
                if running { "Pause" } else { "Play" },
                if running { Action::Pause } else { Action::Play },
                initialized && (running || s.playback.error.is_none()),
            ),
            (
                "Single step",
                Action::Step,
                initialized && !running && s.playback.error.is_none(),
            ),
            ("Reset simulation", Action::Reset, initialized),
        ] {
            if ui.add_enabled(enabled, egui::Button::new(label)).on_hover_text("Play/Pause: Ctrl+Space. Single step: Ctrl+Shift+Space. Reset restores the initial bodies and guide hints.").clicked() {
                s.playback.action(action);
            }
        }
    });
    if s.playback.pending {
        ui.spinner();
        ui.small("Preparing rigid bodies on simulation worker…");
    }
    if let Some(e) = &s.playback.error {
        ui.colored_label(egui::Color32::LIGHT_YELLOW, e);
    }
    if let Some(p) = &s.playback.snapshot {
        ui.label(format!(
            "{} · tick {} · {:.6} s · 240 Hz",
            if p.running { "Running" } else { "Paused" },
            p.ticks,
            p.ticks as f64 * qx_physics::FIXED_STEP_SECONDS
        ));
        let t = &p.telemetry;
        ui.monospace(format!("Measured longitudinal acceleration {:+.4} m/s²\nMass-averaged specific G: normal {:+.4} · lateral {:+.4} · longitudinal {:+.4}",t.longitudinal_acceleration_mps2,
            t.local_specific_force_mps2.y/9.80665,t.local_specific_force_mps2.z/9.80665,t.local_specific_force_mps2.x/9.80665));
        ui.monospace(format!("Velocity {:.3} m/s ({:.2} km/h) · traveled {:.3} m\nAcceleration {:.3} m/s² · specific force {:.3} N/kg\nDevice force {:.1} N · resistance {:.1} N", t.speed_mps,t.speed_mps*3.6,t.distance_traveled_m,t.acceleration_mps2.length(),t.specific_force_mps2.length(),t.applied_device_force_n,t.resistance_force_n));
        ui.collapsing("Device command / response", |ui| {
            for d in &t.devices {
                ui.monospace(format!(
                    "Device {} · {} occupied masses · command {:+.3} m/s² · {:.1}/{:.1} N{}",
                    d.id,
                    d.occupied_bodies,
                    d.commanded_acceleration_mps2,
                    d.applied_force_n,
                    d.requested_force_n,
                    if d.force_limited {
                        " · force limited"
                    } else {
                        ""
                    }
                ));
            }
        });
        if let Some(d) = &p.diagnostics {
            ui.monospace(format!(
                "Residuals: joints {:.6} m / guides {:.6} m / angular {:.6} rad",
                d.max_joint_error_m, d.max_guide_error_m, d.max_angular_error_rad
            ));
            ui.small(format!(
                "{} joint, {} guide and {} angular reactions",
                d.joint_force_on_b_n.len(),
                d.guide_force_n.len(),
                d.angular_torque_on_b_nm.len()
            ));
            ui.collapsing("Constraint reactions (fixed-step averages)", |ui| {
                for (i, f) in d.joint_force_on_b_n.iter().enumerate() {
                    ui.monospace(format!("Joint {}: {:.3} N", i + 1, f.length()));
                }
                for (i, f) in d.guide_force_n.iter().enumerate() {
                    ui.monospace(format!("Guide {}: {:.3} N", i + 1, f.length()));
                }
                for (i, f) in d.angular_torque_on_b_nm.iter().enumerate() {
                    ui.monospace(format!("Angular {}: {:.3} Nm", i + 1, f.length()));
                }
            });
        }
        ui.collapsing("Simulated component speeds", |ui| {
            for (c, v) in p.poses.components.iter().zip(&p.speed_mps) {
                ui.monospace(format!("{}: {:.6} m/s", c.id, v));
            }
        });
        if p.lag_events > 0 {
            ui.small(format!(
                "{} wall-clock catch-up limits reached; simulation time retained",
                p.lag_events
            ));
        }
    }
    ui.small("Experimental bilateral centerline guides; no wheel contact, collision or validated vehicle-family dynamics.");
    ui.separator();
}
