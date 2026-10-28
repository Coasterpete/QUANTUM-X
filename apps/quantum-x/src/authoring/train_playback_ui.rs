//! Replaceable developer controls inside the owner's existing train workspace.
use super::{session::Session, train_playback::Action};

pub(super) fn controls(s: &mut Session, ui: &mut egui::Ui, scene: &qx_rendering::InspectionScene) {
    ui.heading("Rigid-body playback · development controls");
    let active = s.playback.active();
    ui.add_enabled(
        !active,
        egui::DragValue::new(&mut s.playback.initial_speed_mps)
            .speed(0.1)
            .range(-100. ..=100.)
            .prefix("Initial speed m/s "),
    );
    let ready = s.visible_revision == s.revision && !s.visible_preview && s.draft_error.is_none();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                ready && !s.playback.pending,
                egui::Button::new("Initialize playback"),
            )
            .clicked()
        {
            let station = s.train_station_m.unwrap_or(scene.track.length_m() * 0.5);
            s.playback
                .initialize(s.editor.project(), s.options, &scene.track, station);
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
