use super::plot::Plot;
use super::session::Session;
use egui::{Color32, Pos2, Sense, Stroke};
use qx_geometry::transition::Function;
use qx_rendering::InspectionScene;

pub(super) fn editor(session: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    super::graph_gesture::update(session, ui.ctx());
    ui.spacing_mut().item_spacing.y = 3.;
    ui.spacing_mut().button_padding.y = 3.;
    ui.spacing_mut().interact_size.y = 24.;
    super::graph_canvas::route_scroll_tail(session, ui);
    super::graph_view::sync(session);
    if ui.available_height() < 170. {
        compact_editor(session, ui, scene);
        return;
    }
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
                    (ui.available_width() - 550.).max(60.)
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
    let narrow = ui.available_width() < 1100.;
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
                exact_view_bounds(session, ui);
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
            ui.label("?").on_hover_text("Square: drag value vertically\nCircle: local NURBS shape control (endpoint jets fixed)\nDiamond: drag boundary horizontally\nRight-click graph: split at station, merge or remove region\nShift while dragging: fine adjustment\nCtrl+wheel over graph: zoom about pointer\nMiddle drag: pan\nF: Fit Graph · Esc: cancel drag\nPlain wheel over timeline scrolls sections; over precision tools scrolls tools. It never zooms the graph.");
        });
    });
    let valid = session
        .draft
        .channel_mut(session.channel)
        .unwrap()
        .validate();
    // Only optional precision/derived content scrolls; it cannot displace the header.
    let details_height = 54.;
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
        .min_scrolled_height(0.)
        .auto_shrink([false, false])
        .max_height(ui.available_height().max(24.))
        .show(ui, |ui| {
            egui::CollapsingHeader::new(
                "Precision editor · values, boundaries and transition tools",
            )
            .id_salt("transition precision")
            .show(ui, |ui| numeric_transition(session, ui));
            ui.collapsing("Measurements · design speed", |ui| {
                derived(session, ui, scene)
            });
        });
}

/// Keep the graph visible in a short dock. Precision tools remain in input menus.
fn compact_editor(session: &mut Session, ui: &mut egui::Ui, scene: &InspectionScene) {
    let tiny = ui.available_height() < 105.;
    let short_sequence = ui.available_height() < 160.;
    ui.spacing_mut().button_padding.x = 6.;
    ui.spacing_mut().item_spacing.x = 6.;
    if !short_sequence {
        super::timeline::sequence(session, ui);
    }
    let mut editing_range = false;
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("compact authored channel")
            .width(136.)
            .selected_text({
                let (label, color) = super::graph_canvas::channel_style(session.channel);
                egui::RichText::new(label).color(color)
            })
            .show_ui(ui, |ui| {
                for key in qx_document::editing::channel_keys(&session.draft) {
                    let (label, color) = super::graph_canvas::channel_style(key);
                    if ui
                        .selectable_label(
                            session.channel == key,
                            egui::RichText::new(label).color(color),
                        )
                        .clicked()
                    {
                        session.select_channel(key);
                    }
                }
            });
        super::panels::input_menu(ui, "Graph tools", |ui| {
            if short_sequence {
                ui.menu_button("Sections", |ui| {
                    for section in session.editor.project().sections.clone() {
                        if ui
                            .selectable_label(section.id == session.selected, &section.name)
                            .clicked()
                        {
                            session.select(section.id);
                        }
                    }
                });
            }
            ui.label(format!(
                "Section {} · {}",
                session.selected.0, session.draft.name
            ));
            let count = session
                .draft
                .channel_mut(session.channel)
                .unwrap()
                .transitions
                .len();
            super::panels::choice_menu(
                ui,
                "compact selected transition",
                &format!("Transition {} / {count}", session.transition + 1),
                |ui| {
                    for index in 0..count {
                        ui.selectable_value(
                            &mut session.transition,
                            index,
                            format!("Transition {}", index + 1),
                        );
                    }
                },
            );
            if ui.button("Fit Graph").clicked() {
                super::graph_canvas::fit(session);
            }
            if ui.button("Reset View").clicked() {
                let view = super::graph_view::View::reset(
                    session.draft.channel_mut(session.channel).unwrap(),
                );
                session.graph_view.set(view);
            }
            editing_range =
                super::panels::input_menu(ui, "View range", |ui| exact_view_bounds(session, ui))
                    .inner
                    .is_some();
            super::panels::input_menu(ui, "Measurements", |ui| derived(session, ui, scene));
            super::panels::input_menu(ui, "Edit values", |ui| {
                transition_values(session, ui, "compact graph values")
            });
            super::panels::input_menu(ui, "Precision editor", |ui| numeric_transition(session, ui));
            super::panels::input_menu(ui, "Section actions", |ui| {
                super::timeline::ordering(session, ui);
                super::panels::insertion(session, ui);
            });
        });
        super::panels::input_menu(ui, "Edit values", |ui| {
            transition_values(session, ui, "compact quick values")
        });
        for (icon, help, apply) in [
            (super::icons::Icon::Apply, "Apply · Ctrl+Enter", true),
            (super::icons::Icon::Cancel, "Cancel · Esc", false),
        ] {
            if super::icons::button(ui, icon, "", session.draft_dirty(), help).clicked() {
                if apply {
                    session.apply_draft();
                } else {
                    session.cancel_section();
                }
            }
        }
    });
    if !editing_range {
        session.graph_view.range_edit = session.graph_view.view.bounds();
        session.graph_view.error = None;
    }
    if tiny {
        ui.small("Resize or float pane for graph");
    } else {
        super::graph_canvas::show(session, ui);
    }
}

fn exact_view_bounds(session: &mut Session, ui: &mut egui::Ui) {
    ui.label("Exact display bounds · authored values stay unchanged");
    let state = &mut session.graph_view;
    egui::Grid::new("exact graph bounds").show(ui, |ui| {
        for (i, label) in ["X from", "X to", "Y from", "Y to"].iter().enumerate() {
            ui.label(*label);
            ui.add(
                egui::DragValue::new(&mut state.range_edit[i])
                    .speed(0.001)
                    .max_decimals(15),
            );
            ui.end_row();
        }
    });
    if let Some(error) = state.error {
        ui.colored_label(Color32::YELLOW, error);
    }
    if ui.button("Apply range").clicked() {
        state.apply_range(session.draft.channel_mut(session.channel).unwrap().length());
    }
    ui.small("Each section/channel remembers its view. First visit fits; unit/domain conversion fits again.");
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
    let has_next = index + 1 < channel.transitions.len();
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(index > 0, egui::Button::new("Transition earlier"))
            .clicked()
        {
            match qx_document::region_edit::reorder(&session.draft, key, index, index - 1) {
                Ok(section) => {
                    session.draft = section;
                    session.transition -= 1;
                }
                Err(e) => session.status = e.to_string(),
            }
        }
        if ui
            .add_enabled(has_next, egui::Button::new("Transition later"))
            .clicked()
        {
            match qx_document::region_edit::reorder(&session.draft, key, index, index + 1) {
                Ok(section) => {
                    session.draft = section;
                    session.transition += 1;
                }
                Err(e) => session.status = e.to_string(),
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Insert fraction"); ui.add(egui::DragValue::new(&mut session.split_fraction).range(0.001..=0.999).speed(0.01));
        if ui.button("Split region (preserve shape)").on_hover_text("Exact NURBS knot insertion or exact legacy restriction. Preserves profile, physical derivatives, station, and geometry. Apply to commit; Cancel discards.").clicked() {
            let c = session.draft.channel_mut(key).unwrap();
            let start: f64 = c.transitions[..index].iter().map(|t| t.length).sum();
            let station = start + c.transitions[index].length * session.split_fraction;
            super::graph_regions::split(session, station);
        }
        if ui.add_enabled(has_next,egui::Button::new("Merge with next (preserve shape)")).clicked() {
            match qx_document::region_edit::merge(&session.draft,key,index) {Ok(section)=>session.draft=section,Err(e)=>session.status=e.to_string()}
        }
        if ui.button("Remove and extend neighbor").on_hover_text("Explicit reshape; neighbor fills removed duration. Apply to commit.").clicked() {
            match qx_document::editing::remove_transition(&session.draft, key, id) { Ok(section) => { session.draft = section; session.transition = index.saturating_sub(1); }, Err(e) => session.status = e.to_string() }
        }
        if ui.button("Apply section (Ctrl+Enter)").clicked() { session.apply_draft(); }
        if ui.button("Cancel section draft (Esc)").clicked() { session.cancel_section(); }
    });
}
pub(super) fn transition_values(session: &mut Session, ui: &mut egui::Ui, salt: &str) {
    let key = session.channel;
    let c = session.draft.channel_mut(key).unwrap();
    session.transition = session.transition.min(c.transitions.len() - 1);
    let index = session.transition;
    let mut t = c.transitions[index].clone();
    let original = t.clone();
    let mut endpoint_edit = None;
    let mut incoming_edit = None;
    let mut operation_error = None;
    let mut join_edit = false;
    ui.push_id((salt,session.selected.0,key as u8,t.id),|ui| {
        ui.label(format!("Transition {}",t.id));
        ui.small("Constrained shared values edit both adjacent regions; outer boundary jets and other regions are preserved.");
        egui::Grid::new("transition value fields").num_columns(2).show(ui,|ui| {
            ui.label("Start");
            if ui.add(egui::DragValue::new(&mut t.start).speed(0.01).max_decimals(10)).changed() {endpoint_edit=Some((false,t.start));}
            ui.end_row();
            ui.label("End");
            if ui.add_enabled(!matches!(t.function,Function::Constant {}),egui::DragValue::new(&mut t.end).speed(0.01).max_decimals(10)).changed() {endpoint_edit=Some((true,t.end));}
            ui.end_row();
            ui.label("Amplitude");
            ui.add_enabled(!matches!(t.function,Function::Constant {}|Function::Nurbs {..}|Function::CosineSpan {..}),egui::DragValue::new(&mut t.amplitude).speed(0.01).max_decimals(10));
            ui.end_row();
            ui.label("Function");
            super::panels::choice_menu(ui,"transition function",function_name(&t.function),|ui| {
                if ui.selectable_label(matches!(t.function,Function::Nurbs {..}),"NURBS").on_hover_text("Exact conversion where supported; legacy higher degrees are retained.").clicked() {
                    match t.to_nurbs() {Ok(next)=>t=next,Err(e)=>operation_error=Some(e.to_string())}
                }
                for function in [Function::Constant {},Function::Linear {},Function::Quadratic {},Function::CubicSmoothstep {},Function::QuinticSmoothstep {},Function::Cosine {},Function::Hermite {start_slope:0.,end_slope:0.}] {
                    if ui.selectable_label(std::mem::discriminant(&t.function)==std::mem::discriminant(&function),function_name(&function)).on_hover_text("Explicit profile reshape; constrained neighboring joins must remain valid.").clicked()
                        && std::mem::discriminant(&t.function)!=std::mem::discriminant(&function) {
                        t.function=function;
                        if matches!(t.function,Function::Constant {}) {t.end=t.start;t.amplitude=0.;}
                    }
                }
            });
        });
        if ui.button("Reshape as cubic NURBS").on_hover_text("Explicit reshape; preserves endpoint values and first/second derivatives, but changes the interior. Apply or Cancel the draft.").clicked() {
            let result=(|| -> qx_geometry::Result<_> {
                let a=t.evaluate(0.)?;let b=t.evaluate(t.length)?;let rule=t.incoming_continuity();
                let mut next=qx_geometry::transition::Transition::nurbs(t.id,t.length,t.start,t.end);
                next.set_endpoint_jet(false,a,2)?;next.set_endpoint_jet(true,b,2)?;
                if let Function::Nurbs {profile}=&mut next.function {profile.incoming=rule;}
                Ok(next)
            })();
            match result {Ok(next)=>t=next,Err(e)=>operation_error=Some(e.to_string())}
        }
        if ui.add_enabled(index>0,egui::Button::new("Join incoming smoothly (C2 reshape)")).on_hover_text("Explicitly reshape both neighboring profiles. Keeps this start value/slope/second derivative and both outer boundary jets. Legacy cosine interiors become cubic. Preview, then Apply or Cancel.").clicked() {join_edit=true;}
        if let Function::Hermite {start_slope,end_slope}=&mut t.function {
            ui.label("Slopes (output / input)");
            egui::Grid::new("transition slope fields").num_columns(2).show(ui,|ui| {
                ui.label("Start slope");ui.add(egui::DragValue::new(start_slope).speed(0.001).max_decimals(10));ui.end_row();
                ui.label("End slope");ui.add(egui::DragValue::new(end_slope).speed(0.001).max_decimals(10));ui.end_row();
            });
        }
        if let Function::Nurbs {profile}=&mut t.function {
            ui.label(format!("NURBS degree {} · {} controls",profile.degree,profile.values.len()));
            let mut shape=profile.clone();
            if let Err(e)=shape.refine_for_edit() {operation_error=Some(e.to_string());}
            ui.small("Shape controls change this region while preserving both endpoint values, slopes and second derivatives.");
            let mut reshaped=false;
            egui::Grid::new("profile controls").num_columns(2).show(ui,|ui| {
                let count=shape.values.len();
                for (i,value) in shape.values.iter_mut().enumerate().take(count-3).skip(3) {
                    ui.label(format!("Shape control {}",i-2));
                    reshaped|=ui.add(egui::DragValue::new(value).speed(0.001).max_decimals(10)).changed();ui.end_row();
                }
            });
            if reshaped {*profile=shape;}
            ui.collapsing("Advanced NURBS / continuity",|ui| {
                ui.label("Internal knot continuity");
                egui::ComboBox::from_id_salt("internal profile continuity").selected_text(format!("{:?}",profile.interior)).show_ui(ui,|ui| {
                    for rule in [qx_geometry::profile::Continuity::C2,qx_geometry::profile::Continuity::C1,qx_geometry::profile::Continuity::C0,qx_geometry::profile::Continuity::Free] {
                        ui.selectable_value(&mut profile.interior,rule,format!("{rule:?}"));
                    }
                });
                if index>0 {
                    ui.label("Incoming boundary constraint (relaxing never changes values)");
                    for (rule,label) in [(qx_geometry::profile::Continuity::C2,"C2 · value, slope, second derivative"),(qx_geometry::profile::Continuity::C1,"C1 · value and slope"),(qx_geometry::profile::Continuity::C0,"C0 · value"),(qx_geometry::profile::Continuity::Free,"Allow discontinuity · advanced")] {
                        if ui.selectable_label(profile.incoming==rule,label).clicked() {incoming_edit=Some(rule);}
                    }
                }
                if ui.button("Refine reshape controls (preserve shape)").clicked()
                    && let Err(e)=profile.refine_for_edit() {operation_error=Some(e.to_string());}
                ui.label("All interior control values (boundary constraints remain enforced)");
                let count=profile.values.len();
                for (i,v) in profile.values.iter_mut().enumerate().take(count-1).skip(1) {
                    ui.horizontal(|ui| {ui.label(format!("Control {i}"));ui.add(egui::DragValue::new(v).speed(0.001).max_decimals(10));});
                }
                ui.label("Positive rational weights");
                for (i,w) in profile.weights.iter_mut().enumerate() {
                    ui.horizontal(|ui| {ui.label(format!("Weight {i}"));ui.add(egui::DragValue::new(w).speed(0.01).max_decimals(10));});
                }
                ui.monospace(format!("Knots: {:?}",profile.knots));
                ui.label("Insert knot at fraction");
                ui.add(egui::DragValue::new(&mut session.split_fraction).range(0.001..=0.999).speed(0.01));
                if ui.button("Insert knot (preserve shape)").clicked()
                    && let Err(e)=profile.insert_knot(session.split_fraction) {operation_error=Some(e.to_string());}
            });
        }
        if let Ok(issues)=session.draft.channel_mut(key).unwrap().continuity() {
            for issue in issues {
                ui.colored_label(egui::Color32::YELLOW,format!("Boundary {:.8}: value jump {:.3e}, slope jump {:.3e}, second derivative jump {:.3e}",issue.input,issue.value_jump,issue.derivative_jump,issue.second_derivative_jump));
            }
        }
    });
    let next = if let Some((end, value)) = endpoint_edit {
        Some(qx_document::region_edit::endpoint(
            &session.draft,
            key,
            index,
            end,
            value,
        ))
    } else if join_edit {
        Some(qx_document::region_edit::join_incoming(
            &session.draft,
            key,
            index,
        ))
    } else if let Some(rule) = incoming_edit {
        Some(qx_document::region_edit::continuity(
            &session.draft,
            key,
            index,
            rule,
        ))
    } else if t != original {
        Some(qx_document::region_edit::replace(
            &session.draft,
            key,
            index,
            t,
        ))
    } else {
        None
    };
    if let Some(next) = next {
        match next {
            Ok(section) => session.draft = section,
            Err(e) => session.status = e.to_string(),
        }
    }
    if let Some(error) = operation_error {
        session.status = error;
    }
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
        Function::Nurbs { .. } => "NURBS",
        Function::CosineSpan { .. } => "Legacy cosine span",
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
