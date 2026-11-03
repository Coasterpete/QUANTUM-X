use super::{
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

/// Parameter popovers must survive clicks on inputs and nested combo boxes.
/// egui's command-menu default closes on every click, including those inputs.
pub(super) fn input_menu<R>(
    ui: &mut egui::Ui,
    label: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let (response, inner) = egui::menu::MenuButton::new(label)
        .config(
            egui::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
        )
        .ui(ui, content);
    egui::InnerResponse {
        response,
        inner: inner.map(|r| r.inner),
    }
}

/// egui 0.37 stores one memory-backed popup. A ComboBox nested in a menu
/// replaces that parent popup, so its contents disappear on the next pass.
/// Use the menu's submenu stack for choices inside parameter popovers.
pub(super) fn choice_menu<R>(
    ui: &mut egui::Ui,
    salt: &str,
    label: &str,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    if egui::menu::is_in_menu(ui) {
        ui.push_id(salt, |ui| ui.menu_button(label, content)).inner
    } else {
        egui::ComboBox::from_id_salt(salt)
            .selected_text(label)
            .show_ui(ui, content)
    }
}

pub(super) fn workspace(
    session: &mut Session,
    ctx: &egui::Context,
    scene: &InspectionScene,
    display: &mut DisplayOptions,
) -> SurfaceOutput {
    session.expansion.sync(session.editor.project());
    session.poll_completion(ctx);
    let before_expansion = (
        session.expansion.operations.clone(),
        session.expansion.supports.clone(),
        session.expansion.completion.is_some(),
    );
    let before_draft = session.draft.clone();
    let before_heartline = session.heartline_draft.clone();
    let before_initial = session.initial_draft.clone();
    let before_construction = session.construction_draft.clone();
    session.inspect_input = None;
    session.hovered = None;
    if session.visible_revision == session.revision
        && session.draft_error.is_none()
        && let Some((id, station)) = session.viewport_inspection
        && id == session.selected
        && let Some(range) = scene.track.sections.iter().find(|r| r.id == id)
    {
        session.inspect_input = Some((
            id,
            session.draft.extent * (station - range.start_distance_m)
                / (range.end_distance_m - range.start_distance_m),
        ));
    }
    session.poll_file_dialog(ctx);
    super::material_ui::poll(session, ctx);
    session.material_ui.render_warnings = scene.materials.warnings.clone();
    super::train_playback::update(session, ctx);
    if let Some(message) = session.assets.poll(session.editor.project(), &session.path) {
        session.status = message;
    }
    if session.assets.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    if session.file_dialog.is_none() && !session.project_prompt_open() {
        shortcuts(session, ctx);
    }
    super::graph_gesture::update(session, ctx);
    let (viewport, mut view_command, window_command) =
        super::layout_ui::show(session, ctx, scene, display);
    // Parse numeric/text events before accepting a keyboard transaction, even
    // when typing and Ctrl+Enter arrive together in one native input batch.
    if std::mem::take(&mut session.apply_section_requested) {
        session.apply_graph();
    }
    // File/history actions run after numeric widgets consume this input batch.
    session.run_ui_action();
    super::train_playback::update(session, ctx);
    if session.file_dialog.is_some() {
        egui::Modal::new(egui::Id::unique("native file dialog pending")).show(ctx, |ui| {
            ui.heading("File dialog");
            ui.label("Complete or cancel the Windows file dialog to continue.");
        });
    }
    display.selected = scene
        .track
        .sections
        .iter()
        .position(|s| s.id == session.selected);
    display.hovered = session
        .hovered
        .and_then(|id| scene.track.sections.iter().position(|s| s.id == id));
    display.inspection_distance_m = session
        .inspect_input
        .filter(|_| session.visible_revision == session.revision && session.draft_error.is_none())
        .and_then(|(id, input)| {
            let section = if id == session.selected {
                &session.draft
            } else {
                session
                    .editor
                    .project()
                    .sections
                    .iter()
                    .find(|s| s.id == id)?
            };
            let range = scene.track.sections.iter().find(|s| s.id == id)?;
            Some(
                range.start_distance_m
                    + (input / section.extent).clamp(0., 1.)
                        * (range.end_distance_m - range.start_distance_m),
            )
        });
    if viewport.contains(ctx.pointer_hover_pos().unwrap_or(egui::Pos2::ZERO))
        && let Some((id, station)) = session.viewport_inspection
    {
        display.hovered = scene.track.sections.iter().position(|s| s.id == id);
        display.inspection_distance_m = Some(station);
    }
    display.overlay_lines.clear();
    display.asset_instances.clear();
    display.train_bounds = None;
    display.selected_car_bounds = None;
    display.inspection_marker =
        session.workbench.active == super::workbench::Profile::Track || session.development;
    let graph_inspection = display.inspection_distance_m;
    super::ride_camera::present(session, ctx, scene, display);
    if session.workbench.active == super::workbench::Profile::Track && session.expansion.engineering
    {
        display.inspection_distance_m = graph_inspection;
    }
    if session.workbench.active != super::workbench::Profile::Train
        && session.ride.mode == super::ride_camera::Mode::Free
        && !session.playback.active()
        && !session.ride.preview_playing
    {
        display.asset_instances.clear();
        display.overlay_lines.clear();
        display.train_bounds = None;
        display.selected_car_bounds = None;
    } else if !session.development {
        display.overlay_lines.clear();
    }
    display.selected_support = (session.workbench.active == super::workbench::Profile::Track)
        .then_some(session.expansion.structure)
        .flatten()
        .map(|id| (id, session.expansion.member));
    let support_track = if session.expansion.structure.is_some() {
        session
            .expansion
            .attachment_track(scene, session.visible_revision)
            .ok()
    } else {
        None
    };
    if let Some(structure) = session
        .expansion
        .supports
        .iter()
        .find(|s| Some(s.id) == session.expansion.structure)
        && let Some(node) = structure
            .nodes
            .iter()
            .find(|n| Some(n.id) == session.expansion.node)
        && let Some(track) = support_track
        && let Ok(p) = node.position.resolve(&track)
    {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            display.overlay_lines.push(qx_rendering::OverlayLine {
                start_m: p - axis * 0.35,
                end_m: p + axis * 0.35,
                color: [1., 1., 0.],
            });
        }
    }
    let ui_frame = ctx.cumulative_frame_nr();
    if session.frame_requested {
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("navigation reset frame"), ui_frame));
        session.frame_requested = false;
    }
    let reset_navigation = ctx
        .data(|d| d.get_temp::<u64>(egui::Id::unique("navigation reset frame")))
        == Some(ui_frame);
    if reset_navigation {
        view_command = Some(
            if session.workbench.active == super::workbench::Profile::Train {
                ViewCommand::FitTrain
            } else {
                ViewCommand::FitAll
            },
        );
    }
    if !ctx.egui_wants_keyboard_input()
        && !session.project_prompt_open()
        && viewport.contains(ctx.pointer_hover_pos().unwrap_or(egui::Pos2::ZERO))
        && session.file_dialog.is_none()
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F))
    {
        view_command = Some(
            if session.workbench.active == super::workbench::Profile::Train {
                ViewCommand::FitTrain
            } else {
                ViewCommand::FitAll
            },
        );
    }
    if std::mem::take(&mut session.assets.fit) {
        view_command = Some(ViewCommand::FitTrain);
    }
    if std::mem::take(&mut session.assets.focus) {
        view_command = Some(ViewCommand::FocusSelectedCar);
    }
    if view_command.is_some() {
        session.ride.mode = super::ride_camera::Mode::Free;
        display.camera_view = None;
    }
    if session.file_dialog.is_none() {
        if session.draft_request.is_some() {
            super::draft_workflow::dialog(session, ctx);
        } else if session.startup_request.is_some() {
            super::startup::leave_dialog(session, ctx);
        } else if session.confirm_replace.is_some() || session.open_dialog_requested {
            egui::Modal::new(egui::Id::unique("replace unsaved project")).show(ctx, |ui| {
                ui.heading("Unsaved edits");
                ui.label("Opening or creating a project will discard the current unsaved document and drafts only after explicit confirmation and a successful load.");
                ui.label(session.save_state());
                for name in session.draft_names() { ui.label(name); }
                if ui.button("Keep current project").clicked() {
                    session.confirm_replace = None;
                    session.open_dialog_requested = false;
                }
                if ui.button("Discard and continue").clicked() {
                    if std::mem::take(&mut session.open_dialog_requested) {
                        session.open_dialog_confirmed = true;
                        session.file_dialog(super::files::FileAction::Open);
                    } else {
                        let path = session.confirm_replace.take().unwrap();
                        session.open(path, true);
                    }
                }
            });
        } else if session.close_requested {
            egui::Modal::new(egui::Id::unique("close unsaved project")).show(ctx, |ui| {
                ui.heading("Close with unsaved edits?");
                ui.label(session.save_state());
                for name in session.draft_names() { ui.label(name); }
                ui.label("Keep working to save, or explicitly discard both committed changes and drafts.");
                if ui.button("Keep working").clicked() { session.close_requested = false; }
                if ui.button("Discard edits and close").clicked() {
                    session.close_requested = false;
                    session.close_approved = true;
                }
            });
        } else {
            super::workflow::resolution_dialog(session, ctx);
        }
    }
    if session.draft != before_draft
        || session.heartline_draft != before_heartline
        || session.initial_draft != before_initial
        || session.construction_draft != before_construction
        || before_expansion
            != (
                session.expansion.operations.clone(),
                session.expansion.supports.clone(),
                session.expansion.completion.is_some(),
            )
    {
        if session.expansion.dirty(session.editor.project())
            || session.draft_dirty()
            || session.heartline_draft != session.editor.project().heartline
            || session.initial_draft != session.editor.project().initial
            || session.construction_draft != session.editor.project().construction
        {
            session.preview();
        } else {
            session.regenerate();
        }
    }
    session.keyboard_owned_last_frame = ctx.any_popup_open() || ctx.egui_wants_keyboard_input();
    SurfaceOutput {
        viewport,
        view_command,
        close: session.close_approved,
        window_command,
        profile: session.workbench.active.index(),
        reset_navigation,
    }
}
pub(super) fn track(session: &mut Session, ui: &mut egui::Ui) {
    if session.track_tab != 1 {
        ui.heading("Region List");
        ui.push_id("section navigation", |ui| sections(session, ui));
        ui.separator();
    }
    if ui.available_width() < 330. {
        let titles = [
            "Region",
            "Coaster Properties",
            "Reference",
            "Project",
            "Operations",
        ];
        egui::ComboBox::from_id_salt("track context")
            .width(ui.available_width())
            .selected_text(titles[session.track_tab.min(4)])
            .show_ui(ui, |ui| {
                for (index, title) in titles.iter().enumerate() {
                    ui.selectable_value(&mut session.track_tab, index, *title);
                }
            });
    } else {
        ui.horizontal_wrapped(|ui| {
            for (index, title) in ["Region", "Coaster", "Reference", "Project", "Operations"]
                .iter()
                .enumerate()
            {
                ui.selectable_value(&mut session.track_tab, index, *title)
                    .on_hover_text(if index == 1 {
                        "Coaster Properties"
                    } else {
                        title
                    });
            }
        });
    }
    if session.track_tab == 4 {
        super::operations_ui::show(session, ui);
    }
    if session.track_tab == 0 {
        // Dynamic draft notices and the section list must never shift numeric
        // editor IDs: a retained keyboard focus could otherwise land on Cancel.
        ui.push_id(("section properties", session.selected.0), |ui| {
            section_properties(session, ui)
        });
        if matches!(session.draft.kind, SectionKind::Spline { .. }) {
            ui.collapsing("Advanced curve controls — existing representation", |ui| {
                spline::controls(
                    &mut session.draft,
                    ui,
                    &mut session.status,
                    session.workbench.snap,
                )
            });
        }
        ui.collapsing("Starting pose and project design speed", |ui| {
            basic_initial(session, ui)
        });
        if session.development {
            ui.collapsing("Mechanisms · development status", |ui| {
                ui.label("Launch and brake force commands are available under Operations. Lift, station and block control systems are not implemented.");
            });
        }
    }
    if session.track_tab == 1 {
        super::coaster_properties::show(session, ui);
    }
    if session.track_tab == 2 {
        heartline(session, ui);
    }
    if session.track_tab == 3 {
        if ui
            .button("Coaster name and description - Coaster Properties")
            .clicked()
        {
            super::coaster_properties::open(session);
        }
        ui.collapsing("Initial conditions", |ui| {
            let initial = &mut session.initial_draft;
            vector(ui, "Start XYZ m", &mut initial.position_m);
            ui.label("Orientation quaternion WXYZ (normalized by generator)");
            for v in [
                &mut initial.orientation.w,
                &mut initial.orientation.x,
                &mut initial.orientation.y,
                &mut initial.orientation.z,
            ] {
                ui.add(egui::DragValue::new(v).speed(0.001).max_decimals(12));
            }
            ui.horizontal(|ui| {
                ui.label("Design speed m/s");
                ui.add(
                    egui::DragValue::new(&mut initial.reference_speed_mps)
                        .speed(0.1)
                        .max_decimals(10),
                );
            });
            if ui.button("Apply initial conditions").clicked() {
                session.execute(vec![Command::SetInitial(session.initial_draft.clone())]);
            }
            if ui.button("Cancel initial draft").clicked() {
                session.initial_draft = session.editor.project().initial.clone();
            }
        });
    }
}

pub(super) fn shortcuts(session: &mut Session, ctx: &egui::Context) {
    if session.graph_drag.is_some() {
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            super::graph_gesture::cancel(
                session,
                "Graph gesture canceled; preceding draft restored.",
            );
        }
        return;
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::S)) {
        session.ui_action = Some(super::draft_workflow::UiAction::SaveAs);
    } else if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::S)) {
        session.ui_action = Some(super::draft_workflow::UiAction::Save);
    }
    if !ctx.egui_wants_keyboard_input() && ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Z))
    {
        session.ui_action = Some(super::draft_workflow::UiAction::History(false));
    }
    if !ctx.egui_wants_keyboard_input()
        && ctx.input_mut(|i| {
            i.consume_key(Modifiers::CTRL, Key::Y)
                || i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z)
        })
    {
        session.ui_action = Some(super::draft_workflow::UiAction::History(true));
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::N)) {
        session.ui_action = Some(super::draft_workflow::UiAction::New);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
        session.ui_action = Some(super::draft_workflow::UiAction::Open);
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Enter)) {
        session.apply_section_requested = true;
    }
    // egui can dismiss a popup or release text focus at the start of the pass.
    // Preserve that Escape's owner from the preceding frame so the same key
    // cannot also discard the section draft after closing the local editor.
    if !session.keyboard_owned_last_frame
        && !ctx.any_popup_open()
        && !ctx.egui_wants_keyboard_input()
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
    {
        session.cancel_graph();
    }
}
fn sections(session: &mut Session, ui: &mut egui::Ui) {
    let sections = session.editor.project().sections.clone();
    let selection_id = ui.scope_id().with("section list selection");
    let reveal = ui.data(|d| d.get_temp::<u64>(selection_id)) != Some(session.selected.0);
    let list_height = (ui.clip_rect().height() * 0.24).clamp(64., 144.);
    egui::ScrollArea::vertical()
        .id_salt("track section list")
        .max_height(list_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (index, section) in sections.iter().enumerate() {
                let response = super::timeline::card(
                    ui,
                    section,
                    index,
                    section.id == session.selected,
                    false,
                );
                if response.clicked() {
                    session.select(section.id);
                }
                if response.hovered() {
                    session.hovered = Some(section.id);
                }
                if reveal && section.id == session.selected {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
                response.on_hover_text(format!(
                    "{}. Selection is shared with the Transition Editor.",
                    section.name
                ));
            }
        });
    ui.data_mut(|d| d.insert_temp(selection_id, session.selected.0));
    ui.small(format!("Insert: {}", insertion_kind_name(session.new_kind)));
    ui.horizontal_wrapped(|ui| {
        input_menu(ui, "Insert Region…", |ui| insertion(session, ui));
        for (before, label) in [(true, "Before"), (false, "After")] {
            if ui.add_enabled(session.graph_drag.is_none(), egui::Button::new(label))
                .on_hover_text("Insert the chosen Region type immediately. Use Insert Region… to choose Straight, Force-Based or Geometric.").clicked() {
                insert_section(session, session.new_kind, before);
            }
        }
        ui.menu_button("Manage", |ui| {
            super::timeline::ordering(session, ui);
            if super::icons::button(
                ui,
                super::icons::Icon::Remove,
                "Remove",
                sections.len() > 1 && session.graph_drag.is_none(),
                "Remove selected region. Undo restores it.",
            )
            .clicked()
            {
                session.remove_selected();
            }
        });
    });
    if session.draft_dirty() {
        ui.small(session.region_extent_status());
    }
}
pub(super) fn insertion(session: &mut Session, ui: &mut egui::Ui) {
    ui.label("Choose a Region, then insert before or after the selection");
    choice_menu(
        ui,
        "new section type",
        insertion_kind_name(session.new_kind),
        |ui| {
            for i in [3, 0, 1] {
                ui.selectable_value(&mut session.new_kind, i, insertion_kind_name(i));
            }
        },
    );
    ui.horizontal_wrapped(|ui| {
        for (before, label) in [(true, "Insert before"), (false, "Insert after")] {
            if super::icons::button(
                ui,
                super::icons::Icon::Insert,
                label,
                session.graph_drag.is_none(),
                "Insert chosen region type relative to the selected region. One undo step.",
            )
            .clicked()
            {
                insert_section(session, session.new_kind.min(8), before);
                ui.close();
            }
        }
    });
    ui.collapsing("Advanced Region types / coordinate frames", |ui| {
        ui.selectable_value(&mut session.new_kind, 1, "Rider-local geometric");
        ui.selectable_value(&mut session.new_kind, 2, "World-geometric");
        ui.selectable_value(&mut session.new_kind, 4, "Circular arc");
        ui.small("Explicit complementary authoring. Existing curves retain their original representation.");
        for (kind, name) in [(5, "Bézier"), (6, "B-spline"), (7, "NURBS"), (8, "Catmull–Rom")] {
            ui.selectable_value(&mut session.new_kind, kind, name);
        }
    });
}
fn insertion_kind_name(kind: usize) -> &'static str {
    [
        "Force-Based",
        "Geometric",
        "World-geometric",
        "Straight",
        "Circular arc",
        "Bézier",
        "B-spline",
        "NURBS",
        "Catmull–Rom",
    ][kind.min(8)]
}
pub(super) fn insert_section(session: &mut Session, kind: usize, before: bool) {
    if session.defer_section_action(super::workflow::PendingAction::Insert { kind, before }) {
        return;
    }
    let Some(id) = session.next_section_id else {
        session.status = "Region IDs exhausted".into();
        return;
    };
    session.next_section_id = id.checked_add(1);
    let after = if before {
        let sections = &session.editor.project().sections;
        sections
            .iter()
            .position(|s| s.id == session.selected)
            .and_then(|i| i.checked_sub(1))
            .map(|i| sections[i].id)
    } else {
        Some(session.selected)
    };
    let index = after
        .and_then(|id| {
            session
                .editor
                .project()
                .sections
                .iter()
                .position(|s| s.id == id)
        })
        .map_or(0, |i| i + 1);
    let initialized = session
        .editor
        .project()
        .generate(&session.catalog, session.options)
        .and_then(|track| {
            qx_document::insertion::initialize(
                session.editor.project(),
                &track,
                index,
                new_section(id, kind),
            )
        });
    let section = match initialized {
        Ok(section) => section,
        Err(error) => {
            session.status = format!("Insertion rejected: {error}");
            session.draft_error = Some(session.status.clone());
            return;
        }
    };
    session.execute(vec![Command::InsertSection { after, section }]);
    if session
        .editor
        .project()
        .sections
        .iter()
        .any(|s| s.id.0 == id)
    {
        session.select(qx_geometry::authoring::SectionId(id));
        session.track_tab = 0;
        session.select_channel(match kind {
            0 => qx_geometry::authoring::ChannelKey::Normal,
            1 | 2 => qx_geometry::authoring::ChannelKey::Pitch,
            _ => qx_geometry::authoring::ChannelKey::Roll,
        });
        session.status = format!(
            "Inserted {} region {} from the preceding endpoint. Edit its graph below; Fit Selection frames it.",
            method(&session.draft),
            id
        );
    }
}
pub(super) fn new_section(id: u64, kind: usize) -> Section {
    let mut section = Section::straight(id, format!("Region {id}"), 10.);
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
            return Section::spline(id, format!("Region {id}"), curve);
        }
        _ => SectionKind::Straight {},
    };
    section
}
fn section_properties(session: &mut Session, ui: &mut egui::Ui) {
    ui.label(
        super::theme::text(
            format!("Region · {}", method(&session.draft)),
            super::theme::ELECTRIC_PURPLE,
        )
        .strong(),
    );
    if session.draft.domain != Domain::Normalized {
        let mut extent = session.draft.extent;
        ui.horizontal(|ui| {
            ui.label(match session.draft.domain {
                Domain::Time => "Duration (s)",
                _ => "Length (m)",
            });
            super::snap_ui::graph_number(
                ui,
                &mut extent,
                "",
                0.01,
                session.workbench.snap,
                super::snap::GraphAxis::Boundary(session.draft.domain),
            );
        });
        if extent != session.draft.extent {
            match qx_document::editing::resize(&session.draft, extent) {
                Ok(s) => session.draft = s,
                Err(e) => session.status = e.to_string(),
            }
        }
    }
    if session.draft_dirty() {
        ui.small(session.region_extent_status());
    }

    ui.push_id("region name", |ui| {
        ui.text_edit_singleline(&mut session.draft.name)
    });
    ui.small(if session.draft_dirty() {
        "Unapplied draft · viewport previews valid inputs"
    } else {
        "Committed · edit values or the graph below"
    });
    section_commit(session, ui);
    ui.push_id("region design speed", |ui| region_speed(session, ui));
    if let SectionKind::Arc {
        radius_m,
        plane_angle_rad,
    } = &mut session.draft.kind
    {
        super::snap_ui::number(
            ui,
            radius_m,
            "Radius m ",
            0.1,
            session.workbench.snap,
            super::snap::Space::Local,
            &[],
        );
        super::snap_ui::number(
            ui,
            plane_angle_rad,
            "Plane rad ",
            0.01,
            session.workbench.snap,
            super::snap::Space::Angle,
            &[],
        );
    }
    ui.small("Authored channels · edit selected transition");
    ui.horizontal_wrapped(|ui| {
        for key in qx_document::editing::channel_keys(&session.draft) {
            if ui
                .selectable_label(
                    session.channel == key,
                    super::graph_canvas::channel_style(key).0,
                )
                .clicked()
            {
                session.select_channel(key);
            }
        }
    });
    super::graph::transition_values(session, ui, "section values");
    ui.small("Roll is an angular rate, integrated from the incoming bank. Force targets generate geometry at design speed.");
    if session.draft.domain != Domain::Normalized {
        ui.collapsing("Domain conversion", |ui| {
            ui.horizontal_wrapped(|ui| {
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
        });
    }
    ui.collapsing("Advanced Region settings", |ui| {
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
    });
}
fn region_speed(session: &mut Session, ui: &mut egui::Ui) {
    let mut override_speed = session.draft.reference_speed_mps.is_some();
    if ui
        .checkbox(&mut override_speed, "Region design-speed override")
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
    } else {
        ui.small(format!(
            "Design speed: {:.6} m/s · inherited from project",
            session.editor.project().initial.reference_speed_mps
        ));
    }
}
fn section_commit(session: &mut Session, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if super::icons::button(
            ui,
            super::icons::Icon::Apply,
            "Apply region",
            session.draft_dirty(),
            "Commit region draft · Ctrl+Enter",
        )
        .clicked()
        {
            session.apply_draft();
        }
        if super::icons::button(
            ui,
            super::icons::Icon::Cancel,
            "Cancel draft",
            session.draft_dirty(),
            "Restore committed region · Esc",
        )
        .clicked()
        {
            session.cancel_section();
        }
    });
}
fn basic_initial(session: &mut Session, ui: &mut egui::Ui) {
    vector(ui, "Start XYZ (m)", &mut session.initial_draft.position_m);
    ui.horizontal_wrapped(|ui| {
        ui.label("Design speed (m/s)");
        ui.add(
            egui::DragValue::new(&mut session.initial_draft.reference_speed_mps)
                .speed(0.1)
                .max_decimals(12),
        );
    });
    ui.small("Design-speed kinematics; independent of future train dynamics. Orientation quaternion is in Project / Initial conditions.");
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                session.initial_draft != session.editor.project().initial,
                egui::Button::new("Apply start"),
            )
            .clicked()
        {
            session.execute(vec![Command::SetInitial(session.initial_draft.clone())]);
        }
        if ui.button("Cancel start").clicked() {
            session.initial_draft = session.editor.project().initial.clone();
        }
    });
}
pub(super) fn heartline(session: &mut Session, ui: &mut egui::Ui) {
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
    ui.small("Offset changes retain all authored region data. No automatic fitting or layout shift is applied.");
    if ui.button("Apply reference settings").clicked() {
        session.execute(vec![Command::SetHeartline(session.heartline_draft.clone())]);
    }
    if ui.button("Cancel reference draft").clicked() {
        session.heartline_draft = session.editor.project().heartline.clone();
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
