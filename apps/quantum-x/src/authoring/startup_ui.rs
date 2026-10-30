//! Native Welcome composition derived from the owner's original Inkscape drawing.
use super::{
    startup::{App, Page},
    startup_pages as pages, theme,
};
use qx_rendering::{EditorSurface, SurfaceOutput};

pub(super) fn show(app: &mut App, ctx: &egui::Context) -> SurfaceOutput {
    theme::install(ctx);
    ctx.set_zoom_factor(app.window_preferences().ui_scale);
    if app.picker.is_none() && app.page != Page::New {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            app.begin_new();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) {
            app.pick(None);
        }
    }
    if app.picker.is_none()
        && !ctx.egui_wants_keyboard_input()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        if app.page == Page::New {
            app.cancel_setup();
        } else {
            app.page = Page::Welcome;
        }
    }
    let mut window_command = None;
    ctx.root_ui(|root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK).inner_margin(24.))
            .show(root, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_enabled_ui(app.picker.is_none(), |ui| {
                        if app.page == Page::Welcome {
                            welcome(app, ui);
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Welcome").clicked() {
                                    if app.page == Page::New { app.cancel_setup(); }
                                    else { app.page = Page::Welcome; app.refresh_recents(); }
                                }
                                ui.label(egui::RichText::new("QUANTUM Dynamics").strong().color(theme::LIME));
                            });
                            ui.add_space(12.);
                            ui.set_max_width(1100.);
                            match app.page {
                                Page::New => pages::new_project(app, ui),
                                Page::Library => { theme::heading(ui, "LIBRARY", theme::VIOLET); pages::library(app, ui); }
                                Page::Projects => {
                                    theme::heading(ui, "EDITOR", theme::BLUE);
                                    ui.label("Create a project or open an existing project to enter the Editor.");
                                    project_actions(app, ui);
                                    ui.add_space(16.);
                                    pages::recent(app, ui);
                                }
                                Page::Info => pages::info(ui),
                                Page::News => {
                                    theme::heading(ui, "NEWS", theme::RED);
                                    ui.heading("News is not available in this Alpha build.");
                                    ui.label("No news feed is connected. This page contains no generated articles or announcements.");
                                }
                                Page::Configuration => { window_command = pages::configuration(app, ui); }
                                _ => {}
                            }
                        }
                    });
                    if !app.status.is_empty() { ui.separator(); ui.label(&app.status); }
                    if let Some(error) = &app.recent.error { ui.colored_label(theme::YELLOW, error); }
                });
            });
    });
    if app.picker.is_some() {
        egui::Modal::new(egui::Id::unique("welcome native picker")).show(ctx, |ui| {
            ui.heading("Open Project");
            ui.label("Complete or cancel the Windows file dialog to continue.");
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    SurfaceOutput {
        viewport: egui::Rect::NOTHING,
        view_command: None,
        close: false,
        window_command,
        profile: 0,
        reset_navigation: false,
    }
}

fn welcome(app: &mut App, ui: &mut egui::Ui) {
    let width = ui.available_width();
    let short = ui.available_height() < 650.;
    let tiny = ui.available_height() < 400.;
    if tiny {
        ui.spacing_mut().button_padding.y = 3.;
    }
    let available_height = ui.available_height();
    ui.add_space(if tiny {
        6.
    } else if short {
        (available_height * 0.12).min(80.)
    } else {
        (available_height * 0.21).min(200.)
    });
    let brand_size = (width / 6.3)
        .min(available_height * 0.17)
        .clamp(28., if short { 76. } else { 100. });
    // Fixed line spacing follows the original stacked wordmark, rather than
    // inheriting the font's generous paragraph leading at display sizes.
    let (brand, _) =
        ui.allocate_exact_size(egui::vec2(width, brand_size * 2.), egui::Sense::hover());
    for (label, color, line) in [
        ("QUANTUM", theme::LIME, -0.48),
        ("DYNAMICS", theme::BLUE, 0.48),
    ] {
        ui.painter().text(
            brand.center() + egui::vec2(0., brand_size * line),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(brand_size),
            color,
        );
    }
    ui.add_space(if tiny {
        12.
    } else if short {
        26.
    } else {
        72.
    });
    let columns = if width < 600. { 2 } else { 4 };
    let gap = 12.;
    let slot = ((width.min(1200.) - gap * (columns - 1) as f32) / columns as f32).max(90.);
    let font = ((slot - 24.) / 4.5).clamp(18., if tiny { 22. } else { 36. });
    let height = font + if tiny { 16. } else { 24. };
    ui.vertical_centered(|ui| {
        for row in [(0, columns), (columns, 4)] {
            if row.0 >= row.1 {
                continue;
            }
            let items = [
                ("Library", theme::VIOLET, Page::Library),
                ("Editor", theme::BLUE, Page::Projects),
                ("Info", theme::GREEN, Page::Info),
                ("News", theme::RED, Page::News),
            ];
            let row_width = slot * (row.1 - row.0) as f32 + gap * (row.1 - row.0 - 1) as f32;
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - row_width) / 2.).max(0.));
                ui.spacing_mut().item_spacing.x = gap;
                for (label, color, page) in &items[row.0..row.1] {
                    if navigation(ui, label, *color, egui::vec2(slot, height), font) {
                        app.page = *page;
                    }
                }
            });
            ui.add_space(10.);
        }
        if navigation(ui, "Setup", theme::ORANGE, egui::vec2(slot, height), font) {
            app.show_configuration();
        }
        ui.add_space(16.);
        let actions_width = 350_f32.min(ui.available_width());
        ui.allocate_ui_with_layout(
            egui::vec2(actions_width, 36.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| project_actions(app, ui),
        );
        ui.add_space(10.);
    });
    egui::CollapsingHeader::new(format!("Recent Projects ({})", app.recent.entries.len()))
        .id_salt("welcome recents")
        .show(ui, |ui| pages::recent(app, ui));
}

fn navigation(
    ui: &mut egui::Ui,
    label: &str,
    color: egui::Color32,
    size: egui::Vec2,
    font: f32,
) -> bool {
    let response = ui
        .add_sized(
            size,
            egui::Button::new(
                egui::RichText::new(label.to_uppercase())
                    .size(font)
                    .strong()
                    .color(color),
            )
            .fill(egui::Color32::BLACK)
            .stroke(egui::Stroke::new(2., color))
            .corner_radius(3.),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(3.),
            2.,
            egui::Stroke::new(2., egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    response.clicked()
}

fn project_actions(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (icon, label, help, new) in [
            (
                super::icons::Icon::New,
                "New Project…",
                "Ctrl+N · project name only",
                true,
            ),
            (
                super::icons::Icon::Open,
                "Open Project…",
                "Ctrl+O · open a saved project",
                false,
            ),
        ] {
            if super::icons::button(ui, icon, label, true, help).clicked() {
                if new {
                    app.begin_new();
                } else {
                    app.pick(None);
                }
            }
        }
    });
}
