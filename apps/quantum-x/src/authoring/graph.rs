use super::plot::Plot;
use super::session::Session;
use egui::{Color32, Pos2, Sense, Stroke};
use qx_geometry::transition::Function;
use qx_rendering::InspectionScene;

pub(super) fn editor(session: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    super::graph_gesture::update(session, ui.ctx());
    ui.spacing_mut().item_spacing.y = 3.;
    super::graph_canvas::route_scroll_tail(session, ui);
    super::graph_view::sync(session);
    super::timeline::sequence(session, ui);
    // Context, commit controls and navigation are outside every scrolling region.
    let narrow_header = ui.available_width() < 650.;
    ui.horizontal_wrapped(|ui| {
        let channel = session.draft.channel_mut(session.channel).unwrap().clone();
        session.transition = session.transition.min(channel.transitions.len() - 1);
        let selected = &channel.transitions[session.transition];
        let context = format!(
            "Section {} · {} · {}",
            session.selected.0,
            session.draft.name,
            super::timeline::extent(&session.draft)
        );
        ui.add_sized(
            [
                if narrow_header {
                    ui.available_width()
                } else {
                    (ui.available_width() - 430.).max(60.)
                },
                26.,
            ],
            egui::Label::new(context.clone()).truncate(),
        )
        .on_hover_text(context);
        ui.add_enabled_ui(session.graph_drag.is_none(), |ui| {
            egui::ComboBox::from_id_salt("selected transition")
                .width(148.)
                .selected_text(format!(
                    "Transition {} / {}",
                    session.transition + 1,
                    channel.transitions.len()
                ))
                .show_ui(ui, |ui| {
                    for (index, t) in channel.transitions.iter().enumerate() {
                        ui.selectable_value(
                            &mut session.transition,
                            index,
                            format!(
                                "{} · {} (ID {})",
                                index + 1,
                                function_name(&t.function),
                                t.id
                            ),
                        );
                    }
                })
                .response
                .on_hover_text(format!(
                    "{} · {:.10} -> {:.10}",
                    function_name(&selected.function),
                    selected.start,
                    selected.end
                ));
            ui.label(if session.draft_dirty() {
                "Draft"
            } else {
                "Committed"
            });
            if super::icons::button(
                ui,
                super::icons::Icon::Apply,
                "Apply",
                session.draft_dirty(),
                "Commit section draft · Ctrl+Enter",
            )
            .clicked()
            {
                session.apply_draft();
            }
            if super::icons::button(
                ui,
                super::icons::Icon::Cancel,
                "Cancel",
                session.draft_dirty(),
                "Discard section draft · Esc",
            )
            .clicked()
            {
                session.cancel_section();
            }
        });
    });
    let compact = ui.available_width() < 1100.;
    let narrow = ui.available_width() < 800.;
    ui.add_enabled_ui(session.graph_drag.is_none(), |ui| {
        ui.horizontal_wrapped(|ui| {
            for key in qx_document::editing::channel_keys(&session.draft) {
                let (label, color) = super::graph_canvas::channel_style(key);
                if ui.selectable_label(session.channel == key, egui::RichText::new(label).color(color)).clicked() {
                    session.select_channel(key);
                }
            }
            if !narrow && super::icons::button(ui, super::icons::Icon::Transitions, "Fit Graph", true,
                "Fit full section and sampled curve. Constant profiles keep a useful unit-specific range. F over graph.").clicked() {
                super::graph_canvas::fit(session);
            }
            if !narrow && ui.button("Reset View").on_hover_text("Full section domain; Y: force -1..3 g, distance rates ±0.05 rad/m, other rates ±1. Display only.").clicked() {
                let view = super::graph_view::View::reset(session.draft.channel_mut(session.channel).unwrap());
                session.graph_view.set(view);
            }
            let range_menu = super::panels::input_menu(ui, if narrow { "Graph view" } else { "View range" }, |ui| {
                if narrow {
                    if ui.button("Fit Graph").clicked() { super::graph_canvas::fit(session); }
                    if ui.button("Reset View").clicked() {
                        let view=super::graph_view::View::reset(session.draft.channel_mut(session.channel).unwrap());
                        session.graph_view.set(view);
                    }
                }
                ui.label("Exact display bounds · authored values stay unchanged");
                let state = &mut session.graph_view;
                egui::Grid::new("exact graph bounds").show(ui, |ui| {
                    for (i, label) in ["X from", "X to", "Y from", "Y to"].iter().enumerate() {
                        ui.label(*label);
                        ui.add(egui::DragValue::new(&mut state.range_edit[i]).speed(0.001).max_decimals(15));
                        ui.end_row();
                    }
                });
                if let Some(error) = state.error { ui.colored_label(Color32::YELLOW, error); }
                if ui.button("Apply range").clicked() {
                    state.apply_range(session.draft.channel_mut(session.channel).unwrap().length());
                }
                ui.small("Each section/channel remembers its view. First visit fits; unit/domain conversion fits again.");
            });
            if range_menu.inner.is_none() {
                session.graph_view.range_edit = session.graph_view.view.bounds();
                session.graph_view.error = None;
            }
            super::panels::input_menu(ui, "Edit values", |ui| {
                ui.label("Authored values - Apply commits the section");
                transition_values(session, ui, "graph quick values");
            });
            if compact {
                super::panels::input_menu(ui, "More", |ui| {
                    transition_tools(session, ui);
                    ui.separator();
                    super::timeline::ordering(session, ui);
                    super::panels::insertion(session, ui);
                });
            } else {
                super::panels::input_menu(ui, "Transitions", |ui| transition_tools(session, ui));
                super::panels::input_menu(ui, "Section actions", |ui| {
                    super::timeline::ordering(session, ui);
                    super::panels::insertion(session, ui);
                });
            }
            ui.label("?").on_hover_text("Square: drag value vertically\nDiamond: drag boundary horizontally\nCtrl+wheel over graph: zoom about pointer\nMiddle drag: pan\nF: Fit Graph · Esc: cancel drag\nPlain wheel over timeline scrolls sections; over precision tools scrolls tools. It never zooms the graph.");
        });
    });
    let valid = session
        .draft
        .channel_mut(session.channel)
        .unwrap()
        .validate();
    // Only optional precision/derived content scrolls; it cannot displace the header.
    let details_height = (ui.available_height() * 0.28).clamp(ui.spacing().interact_size.y, 160.);
    let canvas_height =
        (ui.available_height() - details_height - ui.spacing().item_spacing.y).max(66.);
    ui.allocate_ui(egui::vec2(ui.available_width(), canvas_height), |ui| {
        if let Err(error) = valid {
            ui.colored_label(
                Color32::from_rgb(240, 195, 100),
                format!("Invalid draft graph: {error}. Correct precision inputs or Cancel."),
            );
        } else {
            super::graph_canvas::show(session, ui);
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("graph precision scroll")
        .auto_shrink([false, false])
        .max_height(ui.available_height().max(24.))
        .show(ui, |ui| {
            egui::CollapsingHeader::new(
                "Precision editor · values, boundaries and transition tools",
            )
            .id_salt("transition precision")
            .show(ui, |ui| numeric_transition(session, ui));
            ui.collapsing("Derived measurements · design-speed diagnostics", |ui| {
                derived(session, ui, scene)
            });
        });
}

pub(super) fn numeric_transition(session: &mut Session, ui: &mut egui::Ui) {
    let channel = session.draft.channel_mut(session.channel).unwrap();
    let id = channel.transitions[session.transition.min(channel.transitions.len() - 1)].id;
    ui.push_id(
        (
            "precise transition",
            session.selected.0,
            session.channel as u8,
            id,
        ),
        |ui| numeric_transition_inner(session, ui),
    );
}
fn numeric_transition_inner(session: &mut Session, ui: &mut egui::Ui) {
    let key = session.channel;
    let channel = session.draft.channel_mut(key).unwrap().clone();
    let index = session.transition.min(channel.transitions.len() - 1);
    let before: f64 = channel.transitions[..index].iter().map(|t| t.length).sum();
    let mut boundary = before + channel.transitions[index].length;
    ui.horizontal_wrapped(|ui| {
        ui.monospace(format!(
            "Start boundary: {before:.8} {:?}",
            session.draft.domain
        ));
        ui.label("End boundary");
        if ui
            .add_enabled(
                index + 1 < channel.transitions.len(),
                egui::DragValue::new(&mut boundary)
                    .speed(0.01)
                    .max_decimals(12),
            )
            .changed()
        {
            match super::graph_canvas::boundary(&session.draft, key, index, boundary - before) {
                Ok(next) => session.draft = next,
                Err(error) => session.status = error,
            }
        }
    });
    ui.collapsing("Individual transition length · advanced timing", |ui| {
        ui.small("Edit lengths precisely; every channel must still sum to the section extent. Use End boundary above to transfer duration between neighbors.");
        let t = &mut session.draft.channel_mut(key).unwrap().transitions[index];
        ui.add(egui::DragValue::new(&mut t.length).speed(0.01).max_decimals(12));
    });
    transition_values(session, ui, "precision values");
    transition_tools(session, ui);
}
pub(super) fn transition_tools(session: &mut Session, ui: &mut egui::Ui) {
    let key = session.channel;
    let channel = session.draft.channel_mut(key).unwrap();
    let index = session.transition.min(channel.transitions.len() - 1);
    let t = &mut channel.transitions[index];
    let id = t.id;
    ui.horizontal_wrapped(|ui| {
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
    ui.horizontal_wrapped(|ui| {
        ui.label("Insert fraction"); ui.add(egui::DragValue::new(&mut session.split_fraction).range(0.001..=0.999).speed(0.01));
        if ui.button("Split into two Hermite regions").on_hover_text("Explicit reshape: retains endpoint values/slopes and section extent; non-cubic source functions can change between endpoints. Apply to commit; Cancel discards.").clicked() {
            match qx_document::editing::split_hermite(&session.draft, key, id, session.split_fraction) { Ok(section) => session.draft = section, Err(e) => session.status = e.to_string() }
        }
        if ui.button("Remove and extend neighbor").on_hover_text("Explicit reshape; neighbor fills removed duration. Apply to commit.").clicked() {
            match qx_document::editing::remove_transition(&session.draft, key, id) { Ok(section) => { session.draft = section; session.transition = 0; }, Err(e) => session.status = e.to_string() }
        }
        if ui.button("Apply section (Ctrl+Enter)").clicked() { session.apply_draft(); }
        if ui.button("Cancel section draft (Esc)").clicked() { session.cancel_section(); }
    });
}
pub(super) fn transition_values(session: &mut Session, ui: &mut egui::Ui, salt: &str) {
    let key = session.channel;
    let channel = session.draft.channel_mut(key).unwrap();
    let id = channel.transitions[session.transition.min(channel.transitions.len() - 1)].id;
    ui.push_id((salt, session.selected.0, key as u8, id), |ui| {
        let channel = session.draft.channel_mut(key).unwrap();
        session.transition = session.transition.min(channel.transitions.len() - 1);
        let index = session.transition;
        let t = &mut channel.transitions[index];
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Transition {}", t.id));
            ui.label("Start");
            let start_changed = ui
                .add(
                    egui::DragValue::new(&mut t.start)
                        .speed(0.01)
                        .max_decimals(10),
                )
                .changed();
            if start_changed && matches!(t.function, Function::Constant {}) {
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
            super::panels::choice_menu(
                ui,
                "transition function",
                function_name(&t.function),
                |ui| {
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
                            && std::mem::discriminant(&t.function)
                                != std::mem::discriminant(&function)
                        {
                            t.function = function;
                            if matches!(t.function, Function::Constant {}) {
                                t.end = t.start;
                                t.amplitude = 0.;
                            }
                        }
                    }
                },
            );
        });
        if let Function::Hermite {
            start_slope,
            end_slope,
        } = &mut t.function
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("Slopes (output / input)");
                ui.label("Start slope");
                ui.add(
                    egui::DragValue::new(start_slope)
                        .speed(0.001)
                        .max_decimals(10),
                );
                ui.label("End slope");
                ui.add(
                    egui::DragValue::new(end_slope)
                        .speed(0.001)
                        .max_decimals(10),
                );
            });
        }
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
    ui.horizontal_wrapped(|ui| {
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
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(120.), 140.),
        Sense::hover(),
    );
    if values.len() < 2 {
        return;
    }
    let (min_x, max_x) = (values[0].0, values.last().unwrap().0);
    let min_y = values.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
    let max_y = values.iter().map(|v| v.1).fold(f64::NEG_INFINITY, f64::max);
    let margin = ((max_y - min_y) * 0.1).max(0.005);
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(70., 17.),
        rect.max - egui::vec2(22., 24.),
    );
    let Some(mapping) = Plot::new(plot, [min_x, max_x], [min_y - margin, max_y + margin]) else {
        return;
    };
    let unit = match session.derived_graph {
        0 | 1 | 6 | 7 => "g",
        2..=4 => "rad",
        _ => "1/m",
    };
    mapping.paint_grid(ui, rect, "m (global)", unit);
    let points: Vec<Pos2> = values.into_iter().map(|(x, y)| mapping.map(x, y)).collect();
    ui.painter()
        .with_clip_rect(plot.intersect(ui.clip_rect()))
        .add(egui::Shape::line(
            points,
            Stroke::new(1.5, Color32::from_rgb(230, 185, 95)),
        ));
}
