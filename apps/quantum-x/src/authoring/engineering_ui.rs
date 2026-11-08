use super::{plot::Plot, session::Session};
use egui::{Color32, Stroke};
use qx_rendering::InspectionScene;
const NAMES: [(&str, &str); 14] = [
    ("Elevation", "m"),
    ("Curvature", "1/m"),
    ("Radius of curvature", "m"),
    ("Bank / roll (wrapped)", "rad"),
    ("Pitch", "rad"),
    ("Heading (wrapped)", "rad"),
    ("Authored design speed", "m/s"),
    ("Normal G at design speed", "g"),
    ("Lateral G at design speed", "g"),
    ("Longitudinal G at design speed", "g"),
    ("Kinematic acceleration magnitude", "m/s²"),
    ("Specific force magnitude / mass", "N/kg"),
    ("Normal force at design speed", "N"),
    ("Lateral force at design speed", "N"),
];
pub(super) fn show(s: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Author transitions").clicked() {
            s.expansion.engineering = false;
        }
        ui.strong("Engineering plots");
        egui::ComboBox::from_id_salt("engineering channel")
            .selected_text(NAMES[s.expansion.engineering_channel].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in NAMES.iter().enumerate() {
                    ui.selectable_value(&mut s.expansion.engineering_channel, i, *name);
                }
            });
        ui.checkbox(&mut s.expansion.whole_track, "Whole track");
    });
    ui.small("Authored design-speed estimates + kinematic geometry. No measured physics data in these curves. Click pins a station; hover inspects in 3D.");
    if scene.track.sections.len() != s.preview_project().sections.len()
        || s.visible_revision != s.revision
    {
        ui.label("Updating shared geometry…");
        return;
    }
    if s.expansion
        .engineering_cache
        .as_ref()
        .is_none_or(|(revision, gravity, _)| {
            *revision != s.visible_revision || *gravity != s.initial_draft.gravity_mps2
        })
    {
        match qx_geometry::engineering::snapshot(&scene.track, s.initial_draft.gravity_mps2) {
            Ok(rows) => {
                s.expansion.engineering_cache = Some((
                    s.visible_revision,
                    s.initial_draft.gravity_mps2,
                    std::sync::Arc::new(rows),
                ))
            }
            Err(e) => {
                ui.label(e.to_string());
                return;
            }
        }
    }
    let rows = s.expansion.engineering_cache.as_ref().unwrap().2.clone();
    let train = s
        .editor
        .project()
        .heartline
        .train_configuration_id
        .as_ref()
        .and_then(|id| s.editor.project().trains.iter().find(|t| &t.id == id));
    let mass = train.map(|t| t.components.iter().map(|c| c.mass_kg).sum::<f64>());
    let channel = s.expansion.engineering_channel;
    let value = |r: &qx_geometry::engineering::EngineeringRow| -> Option<f64> {
        let g = &r.geometry;
        Some(match channel {
            0 => g.elevation_m,
            1 => g.curvature_per_m,
            2 => r.radius_m?,
            3 => g.bank_rad?,
            4 => g.pitch_rad,
            5 => g.yaw_rad,
            6 => g.reference_speed_mps,
            7 => g.normal_g,
            8 => g.lateral_g,
            9 => r.longitudinal_g,
            10 => r.acceleration_mps2.length(),
            11 => r.specific_force_mps2.length(),
            12 => g.normal_g * s.initial_draft.gravity_mps2 * mass?,
            _ => g.lateral_g * s.initial_draft.gravity_mps2 * mass?,
        })
    };
    if channel >= 12 {
        ui.label(
            mass.map_or("Associate a train to show forces in N.".into(), |m| {
                format!("Associated train mass: {m:.1} kg")
            }),
        );
    }
    let rows: Vec<_> = rows
        .iter()
        .filter(|r| s.expansion.whole_track || r.geometry.section_id == s.selected)
        .collect();
    let values: Vec<_> = rows
        .iter()
        .filter_map(|r| value(r).filter(|v| v.is_finite()))
        .collect();
    let height = (ui.available_height() - 80.).max(70.);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(140.), height),
        egui::Sense::click(),
    );
    if values.is_empty() || rows.len() < 2 {
        ui.label("Undefined here (straight-track radius or vertical banking) / no samples.");
        return;
    }
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let margin = ((hi - lo) * 0.08).max(0.005);
    let x = [
        rows[0].geometry.distance_m,
        rows.last().unwrap().geometry.distance_m,
    ];
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(80., 15.),
        rect.max - egui::vec2(20., 26.),
    );
    let Some(mapping) = Plot::new(plot, x, [lo - margin, hi + margin]) else {
        return;
    };
    mapping.paint_grid(ui, rect, "Track station m", NAMES[channel].1);
    // Undefined points split curves; never bridge an infinite radius interval.
    let mut points = vec![];
    for r in &rows {
        if let Some(v) = value(r).filter(|v| v.is_finite()) {
            points.push(mapping.map(r.geometry.distance_m, v));
        } else if !points.is_empty() {
            ui.painter().with_clip_rect(plot).add(egui::Shape::line(
                std::mem::take(&mut points),
                Stroke::new(1.8, super::theme::YELLOW),
            ));
        }
    }
    if !points.is_empty() {
        ui.painter().with_clip_rect(plot).add(egui::Shape::line(
            points,
            Stroke::new(1.8, super::theme::YELLOW),
        ));
    }
    let hover = response
        .hover_pos()
        .filter(|p| plot.contains(*p))
        .map(|p| x[0] + (x[1] - x[0]) * ((p.x - plot.min.x) / plot.width()) as f64);
    if response.clicked() {
        s.expansion.selected_station_m = hover;
    }
    let station = hover
        .or(s.viewport_inspection.map(|(_, station)| station))
        .or(s.expansion.selected_station_m)
        .filter(|v| *v >= x[0] && *v <= x[1]);
    if let Some(station) = station {
        let r = rows
            .iter()
            .min_by(|a, b| {
                (a.geometry.distance_m - station)
                    .abs()
                    .total_cmp(&(b.geometry.distance_m - station).abs())
            })
            .unwrap();
        let g = &r.geometry;
        s.inspect_input = Some((g.section_id, g.input));
        s.hovered = Some(g.section_id);
        let px = mapping.map(station, lo).x;
        ui.painter().line_segment(
            [egui::pos2(px, plot.top()), egui::pos2(px, plot.bottom())],
            Stroke::new(1., Color32::WHITE),
        );
        ui.monospace(format!(
            "Station {:.3} m · {}: {} {} · region {}",
            g.distance_m,
            NAMES[channel].0,
            value(r).map_or("undefined".into(), |v| format!("{v:.5}")),
            NAMES[channel].1,
            g.section_id.0
        ));
        if response.clicked() && s.selected != g.section_id {
            s.select(g.section_id);
        }
    }
    if let Some(p) = &s.playback.snapshot {
        let t = &p.telemetry;
        ui.small(format!("Measured experimental playback (separate): t {:.3} s · velocity {:.3} m/s · acceleration {:.3} m/s² · devices {:.1} N",
            t.elapsed_s,t.speed_mps,t.acceleration_mps2.length(),t.applied_device_force_n));
    }
}
