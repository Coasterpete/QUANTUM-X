//! DEVELOPMENT-ONLY Simulator Mode composition (M4K).
//!
//! No owner-approved Simulator layout exists yet. This composition is a
//! replaceable shell over real services: every control dispatches an existing
//! playback/camera action, and every number is read from the simulation worker.
//! Placement, styling, shortcuts and camera selection UX await owner design.
use super::{
    ride_camera::Mode as Camera,
    session::Session,
    simulator::{self, CAMERAS, NAVIGATION_BANK},
};
use qx_rendering::{DisplayOptions, InspectionScene, SurfaceOutput, ViewCommand};

const STANDARD_GRAVITY_MPS2: f64 = 9.80665;

pub(super) fn workspace(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) -> SurfaceOutput {
    super::theme::install(ctx);
    ctx.set_zoom_factor(session.workbench.window.ui_scale);
    super::train_playback::update(session, ctx);
    simulator::poll(session);
    if !session.simulator.active() {
        // Invalidated this frame: the Editor composition takes over at once.
        return super::panels::workspace(session, ctx, scene, display);
    }
    if let Some(message) = session.assets.poll(session.editor.project(), &session.path) {
        session.status = message;
    }
    if session.assets.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    simulator::shortcuts(session, ctx);
    let mut leave = false;
    let mut view_command = None;
    let mut viewport = egui::Rect::NOTHING;
    ctx.root_ui(|root| {
        egui::Panel::top("simulator mode bar").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("SIMULATOR MODE")
                        .strong()
                        .color(super::theme::TRAIN),
                );
                ui.small("development composition · provisional layout");
                ui.separator();
                ui.label(egui::RichText::new(&session.editor.project().metadata.name).strong());
                ui.small(session.save_state()).on_hover_text(
                    "The authored document is unchanged by simulation. Save from the Editor.",
                );
                ui.separator();
                if ui
                    .button("Return to Editor")
                    .on_hover_text(
                        "Retire this simulation and restore the Editor exactly as it was left.",
                    )
                    .clicked()
                {
                    leave = true;
                }
            });
        });
        egui::Panel::right("simulator controls")
            .default_size(320.)
            .min_size(240.)
            .resizable(true)
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    view_command = controls(session, ui, display);
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| {
                let rect = ui.available_rect_before_wrap().intersect(ui.clip_rect());
                ui.allocate_rect(rect, egui::Sense::hover());
                ui.painter().text(
                    rect.left_top() + egui::vec2(10., 8.),
                    egui::Align2::LEFT_TOP,
                    format!("Simulated rigid-body poses · {}", session.ride.mode.label()),
                    egui::FontId::monospace(11.),
                    egui::Color32::LIGHT_GRAY,
                );
                viewport = rect;
            });
    });
    if !ctx.egui_wants_keyboard_input()
        && viewport.contains(ctx.pointer_hover_pos().unwrap_or(egui::Pos2::ZERO))
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F))
    {
        view_command = Some(ViewCommand::FitTrain);
    }
    if leave {
        simulator::exit(session);
        // Hand the renderer back to the Editor bank with no simulated packet.
        display.asset_instances.clear();
        display.overlay_lines.clear();
        display.camera_view = None;
        display.train_bounds = None;
        display.selected_car_bounds = None;
        return SurfaceOutput {
            viewport,
            view_command: None,
            close: session.close_approved,
            window_command: None,
            profile: session.workbench.active.index(),
            reset_navigation: false,
        };
    }
    // Simulator presents the physical pose packet only: no editor selection,
    // hover, support highlight or inspection marker.
    display.selected = None;
    display.hovered = None;
    display.selected_support = None;
    display.inspection_marker = false;
    super::ride_camera::present(session, ctx, scene, display);
    display.inspection_distance_m = None;
    if !session.development {
        display.overlay_lines.clear();
    }
    if view_command.is_some() {
        session.ride.mode = Camera::Free;
        display.camera_view = None;
    }
    SurfaceOutput {
        viewport,
        view_command,
        close: session.close_approved,
        window_command: None,
        profile: NAVIGATION_BANK,
        reset_navigation: false,
    }
}

fn controls(
    s: &mut Session,
    ui: &mut egui::Ui,
    display: &mut DisplayOptions,
) -> Option<ViewCommand> {
    let mut command = None;
    super::theme::heading(ui, "PLAYBACK", super::theme::TRAIN);
    let running = simulator::running(s);
    let ready = s.playback.snapshot.is_some() && !s.playback.pending;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                ready && (running || s.playback.error.is_none()),
                egui::Button::new(if running { "Pause" } else { "Play" }),
            )
            .on_hover_text("Play/Pause · Ctrl+Space")
            .clicked()
        {
            simulator::toggle_play(s);
        }
        if ui
            .add_enabled(ready, egui::Button::new("Reset"))
            .on_hover_text("Pause and restore the initial bodies and guide hints exactly.")
            .clicked()
        {
            simulator::reset(s);
        }
    });
    if let Some(e) = &s.playback.error {
        ui.colored_label(super::theme::YELLOW, format!("Simulation stopped: {e}"));
        ui.small("Reset restores the initial state; Return to Editor is always available.");
    }
    if let Some(p) = &s.playback.snapshot {
        ui.label(format!(
            "{} · tick {} · {:.4} s simulated",
            if p.running { "Running" } else { "Paused" },
            p.ticks,
            p.ticks as f64 * qx_physics::FIXED_STEP_SECONDS
        ));
        ui.small("Fixed 1/240 s physics step, independent of frame rate.");
        ui.separator();
        super::theme::heading(ui, "TELEMETRY", super::theme::TRAIN);
        let t = &p.telemetry;
        egui::Grid::new("simulator telemetry")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let mut row = |name: &str, value: String| {
                    ui.label(name);
                    ui.monospace(value);
                    ui.end_row();
                };
                row(
                    "Speed",
                    format!("{:.3} m/s · {:.2} km/h", t.speed_mps, t.speed_mps * 3.6),
                );
                row(
                    "Distance traveled",
                    format!("{:.3} m", t.distance_traveled_m),
                );
                row(
                    "Longitudinal accel.",
                    format!("{:+.3} m/s²", t.longitudinal_acceleration_mps2),
                );
                let (f, g) = (t.local_specific_force_mps2, STANDARD_GRAVITY_MPS2);
                row("Specific force, normal", format!("{:+.3} g", f.y / g));
                row("Specific force, lateral", format!("{:+.3} g", f.z / g));
                row("Specific force, long.", format!("{:+.3} g", f.x / g));
                row(
                    "Device / resistance",
                    format!(
                        "{:.1} N / {:.1} N",
                        t.applied_device_force_n, t.resistance_force_n
                    ),
                );
            });
        ui.small(
            "Speed and specific force are mass-weighted over the train's rigid bodies in \
             body-local axes; they are not a rider-seat accelerometer.",
        );
        if let Some(d) = &p.diagnostics {
            ui.small(format!(
                "Constraint residuals: joints {:.2e} m · guides {:.2e} m · angular {:.2e} rad",
                d.max_joint_error_m, d.max_guide_error_m, d.max_angular_error_rad
            ));
        }
        if p.lag_events > 0 {
            ui.small(format!(
                "{} wall-clock catch-up limits reached; simulation time is retained, not skipped.",
                p.lag_events
            ));
        }
        if s.ride.starved && p.running {
            ui.small("Presentation waiting for the next committed step.");
        }
        if let Some(r) = &p.velocity_initialization {
            ui.small(format!(
                "M4J experimental velocity initialization: lead guide {:.6} m/s, residual {:.2e}.",
                r.achieved_speed_mps, r.max_residual
            ));
        }
    }
    ui.separator();
    super::theme::heading(ui, "CAMERA", super::theme::TRAIN);
    egui::ComboBox::from_id_salt("simulator camera")
        .selected_text(s.ride.mode.label())
        .show_ui(ui, |ui| {
            for mode in CAMERAS {
                ui.selectable_value(&mut s.ride.mode, mode, mode.label());
            }
        })
        .response
        .on_hover_text("F6 cycles cameras. Attached cameras follow simulated body poses.");
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                display.train_bounds.is_some(),
                egui::Button::new("Fit Train"),
            )
            .on_hover_text("Free camera framing the simulated train · F over viewport")
            .clicked()
        {
            command = Some(ViewCommand::FitTrain);
        }
        if ui.button("Fit All").clicked() {
            command = Some(ViewCommand::FitAll);
        }
    });
    egui::ComboBox::from_id_salt("simulator track display")
        .selected_text(display.mode.label())
        .show_ui(ui, |ui| {
            for mode in [
                qx_rendering::DisplayMode::Rendered,
                qx_rendering::DisplayMode::RenderedCenterline,
                qx_rendering::DisplayMode::Technical,
                qx_rendering::DisplayMode::Wireframe,
            ] {
                ui.selectable_value(&mut display.mode, mode, mode.label());
            }
        });
    if let Some(e) = &s.ride.error {
        ui.colored_label(super::theme::YELLOW, e);
    }
    ui.separator();
    ui.small(
        "Model: experimental rigid bodies on ideal bilateral centerline guides. No wheel \
         contact, collision, wind or validated vehicle-family dynamics. Not a ride-safety tool.",
    );
    command
}

/// Editor-side entry feedback: a preparation modal with Cancel, and the last
/// entry/runtime failure. Shown over the unchanged Editor composition.
pub(super) fn editor_feedback(s: &mut Session, ctx: &egui::Context) {
    if s.simulator.preparing() {
        egui::Modal::new(egui::Id::unique("simulator preparing")).show(ctx, |ui| {
            ui.heading("Preparing Simulator");
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Seating the train on the simulation worker…");
            });
            if ui.button("Cancel").clicked() {
                simulator::exit(s);
            }
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    } else if s.simulator.error.is_some() {
        let mut close = false;
        egui::Modal::new(egui::Id::unique("simulator entry error")).show(ctx, |ui| {
            ui.heading("Simulator unavailable");
            ui.label(s.simulator.error.as_deref().unwrap_or_default());
            ui.small("The project, history and editor layout are unchanged.");
            close = ui.button("OK").clicked();
        });
        if close {
            s.simulator.error = None;
        }
    }
}
