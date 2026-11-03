//! Normal Coaster Properties appearance controls; reuse the construction draft.
use super::{session::Session, theme};
use qx_style::{
    construction::{Role, TrackConstruction},
    materials::{Binding, Definition, Library},
};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};
type Inspection = Result<(Library, BTreeMap<String, Result<(), String>>), String>;
pub(super) struct State {
    pub appearance_tab: bool,
    pub choosing: Option<String>,
    pub chooser_rect: Option<egui::Rect>,
    pub recent_colors: Vec<[u8; 3]>,
    pub swatches: BTreeMap<String, egui::Rect>,
    pub inspected: Option<String>,
    pub files: BTreeMap<String, Result<(), String>>,
    root: PathBuf,
    pending: Option<mpsc::Receiver<Inspection>>,
    pub error: Option<String>,
    pub render_warnings: Vec<String>,
    pub scroll_to_inspection: bool,
    refresh_assets: bool,
}
impl State {
    pub fn new(root: PathBuf) -> Self {
        let mut state = Self {
            appearance_tab: false,
            choosing: None,
            chooser_rect: None,
            recent_colors: vec![],
            swatches: BTreeMap::new(),
            inspected: None,
            files: BTreeMap::new(),
            root,
            pending: None,
            error: None,
            render_warnings: vec![],
            refresh_assets: false,
            scroll_to_inspection: false,
        };
        state.reload();
        state.refresh_assets = false;
        state
    }
    pub fn reload(&mut self) {
        if self.pending.is_some() {
            return;
        }
        self.refresh_assets = true;
        let root = self.root.clone();
        let (send, receive) = mpsc::channel();
        self.pending = Some(receive);
        std::thread::spawn(move || {
            let result = qx_rendering::material_library::read_manifest(&root).map(|library| {
                let files = qx_rendering::material_library::inspect_maps(&root, &library);
                (library, files)
            });
            let _ = send.send(result);
        });
    }
    pub fn poll(&mut self) -> Option<Inspection> {
        let receive = self.pending.as_ref()?;
        let result = match receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Material library worker stopped. Reload to retry.".into())
            }
        };
        self.pending = None;
        Some(result)
    }
}
pub(super) fn poll(session: &mut Session, ctx: &egui::Context) {
    if let Some(result) = session.material_ui.poll() {
        match result {
            Ok((library, files)) => {
                session.material_catalog = Ok(library);
                session.material_ui.files = files;
                session.material_ui.error = None;
            }
            Err(error) => {
                // Retain known names and all document IDs during recovery.
                session.material_ui.error = Some(error);
                session.material_ui.files.clear();
            }
        }
        session.previews.clear();
        if std::mem::take(&mut session.material_ui.refresh_assets) {
            session.reload_material_assets();
        }
    }
    if session.material_ui.pending.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
    }
}
pub(super) fn apply(session: &mut Session) -> bool {
    if session.style_construction_base.is_some() {
        return super::coaster_properties::apply(session);
    }
    session.execute(vec![qx_document::command::Command::SetConstruction(
        session.construction_draft.clone(),
    )])
}
pub(super) fn cancel(session: &mut Session) {
    if session.style_construction_base.is_some() {
        super::coaster_properties::cancel(session);
        return;
    }
    session.construction_draft = session.editor.project().construction.clone();
    session.refresh_view();
}
pub(super) fn component_name(id: &str) -> String {
    match id {
        "rail-left" => "Left running rail".into(),
        "rail-right" => "Right running rail".into(),
        "spine" => "Structural spine".into(),
        "cross-tie" => "Cross-ties".into(),
        _ => {
            let mut text = id.replace(['-', '_'], " ");
            if let Some(first) = text.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            text
        }
    }
}
fn consumers(c: &TrackConstruction, slot: &str) -> Vec<String> {
    c.continuous
        .iter()
        .filter(|c| c.material == slot)
        .map(|c| component_name(&c.id))
        .chain(
            c.repeated
                .iter()
                .filter(|c| c.material == slot)
                .map(|c| component_name(&c.id)),
        )
        .collect()
}
fn group_name(c: &TrackConstruction, slot: &str) -> String {
    let names = consumers(c, slot);
    let running: Vec<_> = c.continuous.iter().filter(|c| c.material == slot).collect();
    if names.len() > 1
        && names.len() == running.len()
        && running.iter().all(|c| c.role == Role::Running)
    {
        "Running rails".into()
    } else if names.is_empty() {
        "Unused appearance".into()
    } else {
        names.join(" / ")
    }
}
fn name(library: &Result<Library, String>, id: &str) -> String {
    library
        .as_ref()
        .ok()
        .and_then(|l| l.materials.iter().find(|d| d.id == id))
        .map_or_else(|| format!("Missing material: {id}"), |d| d.name.clone())
}
pub(super) fn show(session: &mut Session, ui: &mut egui::Ui) {
    session.material_ui.chooser_rect = None;
    theme::heading(ui, "TRACK APPEARANCE", theme::TRACK);
    ui.small("Choose component colors and finishes. Preview changes here; Apply keeps them.");
    ui.collapsing("Library maintenance", |ui| {
        if ui
            .add_enabled(
                session.material_ui.pending.is_none(),
                egui::Button::new("Reload library"),
            )
            .clicked()
        {
            session.material_ui.reload();
        }
        if session.material_ui.pending.is_some() {
            ui.label("Checking material files…");
        }
    });
    if let Some(error) = &session.material_ui.error {
        ui.colored_label(theme::ERROR, error);
    }
    if let Err(error) = &session.material_catalog {
        ui.colored_label(theme::ERROR, error);
    }
    for warning in &session.material_ui.render_warnings {
        ui.colored_label(theme::ORANGE, warning);
    }
    let groups: Vec<_> = session
        .construction_draft
        .materials
        .iter()
        .filter(|m| !consumers(&session.construction_draft, &m.id).is_empty())
        .map(|m| (m.id.clone(), group_name(&session.construction_draft, &m.id)))
        .collect();
    for (slot, label) in &groups {
        ui.push_id(("appearance", slot), |ui| {
            ui.separator();
            ui.label(theme::text(label, theme::CYAN).strong());
            let material = session.construction_draft.materials.iter_mut().find(|m| &m.id == slot).unwrap();
            let selected = material.library.as_ref().map_or_else(|| "Default finish".into(), |b| name(&session.material_catalog, &b.id));
            let color = if let Some(binding) = &mut material.library { &mut binding.tint } else { &mut material.base_color };
            let swatch=super::color_ui::controls(ui, color);
            session.material_ui.swatches.insert(slot.clone(),swatch);
            ui.ctx().data_mut(|d|d.insert_temp(egui::Id::unique(("appearance color button",slot)),swatch));
            if ui.add(egui::Button::new(&selected).wrap()).on_hover_text("Choose a finish · color and texture placement are kept").clicked() {
                session.material_ui.choosing = if session.material_ui.choosing.as_ref() == Some(slot) {None} else {Some(slot.clone())};
            }
            if session.material_ui.choosing.as_ref() == Some(slot) {
                ui.small("Select a material · the viewport previews the finish");
                let gallery=egui::ScrollArea::vertical().id_salt("material thumbnails").max_height(240.).show(ui, |ui| {
                    if ui.selectable_label(material.library.is_none(), "Default finish").clicked() {
                        if let Some(binding) = material.library.take() { material.base_color = binding.tint; }
                        session.material_ui.choosing = None;
                    }
                    if let Ok(library) = &session.material_catalog {
                        for entry in &library.materials {
                            ui.horizontal(|ui| {
                                if let Some(path) = entry.maps.base_color.as_ref().and_then(|path| session.material_ui.files.get(path).filter(|status|status.is_ok()).map(|_|session.material_ui.root.join(path))) {
                                    ui.allocate_ui(egui::vec2(48.,48.),|ui| {ui.set_max_width(48.);session.previews.show(ui,path,"Sample",44.);});
                                } else {
                                    let (rect,_) = ui.allocate_exact_size(egui::vec2(44.,36.),egui::Sense::hover());
                                    ui.painter().rect_filled(rect,3.,egui::Color32::from(egui::Rgba::from_rgb(entry.base_color[0],entry.base_color[1],entry.base_color[2])));
                                }
                                ui.vertical(|ui| {
                                    if ui.selectable_label(material.library.as_ref().is_some_and(|b| b.id == entry.id), &entry.name).clicked() {
                                        if let Some(binding) = &mut material.library { binding.id = entry.id.clone(); }
                                        else { let mut binding=Binding::new(&entry.id); binding.tint=material.base_color; material.library=Some(binding); }
                                        session.material_ui.choosing = None;
                                    }
                                    ui.small(if entry.metallic > 0.5 { "Metal" } else { "Paint / nonmetal" });
                                });
                            });
                        }
                    }
                });
                session.material_ui.chooser_rect=Some(gallery.inner_rect.intersect(ui.clip_rect()));
            }
            ui.collapsing("Color and finish options", |ui| {
            if ui.small_button("Restore color").on_hover_text("Restore the untinted library color, or the last applied default-finish color").clicked() {
                if let Some(binding) = &mut material.library { binding.tint = [1.;3]; }
                else { material.base_color = session.editor.project().construction.materials.iter().find(|m| m.id == material.id).map_or([1.;3],|m|m.base_color); }
            }
            if let Some(d)=material.library.as_ref().and_then(|b|session.material_catalog.as_ref().ok().and_then(|l|l.materials.iter().find(|d|d.id==b.id))) && d.maps.base_color.is_some() {ui.small("Color tints the original surface pattern.");}
            if material.library.is_some() {
                let mut remove = false;
                ui.horizontal_wrapped(|ui| {
                    remove = ui.button("Remove assignment").clicked();
                });
                if remove && let Some(binding) = material.library.take() { material.base_color = binding.tint; }
                if let Some(binding) = &mut material.library {
                    ui.collapsing("Advanced · texture placement and maps", |ui| {
                        if ui.button("Inspect maps").clicked() { session.material_ui.inspected=Some(binding.id.clone()); session.material_ui.scroll_to_inspection=true; }
                        ui.small("U: profile perimeter (m); V: distance along track (m). Repeated parts use local UVs.");
                        for (label, value) in ["U scale", "V scale", "U offset", "V offset"].into_iter().zip(binding.uv_scale.iter_mut().chain(&mut binding.uv_offset)) {
                            ui.horizontal(|ui| { ui.label(label); ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(6)); });
                        }
                    });
                }
            }
                let color=if let Some(binding)=&mut material.library { &mut binding.tint } else { &mut material.base_color };
                ui.collapsing("RGB / HSV and presets",|ui|super::color_ui::details(ui,color,&mut session.material_ui.recent_colors));
            ui.collapsing("Advanced · fallback material properties", |ui| {
                ui.small("Used when no library material is assigned or its files cannot load. Does not tint library maps.");
                ui.add(egui::Slider::new(&mut material.metallic, 0.0..=1.0).text("Metallic"));
                ui.add(egui::Slider::new(&mut material.roughness, 0.0..=1.0).text("Roughness"));
            });
            });
        });
    }
    ui.separator();
    ui.collapsing("Individual component overrides", |ui| {
        ui.small("Separate a component to edit it independently, or share another component's appearance.");
        let components: Vec<_> = session.construction_draft.continuous.iter().map(|c| (c.id.clone(), c.material.clone()))
            .chain(session.construction_draft.repeated.iter().map(|c| (c.id.clone(), c.material.clone()))).collect();
        for (id, slot) in components {
            ui.push_id(&id, |ui| {
                ui.strong(component_name(&id));
                let c = &mut session.construction_draft;
                let shared = consumers(c, &slot).len() > 1;
                if ui.add_enabled(shared && c.materials.len() < 64, egui::Button::new("Separate appearance")).clicked()
                    && let Err(error) = c.override_component_material(&id) { session.material_ui.error = Some(error.to_string()); }
                egui::ComboBox::from_id_salt("share appearance").selected_text("Share appearance with…").show_ui(ui, |ui| {
                    for (other, label) in &groups {
                        if ui.selectable_label(&slot == other, label).clicked()
                            && let Err(error) = c.share_component_material(&id, other) { session.material_ui.error = Some(error.to_string()); }
                    }
                });
            });
        }
    });
    ui.collapsing("Browse material library", |ui| {
        if let Ok(library) = &session.material_catalog {
            for definition in &library.materials {
                if ui
                    .selectable_label(
                        session.material_ui.inspected.as_ref() == Some(&definition.id),
                        &definition.name,
                    )
                    .clicked()
                {
                    session.material_ui.inspected = Some(definition.id.clone());
                    session.material_ui.scroll_to_inspection = true;
                }
                availability(ui, definition, &session.material_ui);
            }
        }
    });
    if let Some(id) = &session.material_ui.inspected {
        if let Some(definition) = session
            .material_catalog
            .as_ref()
            .ok()
            .and_then(|l| l.materials.iter().find(|d| &d.id == id))
            .cloned()
        {
            inspect(session, ui, &definition);
        } else {
            ui.colored_label(theme::ERROR, format!("{id} is not registered. Restore its library entry, reload, or choose a replacement."));
        }
    }
    if let Err(error) = session.construction_draft.validate() {
        ui.colored_label(theme::ERROR, error.to_string());
    }
    if session.construction_draft != session.editor.project().construction {
        ui.colored_label(theme::ORANGE, "Unapplied appearance / construction edits");
    }
}
fn availability(ui: &mut egui::Ui, d: &Definition, state: &State) {
    let rendered: Vec<_> = d.maps.entries().into_iter().filter(|e| e.3).collect();
    let missing = rendered
        .iter()
        .any(|e| state.files.get(e.1).is_some_and(Result::is_err));
    let checked = rendered.iter().all(|e| state.files.contains_key(e.1)) && state.error.is_none();
    ui.colored_label(
        if missing { theme::ERROR } else { theme::GREEN },
        if missing {
            "Missing map · fallback will be used"
        } else if checked {
            "Map files present · decoded on assignment"
        } else {
            "File availability not checked"
        },
    );
}
fn inspect(session: &mut Session, ui: &mut egui::Ui, d: &Definition) {
    ui.separator();
    let heading = ui.label(theme::text(&d.name, theme::DEEP_PURPLE));
    if std::mem::take(&mut session.material_ui.scroll_to_inspection) {
        heading
            .scroll_to_me_animation(Some(egui::Align::TOP), egui::style::ScrollAnimation::none());
    }
    availability(ui, d, &session.material_ui);
    if d.maps.base_color.is_none() {
        ui.small("No base-color map supplied. Uses the library's constant base color.");
    }
    if let Some(map) = &d.maps.smoothness {
        ui.label("Smoothness → roughness = 1 − sample (once, linear)");
        ui.small(format!("Smoothness source: {}", map.path));
    }
    ui.small(format!(
        "Normal convention: {}",
        if d.normal_convention == qx_style::materials::NormalConvention::OpenGl {
            "OpenGL (+Y), provisional"
        } else {
            "DirectX (−Y)"
        }
    ));
    ui.small(
        "Normal convention follows the registered material. Height / edge maps are metadata only.",
    );
    for (label, path, space, rendered) in d.maps.entries() {
        ui.push_id((d.id.as_str(), label), |ui| {
            ui.collapsing(format!("{label} · {space}"), |ui| {
                ui.label(path);
                match session.material_ui.files.get(path) {
                    Some(Err(error)) => {
                        ui.colored_label(theme::ERROR, error);
                    }
                    Some(Ok(())) => {
                        if let Ok(resolved) =
                            qx_style::library::resolve_existing(&session.material_ui.root, path)
                        {
                            session.previews.show(ui, resolved, label, 130.);
                        }
                    }
                    None => {
                        ui.label("Reload library to inspect this map.");
                    }
                }
                if !rendered {
                    ui.small("Not used in shading or displacement.");
                }
            });
        });
    }
}
