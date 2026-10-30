//! Curated embedded masks. SVG decoding and size-aware texture caching belong to egui_extras.
use egui::{Button, Image, Response, Ui};

macro_rules! registry {
    ($($command:ident => $file:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug)]
        pub(super) enum Icon { $($command),+ }
        impl Icon {
            pub fn asset(self) -> (&'static str, &'static [u8]) {
                match self { $(Self::$command => (
                    concat!("bytes://qx/", $file, ".svg"),
                    include_bytes!(concat!("../../../../assets/editor-icons/", $file, ".svg"))
                )),+ }
            }
            #[cfg(test)]
            const ALL: &'static [Self] = &[$(Self::$command),+];
        }
    }
}
registry! {
    New => "lucide-file-plus", Open => "lucide-folder-open", Save => "lucide-save",
    SaveAs => "lucide-save-pen", Undo => "lucide-undo-2", Redo => "lucide-redo-2",
    Earlier => "lucide-arrow-up", Later => "lucide-arrow-down", Insert => "lucide-plus",
    Remove => "lucide-trash", Apply => "lucide-check", Cancel => "lucide-x",
    Preferences => "lucide-settings-2", FitAll => "carbon-zoom--fit",
    FitSelection => "carbon-center-to-fit", Perspective => "carbon-cube",
    Transitions => "carbon-chart--line",
}
pub(super) fn button(ui: &mut Ui, icon: Icon, label: &str, enabled: bool, help: &str) -> Response {
    let (uri, bytes) = icon.asset();
    let color = match icon {
        Icon::New | Icon::Apply => super::theme::GREEN,
        Icon::Save | Icon::SaveAs => super::theme::BLUE,
        Icon::Remove | Icon::Cancel => super::theme::RED,
        Icon::Undo | Icon::Redo => super::theme::VIOLET,
        _ => super::theme::TRACK,
    };
    let response = ui
        .add_enabled(
            enabled,
            Button::image_and_text(
                Image::from_bytes(uri, bytes).fit_to_exact_size(egui::Vec2::splat(16.)),
                egui::RichText::new(label).color(color),
            )
            .image_tint_follows_text_color(true),
        )
        .on_hover_text(help);
    if label.is_empty() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, enabled && ui.is_enabled(), help)
        });
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_is_unique_and_every_svg_decodes_at_multiple_dpi_sizes() {
        let mut uris = std::collections::HashSet::new();
        for icon in Icon::ALL {
            let (uri, bytes) = icon.asset();
            assert!(uris.insert(uri));
            for size in [16, 24, 32] {
                let image = egui_extras::image::load_svg_bytes_with_size(
                    bytes,
                    egui::load::SizeHint::Size {
                        width: size,
                        height: size,
                        maintain_aspect_ratio: true,
                    },
                    &Default::default(),
                )
                .unwrap();
                assert!(image.pixels.iter().any(|p| p.a() > 0));
                assert_eq!(image.size, [size as usize, size as usize]);
            }
        }
    }
}
