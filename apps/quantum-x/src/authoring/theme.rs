//! Exact owner interface colors with neutral contrast aids.
use egui::{Color32, RichText, Stroke, Ui};
// Owner-approved interface palette, sRGB bytes. Coaster colors are unrestricted.
pub(super) const LIME: Color32 = Color32::from_rgb(0x9A, 0xFF, 0x00);
pub(super) const BLUE: Color32 = Color32::from_rgb(0x00, 0x00, 0xFF);
pub(super) const VIOLET: Color32 = Color32::from_rgb(0x5C, 0x00, 0xFF);
pub(super) const GREEN: Color32 = Color32::from_rgb(0x1B, 0xFF, 0x00);
pub(super) const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x44, 0x00);
pub(super) const ELECTRIC_PURPLE: Color32 = Color32::from_rgb(0x79, 0x06, 0xFF);
pub(super) const DEEP_PURPLE: Color32 = Color32::from_rgb(0x93, 0x04, 0xB3);
pub(super) const CYAN: Color32 = Color32::from_rgb(0x0D, 0xDF, 0xDF);
pub(super) const PURPLE: Color32 = Color32::from_rgb(0x6D, 0x05, 0xCE);
// Additional reference colors: workspace/error red and yaw yellow.
pub(super) const RED: Color32 = Color32::from_rgb(255, 0, 0);
pub(super) const YELLOW: Color32 = Color32::from_rgb(255, 204, 0);
pub(super) const TRACK: Color32 = CYAN;
pub(super) const TRAIN: Color32 = ELECTRIC_PURPLE;
pub(super) const ERROR: Color32 = RED;

/// Small dark accents use the existing neutral foreground for contrast.
/// Exact palette hues remain on icons, rules, branding and graph strokes;
/// ordinary text must not look selected because of a per-glyph background.
pub(super) fn text(label: impl Into<String>, color: Color32) -> RichText {
    let foreground = if [BLUE, VIOLET, ELECTRIC_PURPLE, DEEP_PURPLE, PURPLE].contains(&color) {
        Color32::from_gray(245)
    } else {
        color
    };
    RichText::new(label).color(foreground)
}
pub(super) fn outlined_text(
    painter: &egui::Painter,
    position: egui::Pos2,
    align: egui::Align2,
    label: &str,
    font: egui::FontId,
    color: Color32,
) {
    if [BLUE, VIOLET, ELECTRIC_PURPLE, DEEP_PURPLE, PURPLE].contains(&color) {
        for offset in [
            egui::vec2(-1., 0.),
            egui::vec2(1., 0.),
            egui::vec2(0., -1.),
            egui::vec2(0., 1.),
        ] {
            painter.text(
                position + offset,
                align,
                label,
                font.clone(),
                Color32::from_gray(220),
            );
        }
    }
    painter.text(position, align, label, font, color);
}

pub(super) fn heading(ui: &mut Ui, title: &str, color: Color32) {
    let size = (ui.available_width() / (title.len() as f32 * 0.62)).clamp(13., 22.);
    ui.label(text(title, color).size(size).strong());
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_labels_have_no_text_backing_and_selection_keeps_its_style() {
        let ctx = egui::Context::default();
        install(&ctx);
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                ..Default::default()
            },
            |ctx| {
                ctx.root_ui(|root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.menu_button(text("View", PURPLE), |_| {});
                        super::super::icons::button(
                            ui,
                            super::super::icons::Icon::Save,
                            "Save",
                            true,
                            "Save",
                        );
                        for (label, color) in [
                            ("Transition Editor", BLUE),
                            ("Region · Force-vector", ELECTRIC_PURPLE),
                            ("Track Workspace", RED),
                            ("Train Workspace", TRAIN),
                            ("Normal force", BLUE),
                            ("Pitch rate", VIOLET),
                        ] {
                            ui.label(text(label, color));
                        }
                        let _ = ui.selectable_label(true, text("Active selection", TRACK));
                    });
                });
            },
        );
        // This is a CPU paint-state test without a texture consumer.
        output.textures_delta.clear();
        let mut ordinary = 0;
        for shape in &output.shapes {
            if let egui::Shape::Text(t) = &shape.shape {
                ordinary += 1;
                assert!(
                    t.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.background == Color32::TRANSPARENT),
                    "{}",
                    t.galley.job.text
                );
                if t.galley.job.text == "View" {
                    assert_eq!(
                        t.galley.job.sections[0].format.color,
                        Color32::from_gray(245)
                    );
                }
            }
        }
        assert_eq!(ordinary, 9);
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(
            style.visuals.selection.bg_fill,
            Color32::from_rgb(28, 42, 103)
        );
        assert_eq!(style.visuals.selection.stroke, Stroke::new(2., TRACK));
        assert_eq!(
            style.visuals.widgets.active.bg_stroke,
            Stroke::new(2., TRACK)
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(r) if r.fill == style.visuals.selection.bg_fill)));
    }
    #[test]
    fn exact_owner_palette() {
        for (color, hex) in [
            (LIME, "#9AFF00"),
            (BLUE, "#0000FF"),
            (VIOLET, "#5C00FF"),
            (GREEN, "#1BFF00"),
            (ORANGE, "#FF4400"),
            (ELECTRIC_PURPLE, "#7906FF"),
            (DEEP_PURPLE, "#9304B3"),
            (CYAN, "#0DDFDF"),
            (PURPLE, "#6D05CE"),
        ] {
            assert_eq!(color, Color32::from_hex(hex).unwrap());
        }
    }
}
