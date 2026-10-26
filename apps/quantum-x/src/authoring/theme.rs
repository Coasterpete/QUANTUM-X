//! Restrained provisional presentation of the owner's M5 workspace specification.
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
    style.visuals.panel_fill = egui::Color32::from_gray(29);
    style.visuals.window_fill = egui::Color32::from_gray(29);
    style.visuals.extreme_bg_color = egui::Color32::from_gray(15);
    style.visuals.override_text_color = Some(egui::Color32::from_gray(220));
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(43, 78, 96);
    style.visuals.selection.stroke = egui::Stroke::new(1.5, egui::Color32::from_gray(245));
    style.spacing.item_spacing = egui::vec2(7., 6.);
    style.spacing.button_padding = egui::vec2(8., 5.);
    style.spacing.interact_size.y = 26.;
    style.spacing.text_edit_width = 150.;
    style.drag_value_text_style = egui::TextStyle::Monospace;
    for (kind, size) in [
        (egui::TextStyle::Body, 13.),
        (egui::TextStyle::Button, 13.),
        (egui::TextStyle::Small, 11.),
        (egui::TextStyle::Heading, 17.),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.));
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.data_mut(|d| d.insert_temp(id, true));
}
