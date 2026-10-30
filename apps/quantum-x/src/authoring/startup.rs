//! Application lifecycle. Welcome/setup own no implicit document. A replacement
//! is built and validated before transferring preferences or dropping a session.
use super::{
    files,
    project_library::Projects,
    recent::Recent,
    session::{self, Session},
    workbench::{PreferenceStore, Workbench},
};
use qx_document::{Project, persistence};
use qx_rendering::{DisplayOptions, EditorSurface, InspectionScene, SurfaceOutput};
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Launch {
    Welcome,
    Editor(Option<PathBuf>),
    Command,
}
pub(crate) fn is_project_path(path: &Path) -> bool {
    path.file_name().is_some_and(|n| {
        n.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".qx.json")
    })
}
pub(crate) fn launch(executable: &Path, args: &[std::ffi::OsString]) -> Launch {
    if args.is_empty()
        && executable.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .eq_ignore_ascii_case(&qx_rendering::identity::product().windows_executable)
        })
    {
        Launch::Welcome
    } else if args.len() == 1 && is_project_path(Path::new(&args[0])) {
        Launch::Editor(Some(PathBuf::from(&args[0])))
    } else if args.first().is_some_and(|a| a == "edit") && args.len() <= 2 {
        Launch::Editor(args.get(1).map(PathBuf::from))
    } else {
        Launch::Command
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Destination {
    Welcome,
    New,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Page {
    Welcome,
    Library,
    New,
    Editor,
    Projects,
    Info,
    News,
    Configuration,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LibraryTab {
    Samples,
    Templates,
    Tracks,
    Trains,
}
pub(super) struct Setup {
    pub name: String,
    pub template: Option<(Project, PathBuf)>,
}
impl Setup {
    fn new() -> Self {
        Self {
            name: String::new(),
            template: None,
        }
    }
    fn project(&self, catalog: &qx_style::Catalog) -> Result<Project, String> {
        if self.name.trim().is_empty() {
            return Err("Enter a project name before creating the project.".into());
        }
        let id = session::new_project_id();
        let mut project = if let Some((project, _)) = &self.template {
            project.clone()
        } else {
            Project {
                schema_version: qx_document::SCHEMA_VERSION,
                metadata: qx_document::Metadata {
                    project_id: id.clone(),
                    name: self.name.trim().into(),
                    author: String::new(),
                    description: String::new(),
                },
                initial: Default::default(),
                style: Default::default(),
                sections: vec![qx_geometry::authoring::Section::straight(
                    1,
                    "Initial straight",
                    10.,
                )],
                heartline: Default::default(),
                construction: Default::default(),
                trains: vec![],
            }
        };
        project.metadata.project_id = id;
        project.metadata.name = self.name.trim().into();
        project.validate(catalog).map_err(|e| e.to_string())?;
        Ok(project)
    }
}
pub(super) struct Picker {
    pub result: mpsc::Receiver<Option<PathBuf>>,
    pub relocate: Option<usize>,
}
pub(super) struct App {
    pub page: Page,
    pub session: Option<Session>,
    pub setup: Setup,
    pub library_tab: LibraryTab,
    pub projects: Projects,
    pub library: qx_style::library::Library,
    pub catalog: qx_style::Catalog,
    pub status: String,
    pub picker: Option<Picker>,
    pub recent: Recent,
    pub missing: Vec<bool>,
    pub configuration_draft: qx_rendering::WindowPreferences,
    workbench: Workbench,
    preferences: PreferenceStore,
    pending_scene: Option<InspectionScene>,
}
impl App {
    /// Deterministic diagnostic capture without personal recents or preferences.
    pub(super) fn offscreen() -> Result<Self, String> {
        let mut app = Self::welcome()?;
        app.workbench = Default::default();
        app.preferences = Default::default();
        app.recent = Recent::default();
        app.refresh_recents();
        Ok(app)
    }
    pub fn welcome() -> Result<Self, String> {
        let (catalog, warning) = super::library_ui::load_catalog();
        let root = super::library_ui::assets_root();
        let library = qx_style::library::Library::from_catalog(catalog.clone(), &root)
            .map_err(|e| e.to_string())?;
        let (workbench, preferences) = PreferenceStore::load();
        let mut app = Self {
            page: Page::Welcome,
            session: None,
            setup: Setup::new(),
            library_tab: LibraryTab::Samples,
            projects: Projects::discover(&root),
            library,
            catalog,
            status: warning.unwrap_or_default(),
            picker: None,
            recent: Recent::load(),
            missing: vec![],
            configuration_draft: Default::default(),
            workbench,
            preferences,
            pending_scene: None,
        };
        app.refresh_recents();
        Ok(app)
    }
    pub fn refresh_recents(&mut self) {
        self.missing = self
            .recent
            .entries
            .iter()
            .map(|e| !e.path.is_file())
            .collect();
    }
    pub fn show_configuration(&mut self) {
        self.configuration_draft = self.window_preferences();
        self.page = Page::Configuration;
    }
    pub fn begin_new(&mut self) {
        self.setup = Setup::new();
        self.page = Page::New;
        self.status.clear();
    }
    pub fn cancel_setup(&mut self) {
        self.setup = Setup::new();
        self.page = if self.session.is_some() {
            Page::Editor
        } else {
            Page::Welcome
        };
        self.status.clear();
    }
    fn install(&mut self, mut candidate: Session, scene: InspectionScene) {
        if let Some(current) = self.session.as_mut() {
            candidate.workbench = std::mem::take(&mut current.workbench);
            candidate.preferences = std::mem::take(&mut current.preferences);
        } else {
            candidate.workbench = std::mem::take(&mut self.workbench);
            candidate.preferences = std::mem::take(&mut self.preferences);
        }
        self.session = Some(candidate);
        self.pending_scene = Some(scene);
        self.page = Page::Editor;
        self.setup = Setup::new();
        self.status.clear();
    }
    pub fn create(&mut self) {
        let result = (|| -> Result<_, String> {
            let project = self.setup.project(&self.catalog)?;
            let scene = session::scene(&project, &self.catalog, Default::default())?;
            let mut candidate =
                Session::new(project, None, self.catalog.clone()).map_err(|e| e.to_string())?;
            if let Some((_, source)) = &self.setup.template {
                candidate
                    .assets
                    .remember(candidate.editor.project(), &source.to_string_lossy());
            }
            Ok((candidate, scene))
        })();
        match result {
            Ok((candidate, scene)) => self.install(candidate, scene),
            Err(e) => self.status = format!("Project not created: {e}"),
        }
    }
    pub fn load_project(&mut self, path: PathBuf, relocate: Option<usize>) {
        let result = (|| -> Result<_, String> {
            let project = persistence::load(&path, &self.catalog).map_err(|e| e.to_string())?;
            let scene = session::scene(&project, &self.catalog, Default::default())?;
            let candidate = Session::new(project, Some(&path), self.catalog.clone())
                .map_err(|e| e.to_string())?;
            Ok((candidate, scene))
        })();
        match result {
            Ok((candidate, scene)) => {
                if let Some(index) = relocate {
                    self.recent.remove(index);
                }
                self.recent
                    .remember(&path, &candidate.editor.project().metadata.name);
                self.refresh_recents();
                self.install(candidate, scene);
            }
            Err(e) => {
                self.status = format!(
                    "Open failed for {}: {e}. Choose another file or locate/remove a recent entry.",
                    path.display()
                );
                self.refresh_recents();
            }
        }
    }
    pub fn use_template(&mut self, path: PathBuf) {
        match persistence::load(&path, &self.catalog)
            .map_err(|e| e.to_string())
            .and_then(|p| session::scene(&p, &self.catalog, Default::default()).map(|_| p))
        {
            Ok(project) => {
                self.begin_new();
                self.setup.name = format!("{} copy", project.metadata.name);
                self.setup.template = Some((project, path));
            }
            Err(e) => self.status = format!("Template could not be loaded: {e}"),
        }
    }
    pub fn pick(&mut self, relocate: Option<usize>) {
        if self.picker.is_some() {
            return;
        }
        let path = relocate
            .and_then(|i| self.recent.entries.get(i))
            .map_or(PathBuf::new(), |e| e.path.clone());
        match files::project_picker(path) {
            Ok(result) => self.picker = Some(Picker { result, relocate }),
            Err(e) => self.status = e,
        }
    }
    pub fn poll_picker(&mut self) {
        match self.picker.as_ref().map(|p| p.result.try_recv()) {
            Some(Ok(path)) => {
                let picker = self.picker.take().unwrap();
                if let Some(path) = path {
                    self.load_project(path, picker.relocate);
                } else {
                    self.status = "Open canceled; no project was created or replaced.".into();
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.picker = None;
                self.status =
                    "File dialog stopped; no project was replaced. Try Open again.".into();
            }
            _ => {}
        }
    }
    fn session_events(&mut self) {
        if let Some(s) = self.session.as_mut()
            && let Some(path) = s.recent_project.take()
        {
            self.recent
                .remember(&path, &s.editor.project().metadata.name);
            self.refresh_recents();
        }
        let destination = self
            .session
            .as_mut()
            .and_then(|s| s.startup_approved.take());
        match destination {
            Some(Destination::New) => self.begin_new(),
            Some(Destination::Welcome) => {
                let mut old = self.session.take().unwrap();
                self.workbench = std::mem::take(&mut old.workbench);
                self.preferences = std::mem::take(&mut old.preferences);
                self.page = Page::Welcome;
                self.pending_scene = Some(InspectionScene::empty());
                self.refresh_recents();
            }
            None => {}
        }
    }
}
impl EditorSurface for App {
    fn window_title(&self) -> String {
        let title = if self.page == Page::Editor {
            return format!(
                "{} — Editor — {}",
                qx_rendering::identity::product().name,
                self.session
                    .as_ref()
                    .map(|s| s.editor.project().metadata.name.as_str())
                    .unwrap_or("Untitled")
            );
        } else {
            match self.page {
                Page::New => "New Project",
                Page::Library => "Library",
                Page::Projects => "Editor",
                Page::Info => "Info",
                Page::News => "News",
                Page::Configuration => "Setup",
                _ => "Welcome",
            }
        };
        format!("{} — {title}", qx_rendering::identity::product().name)
    }
    fn ui(
        &mut self,
        ctx: &egui::Context,
        scene: &InspectionScene,
        display: &mut DisplayOptions,
    ) -> SurfaceOutput {
        let output = if self.page == Page::Editor {
            self.session.as_mut().unwrap().ui(ctx, scene, display)
        } else {
            super::startup_ui::show(self, ctx)
        };
        self.session_events();
        output
    }
    fn poll_scene(&mut self) -> Option<InspectionScene> {
        self.poll_picker();
        self.pending_scene.take().or_else(|| {
            if self.page == Page::Editor {
                self.session.as_mut().and_then(Session::poll)
            } else {
                None
            }
        })
    }
    fn select_section(&mut self, id: qx_geometry::authoring::SectionId) {
        if self.page == Page::Editor
            && let Some(s) = &mut self.session
        {
            s.select(id);
        }
    }
    fn inspect_station(&mut self, point: Option<(qx_geometry::authoring::SectionId, f64)>) {
        if self.page == Page::Editor
            && let Some(s) = &mut self.session
        {
            s.viewport_inspection = point;
        }
    }
    fn window_preferences(&self) -> qx_rendering::WindowPreferences {
        self.session
            .as_ref()
            .map_or(&self.workbench, |s| &s.workbench)
            .window
            .clone()
    }
    fn window_changed(&mut self, prefs: qx_rendering::WindowPreferences) {
        if let Some(s) = &mut self.session {
            s.workbench.window = prefs;
        } else {
            self.workbench.window = prefs;
        }
    }
    fn persist_preferences(&mut self, force: bool) {
        if let Some(s) = &mut self.session {
            s.persist_preferences(force);
        } else {
            self.preferences.save(&self.workbench, force);
        }
        self.recent.poll(force);
    }
    fn open_path(&mut self, path: &Path) {
        if self.picker.is_some() {
            return;
        }
        if self.page == Page::Editor {
            self.session.as_mut().unwrap().open_path(path);
        } else if self.session.is_none() && self.page != Page::New {
            self.load_project(path.into(), None);
        } else {
            self.status = "Finish or cancel project setup before opening another project.".into();
        }
    }
    fn request_close(&mut self) -> bool {
        if self.picker.is_some()
            || self
                .session
                .as_ref()
                .is_some_and(|s| s.file_dialog.is_some())
        {
            return false;
        }
        if let Some(s) = &mut self.session {
            self.page = Page::Editor;
            s.request_close()
        } else {
            true
        }
    }
}
impl Session {
    pub fn request_startup(&mut self, destination: Destination) {
        if self.file_dialog.is_some() || self.graph_drag.is_some() {
            return;
        }
        if self.pending_edits() {
            self.startup_request = Some(destination);
        } else {
            self.startup_approved = Some(destination);
        }
    }
    pub fn resolve_startup(&mut self, save: bool) {
        if save {
            self.startup_after_save = true;
            self.save();
        } else {
            self.startup_approved = self.startup_request.take();
            self.startup_after_save = false;
        }
    }
}
pub(super) fn leave_dialog(s: &mut Session, ctx: &egui::Context) {
    egui::Modal::new(egui::Id::unique("leave project for startup")).show(ctx, |ui| {
        ui.heading("Unsaved edits"); ui.label(s.save_state());
        for name in s.draft_names() { ui.label(name); }
        ui.label("Save and resolve drafts, keep this project, or explicitly discard changes. Canceling new-project setup still retains this project.");
        if let Some(e) = &s.save_failure { ui.label(e); }
        if ui.button("Keep current project").clicked() { s.startup_request = None; s.startup_after_save = false; }
        if ui.button("Save & continue").clicked() { s.resolve_startup(true); }
        if ui.button("Discard and continue").clicked() { s.resolve_startup(false); }
    });
}
pub(crate) fn run(entry: Launch) -> Result<(), Box<dyn Error>> {
    let mut app = App::welcome()?;
    app.enter_launch(entry)?;
    let initial = app
        .pending_scene
        .take()
        .unwrap_or_else(InspectionScene::empty);
    qx_rendering::edit(initial, app).map_err(Into::into)
}
impl App {
    fn enter_launch(&mut self, entry: Launch) -> Result<(), Box<dyn Error>> {
        if let Launch::Editor(path) = entry {
            if let Some(path) = path {
                self.load_project(path, None);
            } else {
                let project = session::blank_project();
                let scene = session::scene(&project, &self.catalog, Default::default())?;
                let candidate = Session::new(project, None, self.catalog.clone())?;
                self.install(candidate, scene);
            }
        }
        Ok(())
    }
}
pub(crate) fn render_welcome(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut app = App::offscreen()?;
    println!(
        "{}",
        qx_rendering::render_offscreen(&InspectionScene::empty(), &mut app, path, 1280, 800)?
    );
    Ok(())
}
pub(crate) fn startup_check(
    args: &[std::ffi::OsString],
) -> Result<serde_json::Value, Box<dyn Error>> {
    let entry = launch(&std::env::current_exe()?, args);
    let mut app = App::welcome()?;
    // Verification never changes the owner's history or preferences.
    app.recent = Recent::default();
    app.enter_launch(entry.clone())?;
    Ok(
        serde_json::json!({"product": qx_rendering::identity::product().name,
        "no_arguments": match entry { Launch::Welcome => "welcome", Launch::Command => "cli-help", _ => "editor" },
            "document_open": app.session.is_some(), "project_name": app.session.as_ref().map(|s| &s.editor.project().metadata.name),
            "page": if entry == Launch::Command { "cli-help" } else { match app.page {Page::Editor => "editor", Page::Welcome => "welcome", Page::New => "new", Page::Library => "library", Page::Projects => "projects", Page::Info => "info", Page::News => "news", Page::Configuration => "setup"} },
            "status": app.status, "sample_projects": app.projects.samples.len(), "templates": app.projects.templates.len()}),
    )
}
