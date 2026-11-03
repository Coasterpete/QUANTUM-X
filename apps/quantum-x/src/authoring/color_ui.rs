//! Exact sRGB entry at the UI boundary; document/shader factors stay linear.
use egui::{Color32, Ui};

pub(super) fn srgb(linear: [f32; 3]) -> [u8; 3] {
    let c = egui::Rgba::from_rgb(linear[0], linear[1], linear[2]);
    let [r, g, b, _] = Color32::from(c).to_array();
    [r, g, b]
}
pub(super) fn linear(bytes: [u8; 3]) -> [f32; 3] {
    let c = egui::Rgba::from(Color32::from_rgb(bytes[0], bytes[1], bytes[2]));
    [c.r(), c.g(), c.b()]
}
pub(super) fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}
fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

pub(super) fn controls(ui: &mut Ui, color: &mut [f32; 3]) -> egui::Rect {
    let mut swatch = egui::Rect::NOTHING;
    let before = srgb(*color);
    let mut rgb = before;
    ui.horizontal_wrapped(|ui| {
        ui.label("Color");
        let response = ui.color_edit_button_srgb(&mut rgb);
        swatch = response.rect;
        response.on_hover_text("Open color picker · hue, saturation and brightness");
        let id = ui.scope_id().with("hex color");
        let mut text = ui
            .ctx()
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| hex(rgb));
        let field = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id_salt("hex entry")
                .desired_width(100.)
                .hint_text("#RRGGBB"),
        );
        field
            .clone()
            .on_hover_text("Exact hexadecimal sRGB color, e.g. #2468FF");
        if field.changed()
            && let Some(value) = parse_hex(&text)
        {
            rgb = value;
        }
        if parse_hex(&text).is_none() {
            ui.colored_label(super::theme::ERROR, "Use six hex digits");
        }
        if !field.has_focus() && !field.changed() {
            text = hex(rgb);
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, text));
    });
    if rgb != before {
        *color = linear(rgb);
    }
    swatch
}
pub(super) fn details(ui: &mut Ui, color: &mut [f32; 3], recent: &mut Vec<[u8; 3]>) {
    let before = srgb(*color);
    let mut rgb = before;
    ui.horizontal_wrapped(|ui| {
        for (name, value) in ["R", "G", "B"].into_iter().zip(&mut rgb) {
            ui.label(name);
            ui.add(egui::DragValue::new(value).range(0..=255));
        }
    });
    let mut hsv = egui::ecolor::Hsva::from(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    let mut values = [hsv.h * 360., hsv.s * 100., hsv.v * 100.];
    let mut changed = false;
    for (index, label) in ["Hue (°)", "Saturation (%)", "Brightness (%)"]
        .into_iter()
        .enumerate()
    {
        changed |= ui
            .add(
                egui::Slider::new(
                    &mut values[index],
                    0. ..=if index == 0 { 360. } else { 100. },
                )
                .text(label),
            )
            .changed();
    }
    if changed {
        hsv.h = values[0] / 360.;
        hsv.s = values[1] / 100.;
        hsv.v = values[2] / 100.;
        let c = Color32::from(hsv);
        rgb = [c.r(), c.g(), c.b()];
    }
    ui.small("Color presets");
    swatches(
        ui,
        &[
            [255, 255, 255],
            [35, 35, 35],
            [230, 30, 45],
            [30, 100, 255],
            [145, 30, 240],
            [20, 210, 75],
            [255, 160, 20],
            [15, 220, 220],
        ],
        &mut rgb,
    );
    if !recent.is_empty() {
        ui.small("Saved colors · this session");
        swatches(ui, recent, &mut rgb);
    }
    if ui.button("Save color preset").clicked() {
        recent.retain(|c| c != &rgb);
        recent.insert(0, rgb);
        recent.truncate(12);
    }
    if rgb != before {
        *color = linear(rgb);
    }
}
fn swatches(ui: &mut Ui, colors: &[[u8; 3]], rgb: &mut [u8; 3]) {
    ui.horizontal_wrapped(|ui| {
        for &color in colors {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(24., 24.), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, 2., Color32::from_rgb(color[0], color[1], color[2]));
            ui.painter().rect_stroke(
                rect,
                2.,
                egui::Stroke::new(if color == *rgb { 3. } else { 1. }, Color32::WHITE),
                egui::StrokeKind::Inside,
            );
            if response.on_hover_text(hex(color)).clicked() {
                *rgb = color;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_srgb_roundtrip_and_invalid_hex() {
        for value in 0..=255 {
            let c = [value, 255 - value, value];
            assert_eq!(srgb(linear(c)), c);
        }
        assert_eq!(parse_hex("#2468FF"), Some([36, 104, 255]));
        for text in ["#12345", "##FFFFFF", "zzzzzz", "💙", "12345678", "nan"] {
            assert_eq!(parse_hex(text), None);
        }
    }
}
