use super::session::Session;
use egui::{Color32, Pos2, Sense, Stroke};
use qx_geometry::transition::Function;
use qx_rendering::InspectionScene;

pub(super) fn editor(session: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    ui.strong("Transition Editor — development controls");
    egui::ScrollArea::horizontal()
        .id_salt("section timeline")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let sections = session.editor.project().sections.clone();
                for section in sections {
                    let text = format!(
                        "{}: {}  {:.3} {:?}",
                        section.id.0, section.name, section.extent, section.domain
                    );
                    if ui
                        .selectable_label(session.selected == section.id, text)
                        .clicked()
                    {
                        session.select(section.id);
                    }
                }
            });
        });
    ui.horizontal(|ui| {
        for key in qx_document::editing::channel_keys(&session.draft) {
            if ui
                .selectable_label(session.channel == key, format!("{key:?}"))
                .clicked()
            {
                session.channel = key;
                session.transition = 0;
            }
        }
        ui.label("Y range");
        ui.add(egui::DragValue::new(&mut session.graph_min).speed(0.01));
        ui.add(egui::DragValue::new(&mut session.graph_max).speed(0.01));
        if ui.button("Fit values").clicked() {
            let channel = session.draft.channel_mut(session.channel).unwrap();
            let values = channel.transitions.iter().flat_map(|t| {
                (0..=32).filter_map(|i| t.evaluate(t.length * i as f64 / 32.).ok().map(|e| e.value))
            });
            let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
            for value in values {
                min = min.min(value);
                max = max.max(value);
            }
            let margin = ((max - min) * 0.15).max(0.01);
            session.graph_min = min - margin;
            session.graph_max = max + margin;
        }
    });
    if let Err(error) = session
        .draft
        .channel_mut(session.channel)
        .unwrap()
        .validate()
    {
        ui.colored_label(
            Color32::YELLOW,
            format!("Invalid draft graph: {error}. Fix numeric inputs or Cancel."),
        );
        numeric_transition(session, ui);
        derived(session, ui, scene);
        return;
    }
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 110.), Sense::click());
    let plot = rect.shrink2(egui::vec2(45., 10.));
    let domain = session.draft.domain;
    let channel = session.draft.channel_mut(session.channel).unwrap();
    let extent = channel.length();
    let min = if session.graph_min.is_finite() {
        session.graph_min
    } else {
        -1.
    };
    let max = if session.graph_max.is_finite() {
        session.graph_max.max(min + 1e-9)
    } else {
        min + 4.
    };
    let map = |x: f64, y: f64| {
        egui::pos2(
            plot.left() + (x / extent) as f32 * plot.width(),
            plot.bottom() - ((y - min) / (max - min)) as f32 * plot.height(),
        )
    };
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(plot, 0., Color32::from_gray(22));
    painter.line_segment(
        [map(0., 0.), map(extent, 0.)],
        Stroke::new(1., Color32::GRAY),
    );
    painter.text(
        rect.left_top(),
        egui::Align2::LEFT_TOP,
        format!("{max:.3}"),
        egui::FontId::monospace(10.),
        Color32::WHITE,
    );
    painter.text(
        rect.left_bottom(),
        egui::Align2::LEFT_BOTTOM,
        format!("{min:.3}"),
        egui::FontId::monospace(10.),
        Color32::WHITE,
    );
    painter.text(
        rect.right_bottom(),
        egui::Align2::RIGHT_BOTTOM,
        format!("{extent:.5} {:?}  /  {:?}", domain, channel.unit),
        egui::FontId::monospace(10.),
        Color32::WHITE,
    );
    let mut cursor = 0.;
    let mut commit = false;
    for (index, t) in channel.transitions.iter_mut().enumerate() {
        let color = if session.transition == index {
            Color32::WHITE
        } else {
            Color32::from_rgb(120, 185, 205)
        };
        let points: Vec<_> = (0..=64)
            .filter_map(|i| {
                let x = t.length * i as f64 / 64.;
                t.evaluate(x).ok().map(|e| map(cursor + x, e.value))
            })
            .collect();
        if points.len() > 1 {
            painter.add(egui::Shape::line(points, Stroke::new(1.5, color)));
        }
        for end in [false, true] {
            let value = if end { t.end } else { t.start };
            let position = map(cursor + if end { t.length } else { 0. }, value);
            let handle_id =
                ui.scope_id()
                    .with((session.selected.0, session.channel as u8, t.id, end));
            let handle = ui.interact(
                egui::Rect::from_center_size(position, egui::vec2(12., 12.)),
                handle_id,
                Sense::drag(),
            );
            painter.rect_filled(
                egui::Rect::from_center_size(position, egui::vec2(6., 6.)),
                0.,
                color,
            );
            if handle.drag_started() {
                ui.ctx().data_mut(|d| d.insert_temp(handle_id, value));
                session.transition = index;
            }
            if handle.dragged() {
                let base = ui
                    .ctx()
                    .data(|d| d.get_temp::<f64>(handle_id))
                    .unwrap_or(value);
                let next = base - handle.drag_delta().y as f64 * (max - min) / plot.height() as f64;
                if end {
                    t.end = next;
                } else {
                    t.start = next;
                }
                if matches!(t.function, Function::Constant {}) {
                    t.start = next;
                    t.end = next;
                }
            }
            if handle.drag_stopped() {
                commit = true;
            }
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let x = (pos.x - plot.left()) as f64 / plot.width() as f64 * extent;
            if (cursor..=cursor + t.length).contains(&x) {
                session.transition = index;
            }
        }
        cursor += t.length;
    }
    ui.label("Drag square endpoints to edit values; release commits one undo step. Click a region to select. Numeric edits below are staged until Apply.");
    if commit {
        session.apply_draft();
    }
    numeric_transition(session, ui);
    derived(session, ui, scene);
}
fn numeric_transition(session: &mut Session, ui: &mut egui::Ui) {
    let key = session.channel;
    let channel = session.draft.channel_mut(key).unwrap();
    session.transition = session.transition.min(channel.transitions.len() - 1);
    let index = session.transition;
    let t = &mut channel.transitions[index];
    ui.horizontal(|ui| {
        ui.label(format!("Transition {}", t.id));
        ui.label("Length");
        ui.add(
            egui::DragValue::new(&mut t.length)
                .speed(0.01)
                .max_decimals(10),
        );
        ui.label("Start");
        ui.add(
            egui::DragValue::new(&mut t.start)
                .speed(0.01)
                .max_decimals(10),
        );
        if matches!(t.function, Function::Constant {}) {
            t.end = t.start;
        }
        ui.label("End");
        ui.add_enabled(
            !matches!(t.function, Function::Constant {}),
            egui::DragValue::new(&mut t.end)
                .speed(0.01)
                .max_decimals(10),
        );
        ui.label("Amplitude");
        ui.add_enabled(
            !matches!(t.function, Function::Constant {}),
            egui::DragValue::new(&mut t.amplitude)
                .speed(0.01)
                .max_decimals(10),
        );
        egui::ComboBox::from_id_salt("transition function")
            .selected_text(function_name(&t.function))
            .show_ui(ui, |ui| {
                for function in [
                    Function::Constant {},
                    Function::Linear {},
                    Function::Quadratic {},
                    Function::CubicSmoothstep {},
                    Function::QuinticSmoothstep {},
                    Function::Cosine {},
                    Function::Hermite {
                        start_slope: 0.,
                        end_slope: 0.,
                    },
                ] {
                    if ui
                        .selectable_label(
                            std::mem::discriminant(&t.function)
                                == std::mem::discriminant(&function),
                            function_name(&function),
                        )
                        .clicked()
                    {
                        t.function = function;
                        if matches!(t.function, Function::Constant {}) {
                            t.end = t.start;
                            t.amplitude = 0.;
                        }
                    }
                }
            });
    });
    if let Function::Hermite {
        start_slope,
        end_slope,
    } = &mut t.function
    {
        ui.horizontal(|ui| {
            ui.label("Endpoint derivatives (output / input)");
            ui.add(
                egui::DragValue::new(start_slope)
                    .speed(0.001)
                    .max_decimals(10),
            );
            ui.add(
                egui::DragValue::new(end_slope)
                    .speed(0.001)
                    .max_decimals(10),
            );
        });
    }
    let id = t.id;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(index > 0, egui::Button::new("Transition earlier"))
            .clicked()
        {
            channel.transitions.swap(index, index - 1);
            session.transition -= 1;
        }
        if ui
            .add_enabled(
                index + 1 < channel.transitions.len(),
                egui::Button::new("Transition later"),
            )
            .clicked()
        {
            channel.transitions.swap(index, index + 1);
            session.transition += 1;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Insert fraction"); ui.add(egui::DragValue::new(&mut session.split_fraction).range(0.001..=0.999).speed(0.01));
        if ui.button("Split into two Hermite regions").on_hover_text("Explicit reshape: retains endpoint values/slopes and section extent; non-cubic source functions can change between endpoints. Apply to commit; Cancel discards.").clicked() {
            match qx_document::editing::split_hermite(&session.draft, key, id, session.split_fraction) { Ok(section) => session.draft = section, Err(e) => session.status = e.to_string() }
        }
        if ui.button("Remove and extend neighbor").on_hover_text("Explicit reshape; neighbor fills removed duration. Apply to commit.").clicked() {
            match qx_document::editing::remove_transition(&session.draft, key, id) { Ok(section) => { session.draft = section; session.transition = 0; }, Err(e) => session.status = e.to_string() }
        }
        if ui.button("Apply section (Ctrl+Enter)").clicked() { session.apply_draft(); }
        if ui.button("Cancel section draft (Esc)").clicked() { session.sync_draft(); }
    });
}
fn function_name(function: &Function) -> &'static str {
    match function {
        Function::Constant {} => "Constant",
        Function::Linear {} => "Linear",
        Function::Quadratic {} => "Quadratic",
        Function::CubicSmoothstep {} => "Cubic smoothstep",
        Function::QuinticSmoothstep {} => "Quintic smoothstep",
        Function::Cosine {} => "Cosine",
        Function::Hermite { .. } => "Hermite",
    }
}
fn derived(session: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    const NAMES: [&str; 8] = [
        "Normal g",
        "Lateral g",
        "Bank rad",
        "Pitch rad",
        "Heading rad",
        "Curvature /m",
        "Calculated path normal g",
        "Calculated path lateral g",
    ];
    ui.horizontal(|ui| {
        ui.label("Derived design-speed diagnostics (view revision)");
        egui::ComboBox::from_id_salt("derived graph")
            .selected_text(NAMES[session.derived_graph])
            .show_ui(ui, |ui| {
                for (i, name) in NAMES.iter().enumerate() {
                    ui.selectable_value(&mut session.derived_graph, i, *name);
                }
            });
    });
    let values: Vec<(f64, f64)> = if session.derived_graph >= 6 {
        scene.reference.as_ref().map_or(vec![], |r| {
            r.points
                .iter()
                .filter(|g| g.section_id == session.selected)
                .map(|g| {
                    (
                        g.source_distance_m,
                        if session.derived_graph == 6 {
                            g.normal_g
                        } else {
                            g.lateral_g
                        },
                    )
                })
                .collect()
        })
    } else {
        scene
            .track
            .graphs
            .iter()
            .filter(|g| g.section_id == session.selected)
            .filter_map(|g| {
                Some((
                    g.distance_m,
                    match session.derived_graph {
                        0 => g.normal_g,
                        1 => g.lateral_g,
                        2 => g.bank_rad?,
                        3 => g.pitch_rad,
                        4 => g.yaw_rad,
                        _ => g.curvature_per_m,
                    },
                ))
            })
            .collect()
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 60.), Sense::hover());
    if values.len() < 2 {
        return;
    }
    let (min_x, max_x) = (values[0].0, values.last().unwrap().0);
    let min_y = values.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
    let max_y = values
        .iter()
        .map(|v| v.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(min_y + 0.01);
    let plot = rect.shrink2(egui::vec2(45., 5.));
    let points: Vec<Pos2> = values
        .into_iter()
        .map(|(x, y)| {
            egui::pos2(
                plot.left() + ((x - min_x) / (max_x - min_x).max(1e-9)) as f32 * plot.width(),
                plot.bottom() - ((y - min_y) / (max_y - min_y)) as f32 * plot.height(),
            )
        })
        .collect();
    ui.painter().add(egui::Shape::line(
        points,
        Stroke::new(1., Color32::from_rgb(230, 185, 95)),
    ));
    ui.painter().text(
        rect.left_top(),
        egui::Align2::LEFT_TOP,
        format!("{max_y:.3}"),
        egui::FontId::monospace(10.),
        Color32::WHITE,
    );
    ui.painter().text(
        rect.left_bottom(),
        egui::Align2::LEFT_BOTTOM,
        format!("{min_y:.3}"),
        egui::FontId::monospace(10.),
        Color32::WHITE,
    );
}
