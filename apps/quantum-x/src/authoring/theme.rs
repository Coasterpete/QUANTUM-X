//! Owner's Inkscape palette. Blue/violet retain their hue with enough luminance
//! for small engineering labels on black; color always accompanies text/state.
use egui::{Color32, RichText, Stroke, Ui};
pub(super) const LIME: Color32 = Color32::from_rgb(164, 255, 0);
pub(super) const BLUE: Color32 = Color32::from_rgb(100, 132, 255);
pub(super) const VIOLET: Color32 = Color32::from_rgb(188, 112, 255);
pub(super) const GREEN: Color32 = Color32::from_rgb(52, 255, 97);
pub(super) const RED: Color32 = Color32::from_rgb(255, 83, 91);
pub(super) const ORANGE: Color32 = Color32::from_rgb(255, 139, 54);
pub(super) const YELLOW: Color32 = Color32::from_rgb(255, 221, 56);
pub(super) const TRACK: Color32 = Color32::from_rgb(65, 225, 255);
pub(super) const TRAIN: Color32 = VIOLET;
pub(super) const ERROR: Color32 = RED;

pub(super) fn heading(ui: &mut Ui, title: &str, color: Color32) {
    let size = (ui.available_width() / (title.len() as f32 * 0.62)).clamp(13., 22.);
    ui.label(RichText::new(title).size(size).strong().color(color));
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 3.), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0., color);
    ui.add_space(2.);
}
pub(super) fn install(ctx: &egui::Context) {
    let id = egui::Id::unique("M5 typography and icons");
    if ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Overpass Mono".into(),
        egui::FontData::from_static(include_bytes!(
            "../../../../assets/fonts/OverpassMono/OverpassMono-Regular.ttf"
        ))
        .into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "Overpass Mono".into());
    }
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = egui::Color32::from_gray(3);
    style.visuals.window_fill = egui::Color32::from_gray(7);
    style.visuals.extreme_bg_color = egui::Color32::BLACK;
    style.visuals.override_text_color = Some(egui::Color32::from_gray(245));
    style.visuals.weak_text_color = Some(egui::Color32::from_gray(190));
    style.visuals.error_fg_color = ERROR;
    style.visuals.warn_fg_color = YELLOW;
    style.visuals.hyperlink_color = TRACK;
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(28, 42, 103);
    style.visuals.selection.stroke = Stroke::new(2., TRACK);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(15);
    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_gray(15);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., egui::Color32::from_gray(140));
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1., egui::Color32::from_gray(245));
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(15, 42, 57);
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, TRACK);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(39, 61, 115);
    style.visuals.widgets.active.weak_bg_fill = style.visuals.widgets.active.bg_fill;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(2., TRACK);
    style.visuals.window_stroke = Stroke::new(1.5, egui::Color32::from_gray(180));
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., egui::Color32::from_gray(100));
    style.visuals.widgets.inactive.corner_radius = 3.into();
    style.spacing.item_spacing = egui::vec2(8., 8.);
    style.spacing.button_padding = egui::vec2(10., 7.);
    style.spacing.interact_size.y = 32.;
    style.spacing.text_edit_width = 150.;
    style.drag_value_text_style = egui::TextStyle::Monospace;
    for (kind, size) in [
        (egui::TextStyle::Body, 15.),
        (egui::TextStyle::Button, 15.),
        (egui::TextStyle::Small, 13.),
        (egui::TextStyle::Heading, 20.),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(15.));
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.data_mut(|d| d.insert_temp(id, true));
}
