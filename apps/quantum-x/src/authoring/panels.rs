use super::{
    graph,
    session::{Session, method},
    spline,
};
use egui::{Key, Modifiers};
use qx_document::command::Command;
use qx_geometry::{
    authoring::{BoundaryPolicy, Domain, Section, SectionKind},
    heartline::{AuthoredReference, OffsetProfile},
    math::Vec3,
    transition::{Channel, Unit},
};
use qx_rendering::{DisplayOptions, InspectionScene, SurfaceOutput, ViewCommand};

pub(super) fn workspace(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) -> SurfaceOutput {
    let before_draft = session.draft.clone();
    let before_heartline = session.heartline_draft.clone();
    shortcuts(session, ctx);
    let mut viewport = egui::Rect::NOTHING;
    let mut view_command = None;
    ctx.root_ui(|root| {
        egui::Panel::top("commands").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("QUANTUM-X — DEVELOPMENT authoring workspace");
                if ui.button("New (Ctrl+N)").clicked() { session.open(None, false); }
                if ui.button("Open path (Ctrl+O)").clicked() { session.open(Some(session.path.clone().into()), false); }
                if ui.button("Save path (Ctrl+S)").clicked() { session.save(); }
                if ui.add_enabled(session.editor.can_undo(), egui::Button::new("Undo (Ctrl+Z)")).clicked() { session.undo(false); }
                if ui.add_enabled(session.editor.can_redo(), egui::Button::new("Redo (Ctrl+Y)")).clicked() { session.undo(true); }
                ui.label(if session.dirty() { "* Unsaved document" } else { "Saved" });
            });
            ui.horizontal(|ui| { ui.label("Project path"); ui.add(egui::TextEdit::singleline(&mut session.path).desired_width(f32::INFINITY)); });
            ui.label(&session.status);
            if session.visible_revision != session.revision { ui.colored_label(egui::Color32::YELLOW, format!("View revision {} differs from document request {}. See generation status.", session.visible_revision, session.revision)); }
        });
        egui::Panel::bottom("transition editor").resizable(true).default_size(360.).show(root, |ui| {
            egui::ScrollArea::vertical().id_salt("transition scroll").show(ui, |ui| graph::editor(session, ui, scene));
        });
        egui::Panel::left("track workspace").resizable(true).default_size(285.).show(root, |ui| {
            egui::ScrollArea::vertical().id_salt("track scroll").show(ui, |ui| {
                ui.heading("Track Workspace");
                ui.small("Replaceable developer controls; graph/timeline is the primary shaping surface.");
                sections(session, ui);
                ui.separator();
                section_properties(session, ui);
                ui.collapsing("Spline controls (complementary)", |ui| spline::controls(&mut session.draft, ui, &mut session.status));
                ui.collapsing("Rider reference / heartline", |ui| heartline(session, ui));
                ui.collapsing("Coaster Properties — concept catalog", |ui| styles(session, ui));
                ui.collapsing("Project metadata", |ui| {
                    ui.label("Name"); ui.text_edit_singleline(&mut session.metadata_draft.name);
                    ui.label("Author"); ui.text_edit_singleline(&mut session.metadata_draft.author);
                    ui.label("Description"); ui.text_edit_multiline(&mut session.metadata_draft.description);
                    if ui.button("Apply metadata").clicked() { session.execute(vec![Command::SetMetadata(session.metadata_draft.clone())]); }
                    if ui.button("Cancel metadata draft").clicked() { session.metadata_draft = session.editor.project().metadata.clone(); }
                });
                ui.collapsing("Initial conditions", |ui| {
                    let initial = &mut session.initial_draft;
                    vector(ui, "Start XYZ m", &mut initial.position_m);
                    ui.label("Orientation quaternion WXYZ (normalized by generator)");
                    for v in [&mut initial.orientation.w, &mut initial.orientation.x, &mut initial.orientation.y, &mut initial.orientation.z] { ui.add(egui::DragValue::new(v).speed(0.001).max_decimals(12)); }
                    ui.horizontal(|ui| { ui.label("Design speed m/s"); ui.add(egui::DragValue::new(&mut initial.reference_speed_mps).speed(0.1).max_decimals(10)); });
                    if ui.button("Apply initial conditions").clicked() { session.execute(vec![Command::SetInitial(session.initial_draft.clone())]); }
                    if ui.button("Cancel initial draft").clicked() { session.initial_draft = session.editor.project().initial.clone(); }
                });
            });
        });
        egui::Panel::right("support workspace").resizable(true).default_size(270.).show(root, |ui| {
            egui::ScrollArea::vertical().id_salt("support scroll").show(ui, |ui| {
                ui.heading("Support Workspace");
                ui.small("Development diagnostics. Support construction is not implemented in this pass.");
                ui.separator(); ui.strong("Viewport");
                ui.horizontal_wrapped(|ui| {
                    for (label, command) in [("Fit all (F)", ViewCommand::FitAll), ("Fit section", ViewCommand::FitSelected), ("Perspective", ViewCommand::Perspective), ("Top", ViewCommand::Top), ("Side", ViewCommand::Side), ("Front", ViewCommand::Front), ("Orthographic", ViewCommand::ToggleOrthographic)] {
                        if ui.button(label).clicked() { view_command = Some(command); }
                    }
                });
                ui.small("Viewport drag: left orbit; middle/right pan. Wheel zoom. T/N/L axes: X/Y/Z at identity.");
                ui.checkbox(&mut display.centerline, "Authored reference path");
                ui.checkbox(&mut display.reference, if scene.authored_reference == AuthoredReference::TrackCenterline { "Calculated rider heartline" } else { "Constrained physical track reference" });
                ui.checkbox(&mut display.offsets, "Signed offset vectors");
                ui.checkbox(&mut display.points, "Reference-point crosses");
                ui.checkbox(&mut display.frames, "Orientation frames");
                ui.checkbox(&mut display.curvature, "Curvature vectors (scaled)");
                ui.checkbox(&mut display.loads, "Calculated-path loads (scaled)");
                ui.checkbox(&mut display.grid, "Ground reference grid");
                ui.add(egui::DragValue::new(&mut display.grid_spacing).range(0.01..=1e6).prefix("Grid m "));
                ui.separator();
                ui.label(format!("Authored path: {:.6} m", scene.track.length_m()));
                if let Some(reference) = &scene.reference {
                    ui.label(format!("Calculated path: {:.6} m", reference.length_m));
                    ui.small("Loads use each section's constant design speed, rotating-frame offsets and gravity. They are kinematic estimates, not train telemetry.");
                    for warning in &reference.warnings { ui.label(warning); }
                }
                ui.strong("Continuity / generation diagnostics");
                for diagnostic in &scene.track.diagnostics { ui.label(format!("Section {} / {}: {}", diagnostic.section_id.0, diagnostic.code, diagnostic.message)); }
                if scene.track.diagnostics.is_empty() { ui.label("No configured diagnostics triggered."); }
                ui.collapsing("Sampling", |ui| {
                    ui.add(egui::DragValue::new(&mut session.options.max_step_m).speed(0.01).range(1e-6..=10.).prefix("Max step m "));
                    if ui.button("Regenerate").clicked() { session.regenerate(); }
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            viewport = ui.available_rect_before_wrap();
            ui.label("Live 3D · geometric references · no rail surfaces");
            ui.allocate_rect(viewport, egui::Sense::hover());
        });
    });
    display.selected = scene
        .track
        .sections
        .iter()
        .position(|s| s.id == session.selected);
    if !ctx.egui_wants_keyboard_input() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F))
    {
        view_command = Some(ViewCommand::FitAll);
    }
    if session.confirm_replace.is_some() {
        egui::Window::new("Unsaved edits").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Opening or creating a project will discard the current unsaved document and drafts.");
            ui.horizontal(|ui| {
                if ui.button("Keep current project").clicked() { session.confirm_replace = None; }
                if ui.button("Discard and continue").clicked() { let path = session.confirm_replace.take().unwrap(); session.open(path, true); }
            });
        });
    }
    if session.close_requested {
        egui::Window::new("Close with unsaved edits?")
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Apply/cancel drafts and save before closing, or explicitly discard.");
                if ui.button("Keep working").clicked() {
                    session.close_requested = false;
                }
                if ui.button("Discard edits and close").clicked() {
                    session.close_requested = false;
                    session.close_approved = true;
                }
            });
    }
    if session.draft != before_draft || session.heartline_draft != before_heartline {
        if session.draft_dirty() || session.heartline_draft != session.editor.project().heartline {
            session.preview();
        } else {
            session.regenerate();
        }
    }
    SurfaceOutput {
        viewport,
        view_command,
        close: session.close_approved,
    }
}
fn shortcuts(session: &mut Session, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::S)) {
        session.save();
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Z)) {
        session.undo(false);
    }
    if ctx.input_mut(|i| {
        i.consume_key(Modifiers::CTRL, Key::Y)
            || i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z)
    }) {
        session.undo(true);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::N)) {
        session.open(None, false);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
        session.open(Some(session.path.clone().into()), false);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Enter)) {
        session.apply_draft();
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        session.sync_draft();
    }
}
fn sections(session: &mut Session, ui: &mut egui::Ui) {
    let sections = session.editor.project().sections.clone();
    for section in &sections {
        if ui
            .selectable_label(
                section.id == session.selected,
                format!("{}: {} ({})", section.id.0, section.name, method(section)),
            )
            .clicked()
        {
            session.select(section.id);
        }
    }
    let index = sections
        .iter()
        .position(|s| s.id == session.selected)
        .unwrap_or(0);
    ui.horizontal(|ui| {
        let ready = !session.draft_dirty();
        if ui
            .add_enabled(ready && index > 0, egui::Button::new("Earlier"))
            .clicked()
        {
            let after = index.checked_sub(2).map(|i| sections[i].id);
            session.execute(vec![Command::MoveSection {
                id: session.selected,
                after,
            }]);
        }
        if ui
            .add_enabled(
                ready && index + 1 < sections.len(),
                egui::Button::new("Later"),
            )
            .clicked()
        {
            session.execute(vec![Command::MoveSection {
                id: session.selected,
                after: Some(sections[index + 1].id),
            }]);
        }
        if ui
            .add_enabled(
                ready && sections.len() > 1,
                egui::Button::new("Remove section"),
            )
            .clicked()
        {
            session.execute(vec![Command::RemoveSection {
                id: session.selected,
            }]);
        }
    });
    const KINDS: [&str; 9] = [
        "Force-vector",
        "Rider-local",
        "World-geometric",
        "Straight",
        "Arc",
        "Bézier",
        "B-spline",
        "NURBS",
        "Catmull–Rom",
    ];
    egui::ComboBox::from_id_salt("new section type")
        .selected_text(KINDS[session.new_kind])
        .show_ui(ui, |ui| {
            for (i, kind) in KINDS.iter().enumerate() {
                ui.selectable_value(&mut session.new_kind, i, *kind);
            }
        });
    if ui
        .add_enabled(
            !session.draft_dirty(),
            egui::Button::new("Insert after selected"),
        )
        .clicked()
    {
        let Some(id) = sections
            .iter()
            .map(|s| s.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
        else {
            session.status = "Section IDs exhausted".into();
            return;
        };
        let section = new_section(id, session.new_kind);
        session.execute(vec![Command::InsertSection {
            after: Some(session.selected),
            section,
        }]);
        session.select(qx_geometry::authoring::SectionId(id));
    }
}
pub(super) fn new_section(id: u64, kind: usize) -> Section {
    let mut section = Section::straight(id, format!("Section {id}"), 10.);
    section.kind = match kind {
        0 => SectionKind::Force {
            normal_g: Channel::constant(Unit::G, 10., 1.),
            lateral_g: Channel::constant(Unit::G, 10., 0.),
        },
        1 => SectionKind::RiderLocal {
            pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
            yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
        },
        2 => SectionKind::Geometric {
            pitch_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
            yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
        },
        4 => SectionKind::Arc {
            radius_m: 40.,
            plane_angle_rad: 0.,
        },
        5..=8 => {
            use qx_geometry::curve::{ControlPoint, Curve, Parameterization};
            let points = (0..4)
                .map(|i| ControlPoint {
                    id: i + 1,
                    position_m: Vec3::X * (i as f64 * 10. / 3.),
                })
                .collect();
            let curve = match kind {
                5 => Curve::Bezier { points },
                6 => Curve::BSpline {
                    degree: 3,
                    points,
                    knots: vec![0., 0., 0., 0., 1., 1., 1., 1.],
                },
                7 => Curve::Nurbs {
                    degree: 3,
                    points,
                    knots: vec![0., 0., 0., 0., 1., 1., 1., 1.],
                    weights: vec![1.; 4],
                },
                _ => Curve::CatmullRom {
                    points,
                    parameterization: Parameterization::Centripetal,
                },
            };
            return Section::spline(id, format!("Section {id}"), curve);
        }
        _ => SectionKind::Straight {},
    };
    section
}
fn section_properties(session: &mut Session, ui: &mut egui::Ui) {
    ui.strong(format!(
        "Selected {} — {}",
        session.selected.0,
        method(&session.draft)
    ));
    ui.text_edit_singleline(&mut session.draft.name);
    ui.label(
        "Values below are a draft; Apply commits and regenerates. Cancel restores the document.",
    );
    if session.draft.domain != Domain::Normalized {
        let mut extent = session.draft.extent;
        ui.horizontal(|ui| {
            ui.label(format!("Extent {:?}", session.draft.domain));
            ui.add(
                egui::DragValue::new(&mut extent)
                    .speed(0.01)
                    .max_decimals(12),
            );
        });
        if extent != session.draft.extent {
            match qx_document::editing::resize(&session.draft, extent) {
                Ok(s) => session.draft = s,
                Err(e) => session.status = e.to_string(),
            }
        }
        ui.horizontal(|ui| {
            for domain in [Domain::Time, Domain::Distance] {
                if ui
                    .add_enabled(
                        domain != session.draft.domain,
                        egui::Button::new(format!("Convert to {domain:?}")),
                    )
                    .clicked()
                {
                    let speed = session
                        .draft
                        .reference_speed_mps
                        .unwrap_or(session.editor.project().initial.reference_speed_mps);
                    match qx_document::editing::convert_domain(&session.draft, domain, speed) {
                        Ok(s) => session.draft = s,
                        Err(e) => session.status = e.to_string(),
                    }
                }
            }
        });
    }
    let mut override_speed = session.draft.reference_speed_mps.is_some();
    if ui
        .checkbox(&mut override_speed, "Section design-speed override")
        .changed()
    {
        session.draft.reference_speed_mps =
            override_speed.then_some(session.editor.project().initial.reference_speed_mps);
    }
    if let Some(speed) = &mut session.draft.reference_speed_mps {
        ui.add(
            egui::DragValue::new(speed)
                .speed(0.1)
                .max_decimals(12)
                .suffix(" m/s"),
        );
    }
    let mut strict = session.draft.incoming_boundary == BoundaryPolicy::RequireContinuousRates;
    if ui
        .checkbox(&mut strict, "Require continuous incoming rates")
        .changed()
    {
        session.draft.incoming_boundary = if strict {
            BoundaryPolicy::RequireContinuousRates
        } else {
            BoundaryPolicy::AllowRateJump
        };
    }
    if let SectionKind::Arc {
        radius_m,
        plane_angle_rad,
    } = &mut session.draft.kind
    {
        ui.add(
            egui::DragValue::new(radius_m)
                .speed(0.1)
                .prefix("Radius m ")
                .max_decimals(12),
        );
        ui.add(
            egui::DragValue::new(plane_angle_rad)
                .speed(0.01)
                .prefix("Plane rad ")
                .max_decimals(12),
        );
    }
    if ui.button("Apply section (Ctrl+Enter)").clicked() {
        session.apply_draft();
    }
    if ui.button("Cancel section draft").clicked() {
        session.sync_draft();
    }
}
fn heartline(session: &mut Session, ui: &mut egui::Ui) {
    let settings = &mut session.heartline_draft;
    ui.label("Offsets in metres along +T forward, +N rider up, +L lateral. Negative values are supported. Track visual style has no effect.");
    egui::ComboBox::from_id_salt("authored reference")
        .selected_text(format!("{:?}", settings.authored_reference))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut settings.authored_reference,
                AuthoredReference::TrackCenterline,
                "Author track centerline",
            );
            ui.selectable_value(
                &mut settings.authored_reference,
                AuthoredReference::RiderReference,
                "Author rider reference (constrained inverse)",
            );
        });
    if settings.authored_reference == AuthoredReference::RiderReference {
        ui.label("Explicit reinterpretation. Inverse uses the authored orientation field; incompatible tangents/curvature return diagnostics. General inverse solving is unavailable.");
    }
    let mut variable = matches!(settings.rider_offset, OffsetProfile::Quintic { .. });
    if ui
        .checkbox(
            &mut variable,
            "Variable quintic offset along total arc distance",
        )
        .changed()
    {
        settings.rider_offset = match settings.rider_offset {
            OffsetProfile::Constant { offset_m } => OffsetProfile::Quintic {
                start_m: offset_m,
                end_m: offset_m,
            },
            OffsetProfile::Quintic { start_m, .. } => OffsetProfile::Constant { offset_m: start_m },
        };
    }
    match &mut settings.rider_offset {
        OffsetProfile::Constant { offset_m } => {
            vector(ui, "T / N / L", offset_m);
        }
        OffsetProfile::Quintic { start_m, end_m } => {
            vector(ui, "Start T / N / L", start_m);
            vector(ui, "End T / N / L", end_m);
        }
    }
    let mut has_train = settings.train_configuration_id.is_some();
    if ui
        .checkbox(&mut has_train, "Associate nominal train configuration ID")
        .changed()
    {
        settings.train_configuration_id = has_train.then(|| "nominal-train".into());
    }
    if let Some(id) = &mut settings.train_configuration_id {
        ui.text_edit_singleline(id);
    }
    ui.small("Offset changes retain all authored section data. No automatic fitting or layout shift is applied.");
    if ui.button("Apply reference settings").clicked() {
        session.execute(vec![Command::SetHeartline(session.heartline_draft.clone())]);
    }
    if ui.button("Cancel reference draft").clicked() {
        session.heartline_draft = session.editor.project().heartline.clone();
    }
}
fn styles(session: &mut Session, ui: &mut egui::Ui) {
    let selection = session.editor.project().style.clone();
    let mut choose = None;
    for family in &session.catalog.families {
        ui.collapsing(&family.name, |ui| {
            for style in session
                .catalog
                .styles
                .iter()
                .filter(|s| s.family_id == family.id)
            {
                ui.label(format!(
                    "{} — #{} ({:?})",
                    style.name, style.number, style.status
                ));
                ui.label(&style.description);
                if style.preview.is_none() {
                    ui.small("Concept: no asset or representative preview available.");
                }
                ui.small(format!(
                    "Compatible trains: {}",
                    style.compatible_train_ids.join(", ")
                ));
                if ui
                    .add_enabled(
                        selection.style_id != style.id,
                        egui::Button::new("Apply visual style"),
                    )
                    .clicked()
                {
                    choose = Some(qx_style::StyleSelection {
                        family_id: family.id.clone(),
                        style_id: style.id.clone(),
                    });
                }
            }
        });
    }
    if let Some(selection) = choose {
        session.execute(vec![Command::SetStyle(selection)]);
    }
}
fn vector(ui: &mut egui::Ui, label: &str, v: &mut Vec3) -> bool {
    let mut changed = false;
    ui.label(label);
    ui.horizontal(|ui| {
        for value in [&mut v.x, &mut v.y, &mut v.z] {
            changed |= ui
                .add(egui::DragValue::new(value).speed(0.01).max_decimals(12))
                .changed();
        }
    });
    changed
}
