//! Bounded, asynchronous catalog image decoding; textures are presentation state.
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
};

enum Entry {
    Loading(mpsc::Receiver<Result<egui::ColorImage, String>>),
    Ready(egui::TextureHandle),
    Failed(String),
}
#[derive(Default)]
pub(super) struct State {
    entries: BTreeMap<PathBuf, Entry>,
}
impl State {
    pub fn show(&mut self, ui: &mut egui::Ui, path: PathBuf, label: &str, height: f32) {
        for entry in self.entries.values_mut() {
            poll(entry, ui.ctx());
        }
        // Keep a bounded working set, including pending workers and GPU textures.
        if !self.entries.contains_key(&path) && self.entries.len() >= 8 {
            if self
                .entries
                .values()
                .any(|e| matches!(e, Entry::Loading(_)))
            {
                ui.label("Loading style previews...");
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50));
                return;
            }
            self.entries.clear();
        }
        let entry = self.entries.entry(path.clone()).or_insert_with(|| {
            let (send, receive) = mpsc::channel();
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let _ = send.send(decode(&path));
                ctx.request_repaint();
            });
            Entry::Loading(receive)
        });
        poll(entry, ui.ctx());
        match entry {
            Entry::Ready(texture) => {
                ui.add(
                    egui::Image::new(&*texture)
                        .max_size(egui::vec2(ui.available_width().clamp(1., 480.), height))
                        .shrink_to_fit()
                        .alt_text(label),
                );
            }
            Entry::Failed(error) => {
                ui.label(format!("{label} unavailable: {error}"));
            }
            Entry::Loading(_) => {
                ui.label(format!("Loading {label}..."));
            }
        }
    }
}

fn poll(entry: &mut Entry, ctx: &egui::Context) {
    if let Entry::Loading(receive) = entry {
        match receive.try_recv() {
            Ok(Ok(image)) => {
                *entry = Entry::Ready(ctx.load_texture("Style preview", image, Default::default()))
            }
            Ok(Err(error)) => *entry = Entry::Failed(error),
            Err(mpsc::TryRecvError::Disconnected) => {
                *entry = Entry::Failed("Image worker stopped; reopen the project to retry.".into())
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(50))
            }
        }
    }
}

fn decode(path: &Path) -> Result<egui::ColorImage, String> {
    const MAX_BYTES: u64 = 16 * 1024 * 1024;
    let read = || -> Result<_, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > MAX_BYTES {
            return Err("preview exceeds 16 MiB".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err("preview exceeds 16 MiB".into());
        }
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        if !matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ) {
            return Err("only PNG/JPEG previews are supported".into());
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let rgba = reader.decode()?.into_rgba8();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        ))
    };
    read().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn real_png_decodes_and_bad_or_oversized_images_fail() {
        let path = std::env::temp_dir().join(format!(
            "qx-preview-{}.png",
            super::super::session::new_project_id()
        ));
        image::RgbaImage::from_pixel(2, 3, image::Rgba([32, 64, 128, 255]))
            .save(&path)
            .unwrap();
        assert_eq!(super::decode(&path).unwrap().size, [2, 3]);
        std::fs::write(&path, b"invalid image").unwrap();
        assert!(super::decode(&path).is_err());
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(16 * 1024 * 1024 + 1).unwrap();
        drop(file);
        assert!(super::decode(&path).unwrap_err().contains("16 MiB"));
        image::RgbaImage::new(4097, 1).save(&path).unwrap();
        assert!(super::decode(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
