use qx_document::{
    HeartlineSettings, Project,
    command::{Command, Editor},
    persistence,
};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionId, SectionKind},
    solver::GenerationOptions,
};
use qx_rendering::{InspectionScene, SectionLabel};
use qx_style::Catalog;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    time::Instant,
};

pub(super) fn scene(
    project: &Project,
    catalog: &Catalog,
    options: GenerationOptions,
) -> Result<InspectionScene, String> {
    scene_cached(project, catalog, options, None)
}
fn scene_cached(
    project: &Project,
    catalog: &Catalog,
    options: GenerationOptions,
    cache: Option<&(Project, GenerationOptions, InspectionScene)>,
) -> Result<InspectionScene, String> {
    let same_path = cache.is_some_and(|(p, o, _)| {
        *o == options && p.initial == project.initial && p.sections == project.sections
    });
    let solver_start = Instant::now();
    let mut track = if same_path {
        cache.unwrap().2.track.clone()
    } else {
        project
            .generate(catalog, options)
            .map_err(|e| e.to_string())?
    };
    track.diagnostics.retain(|d| d.code != "reference_error");
    let solver_ms = solver_start.elapsed().as_secs_f64() * 1000.;
    let reference = if same_path && cache.unwrap().0.heartline == project.heartline {
        cache.unwrap().2.reference.clone()
    } else {
        match project.reference_geometry(&track) {
            Ok(reference) => Some(reference),
            Err(error) => {
                track.diagnostics.push(qx_geometry::solver::Diagnostic {
                    section_id: project.sections[0].id,
                    input: None,
                    code: "reference_error",
                    message: format!("Reference geometry unavailable: {error}"),
                });
                None
            }
        }
    };
    let labels = project
        .sections
        .iter()
        .map(|s| SectionLabel {
            id: s.id,
            name: s.name.clone(),
            method: method(s).into(),
        })
        .collect();
    let start = Instant::now();
    let mesh = if same_path
        && cache.unwrap().0.heartline == project.heartline
        && cache.unwrap().0.construction == project.construction
    {
        cache.unwrap().2.mesh.clone().unwrap()
    } else {
        std::sync::Arc::new(
            if project.heartline.authored_reference
                == qx_geometry::heartline::AuthoredReference::RiderReference
            {
                qx_geometry::construction::build_reference(
                    &track,
                    reference
                        .as_ref()
                        .ok_or("Physical track inverse unavailable; previous mesh retained")?,
                    &project.construction,
                    Default::default(),
                )
            } else {
                qx_geometry::construction::build(&track, &project.construction, Default::default())
            }
            .map_err(|e| e.to_string())?,
        )
    };
    Ok(InspectionScene {
        mesh: Some(mesh),
        mesh_ms: start.elapsed().as_secs_f64() * 1000.,
        solver_ms,
        name: project.metadata.name.clone(),
        track,
        labels,
        reference,
        authored_reference: project.heartline.authored_reference,
    })
}
pub(super) fn method(section: &Section) -> &'static str {
    match &section.kind {
        SectionKind::Spline { curve } => curve.method(),
        SectionKind::Force { .. } => "force-vector",
        SectionKind::Geometric { .. } => "world-geometric",
        SectionKind::RiderLocal { .. } => "rider-local",
        SectionKind::Straight {} => "straight",
        SectionKind::Arc { .. } => "arc",
    }
}
struct GenerationResult {
    revision: u64,
    result: Result<InspectionScene, String>,
    elapsed_ms: f64,
}
type Job = (u64, Project, GenerationOptions);
pub(super) struct Session {
    pub workbench: super::workbench::Workbench,
    pub preferences: super::workbench::PreferenceStore,
    pub display_dialog: bool,
    pub window_draft: qx_rendering::WindowPreferences,
    pub editor: Editor,
    pub catalog: Catalog,
    pub path: String,
    pub file_dialog: Option<super::files::Dialog>,
    pub selected: SectionId,
    pub next_section_id: Option<u64>,
    pub draft: Section,
    pub channel: ChannelKey,
    pub transition: usize,
    pub track_tab: usize,
    pub heartline_draft: HeartlineSettings,
    pub status: String,
    pub revision: u64,
    pub visible_revision: u64,
    pub options: GenerationOptions,
    pub split_fraction: f64,
    pub new_kind: usize,
    pub confirm_replace: Option<Option<PathBuf>>,
    pub close_requested: bool,
    pub close_approved: bool,
    pub graph_view: super::graph_view::State,
    pub derived_graph: usize,
    pub graph_drag: Option<super::graph_gesture::Drag>,
    pub graph_feedback: Option<String>,
    pub graph_readout: Option<String>,
    pub pending_action: Option<super::workflow::PendingAction>,
    pub draft_error: Option<String>,
    pub visible_preview: bool,
    pub frame_requested: bool,
    pub inspect_input: Option<(SectionId, f64)>,
    pub hovered: Option<SectionId>,
    pub viewport_inspection: Option<(SectionId, f64)>,
    pub train_station_m: Option<f64>,
    pub playback: super::train_playback::State,
    pub assets: super::train_assets::State,
    pub keyboard_owned_last_frame: bool,
    pub apply_section_requested: bool,
    pub initial_draft: qx_geometry::authoring::InitialConditions,
    pub metadata_draft: qx_document::Metadata,
    pub construction_draft: qx_style::construction::TrackConstruction,
    pub draft_request: Option<super::draft_workflow::DraftRequest>,
    pub resolution_error: Option<String>,
    pub save_failure: Option<String>,
    pub open_dialog_requested: bool,
    pub open_dialog_confirmed: bool,
    pub ui_action: Option<super::draft_workflow::UiAction>,
    saved: Option<Project>,
    jobs: mpsc::SyncSender<Job>,
    queued_job: Option<Job>,
    results: mpsc::Receiver<GenerationResult>,
    last_job: Option<(Project, GenerationOptions, bool)>,
    frame_revision: Option<u64>,
}
impl Session {
    pub fn new(
        project: Project,
        path: Option<&Path>,
        catalog: Catalog,
    ) -> qx_document::Result<Self> {
        let editor = Editor::new(project.clone(), catalog.clone(), 64)?;
        let (jobs, work) = mpsc::sync_channel::<Job>(1);
        let (send, results) = mpsc::channel();
        let worker_catalog = catalog.clone();
        std::thread::spawn(move || {
            let mut cache = None;
            while let Ok(mut job) = work.recv() {
                while let Ok(newer) = work.try_recv() {
                    job = newer;
                }
                let start = Instant::now();
                let result = scene_cached(&job.1, &worker_catalog, job.2, cache.as_ref());
                if let Ok(scene) = &result {
                    cache = Some((job.1, job.2, scene.clone()));
                }
                if send
                    .send(GenerationResult {
                        revision: job.0,
                        result,
                        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut session = Self {
            workbench: Default::default(),
            preferences: Default::default(),
            display_dialog: false,
            window_draft: Default::default(),
            selected: project.sections[0].id,
            next_section_id: project
                .sections
                .iter()
                .map(|s| s.id.0)
                .max()
                .unwrap_or(0)
                .checked_add(1),
            draft: project.sections[0].clone(),
            channel: ChannelKey::Roll,
            transition: 0,
            track_tab: 0,
            heartline_draft: project.heartline.clone(),
            initial_draft: project.initial.clone(),
            metadata_draft: project.metadata.clone(),
            construction_draft: project.construction.clone(),
            draft_request: None,
            resolution_error: None,
            save_failure: None,
            open_dialog_requested: false,
            open_dialog_confirmed: false,
            ui_action: None,
            status: "Ready. Select a section to author its transitions.".into(),
            path: path.map_or(String::new(), |p| p.to_string_lossy().into_owned()),
            file_dialog: None,
            saved: path.map(|_| project),
            editor,
            catalog,
            jobs,
            queued_job: None,
            results,
            revision: 0,
            visible_revision: 0,
            options: GenerationOptions::default(),
            split_fraction: 0.5,
            new_kind: 0,
            confirm_replace: None,
            close_requested: false,
            close_approved: false,
            graph_view: Default::default(),
            derived_graph: 0,
            graph_drag: None,
            graph_feedback: None,
            graph_readout: None,
            pending_action: None,
            draft_error: None,
            visible_preview: false,
            frame_requested: true,
            inspect_input: None,
            hovered: None,
            viewport_inspection: None,
            train_station_m: None,
            playback: Default::default(),
            assets: Default::default(),
            keyboard_owned_last_frame: false,
            apply_section_requested: false,
            last_job: None,
            frame_revision: None,
        };
        super::graph_view::sync(&mut session);
        session
            .assets
            .remember(session.editor.project(), &session.path);
        Ok(session)
    }
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(self.editor.project())
    }
    pub fn draft_dirty(&self) -> bool {
        self.editor
            .project()
            .sections
            .iter()
            .find(|s| s.id == self.selected)
            != Some(&self.draft)
    }
    pub fn pending_edits(&self) -> bool {
        self.dirty()
            || self.assets.draft_dirty(self.editor.project())
            || self.draft_dirty()
            || self.heartline_draft != self.editor.project().heartline
            || self.initial_draft != self.editor.project().initial
            || self.metadata_draft != self.editor.project().metadata
            || self.construction_draft != self.editor.project().construction
    }
    pub fn sync_draft(&mut self) {
        let selected = self
            .editor
            .project()
            .sections
            .iter()
            .find(|s| s.id == self.selected)
            .unwrap_or(&self.editor.project().sections[0]);
        self.selected = selected.id;
        self.draft = selected.clone();
        if !qx_document::editing::channel_keys(selected).contains(&self.channel) {
            self.channel = ChannelKey::Roll;
        }
        self.transition = self.transition.min(
            self.draft
                .channel_mut(self.channel)
                .unwrap()
                .transitions
                .len()
                - 1,
        );
        super::graph_view::sync(self);
    }
    pub fn select(&mut self, id: SectionId) {
        if id == self.selected {
            return;
        }
        if !self.editor.project().sections.iter().any(|s| s.id == id) {
            self.status = "Selected section is no longer in this document.".into();
            return;
        }
        if self.defer_section_action(super::workflow::PendingAction::Select(id)) {
            return;
        }
        self.selected = id;
        self.graph_feedback = None;
        self.inspect_input = None;
        self.pending_action = None;
        self.sync_draft();
    }
    pub fn select_channel(&mut self, key: ChannelKey) {
        if self.graph_drag.is_none()
            && key != self.channel
            && qx_document::editing::channel_keys(&self.draft).contains(&key)
        {
            self.channel = key;
            self.graph_feedback = None;
            self.transition = 0;
            super::graph_view::sync(self);
        }
    }
    pub fn execute(&mut self, mut commands: Vec<Command>) -> bool {
        let previous_section = self
            .editor
            .project()
            .sections
            .iter()
            .find(|s| s.id == self.selected)
            .cloned();
        let previous_heartline = self.editor.project().heartline.clone();
        let previous_initial = self.editor.project().initial.clone();
        let previous_construction = self.editor.project().construction.clone();
        let previous_metadata = self.editor.project().metadata.clone();
        let draft = self.draft.clone();
        // Explicit acceptance validates the complete candidate, including strict
        // downstream boundaries, before publishing either document or history.
        // Live previews remain on the coalescing worker. This bounded preflight
        // runs only at a transaction boundary, never on ordinary pointer motion.
        if commands.iter().any(|c| {
            !matches!(
                c,
                Command::SetMetadata(_)
                    | Command::SetStyle(_)
                    | Command::SetHeartline(_)
                    | Command::SetTrains(_)
                    | Command::SetConstruction(_)
            )
        }) {
            commands.push(Command::ValidateConnections {
                options: self.options,
            });
        }
        match self.editor.execute(commands) {
            Ok(()) => {
                self.sync_draft();
                if self
                    .editor
                    .project()
                    .sections
                    .iter()
                    .find(|s| s.id == self.selected)
                    == previous_section.as_ref()
                {
                    self.draft = draft;
                }
                if self.editor.project().heartline != previous_heartline {
                    self.heartline_draft = self.editor.project().heartline.clone();
                }
                if self.editor.project().initial != previous_initial {
                    self.initial_draft = self.editor.project().initial.clone();
                }
                if self.editor.project().metadata != previous_metadata {
                    self.metadata_draft = self.editor.project().metadata.clone();
                }
                if self.editor.project().construction != previous_construction {
                    self.construction_draft = self.editor.project().construction.clone();
                }
                self.refresh_view();
                self.draft_error = None;
                self.graph_feedback = None;
                true
            }
            Err(error) => {
                // Results queued before the rejected inputs must not erase the
                // actionable failure or publish a now-unrelated preview.
                self.revision += 1;
                self.queued_job = None;
                self.last_job = None;
                self.status = format!("Edit rejected; document/history retained: {error}");
                self.draft_error = Some(self.status.clone());
                false
            }
        }
    }
    pub fn apply_draft(&mut self) -> bool {
        let accepted = self.execute(vec![Command::ReplaceSection {
            section: self.draft.clone(),
        }]);
        if accepted {
            self.pending_action = None;
        }
        accepted
    }
    pub fn regenerate(&mut self) {
        self.queue_generation(self.editor.project().clone(), false);
    }
    pub fn refresh_view(&mut self) {
        if self.draft_dirty()
            || self.heartline_draft != self.editor.project().heartline
            || self.initial_draft != self.editor.project().initial
            || self.construction_draft != self.editor.project().construction
        {
            self.preview();
        } else {
            self.regenerate();
        }
    }
    pub fn preview(&mut self) {
        self.queue_generation(self.preview_project(), true);
    }
    pub fn preview_project(&self) -> Project {
        let mut candidate = self.editor.project().clone();
        if let Some(section) = candidate
            .sections
            .iter_mut()
            .find(|s| s.id == self.selected)
        {
            *section = self.draft.clone();
        }
        candidate.heartline = self.heartline_draft.clone();
        candidate.initial = self.initial_draft.clone();
        candidate.construction = self.construction_draft.clone();
        if let (Some((train, component)), Some(draft)) = (&self.assets.selected, &self.assets.draft)
            && let Some(c) = candidate
                .trains
                .iter_mut()
                .find(|t| &t.id == train)
                .and_then(|t| t.components.iter_mut().find(|c| &c.id == component))
        {
            let mut asset = draft.clone();
            if let Ok(q) = asset.rotation.normalized() {
                asset.rotation = q;
            }
            c.asset = Some(asset);
        }
        candidate
    }
    fn queue_generation(&mut self, project: Project, preview: bool) {
        if let Some((p, o, was_preview)) = &mut self.last_job
            && p == &project
            && *o == self.options
        {
            // Acceptance changes the role of identical geometry, not its inputs.
            // Reuse an in-flight or published preview instead of generating twice.
            if *was_preview != preview
                && self.visible_revision == self.revision
                && self.draft_error.is_none()
            {
                self.visible_preview = preview;
                self.status = self.status.replace(
                    if preview {
                        "Document"
                    } else {
                        "DRAFT PREVIEW (Apply to save)"
                    },
                    if preview {
                        "DRAFT PREVIEW (Apply to save)"
                    } else {
                        "Document"
                    },
                );
            }
            *was_preview = preview;
            return;
        }
        self.last_job = Some((project.clone(), self.options, preview));
        self.draft_error = None;
        self.revision += 1;
        if self.frame_revision.is_some() {
            self.frame_revision = Some(self.revision);
        }
        self.queued_job = None;
        match self.jobs.try_send((self.revision, project, self.options)) {
            Ok(()) => self.status = format!("Generating revision {}…", self.revision),
            Err(mpsc::TrySendError::Full(job)) => {
                self.queued_job = Some(job);
                self.status = format!("Queued latest revision {}…", self.revision);
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.status = "Generation worker stopped; save and restart.".into()
            }
        }
    }
    pub fn poll(&mut self) -> Option<InspectionScene> {
        if let Some(job) = self.queued_job.take() {
            match self.jobs.try_send(job) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(job)) => self.queued_job = Some(job),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    self.status = "Generation worker stopped; save and restart.".into()
                }
            }
        }
        let mut latest = None;
        while let Ok(result) = self.results.try_recv() {
            if result.revision != self.revision {
                continue;
            }
            match result.result {
                Ok(scene) => {
                    self.visible_revision = result.revision;
                    self.visible_preview = self
                        .last_job
                        .as_ref()
                        .is_some_and(|(_, _, preview)| *preview);
                    self.draft_error = None;
                    if self.frame_revision == Some(result.revision) {
                        self.frame_requested = true;
                        self.frame_revision = None;
                    }
                    self.status = format!(
                        "{} revision {} generated in {:.2} ms; {:.3} m, {} samples",
                        if self.visible_preview {
                            "DRAFT PREVIEW (Apply to save)"
                        } else {
                            "Document"
                        },
                        result.revision,
                        result.elapsed_ms,
                        scene.track.length_m(),
                        scene.track.samples.len()
                    );
                    latest = Some(scene);
                }
                Err(error) => {
                    self.status = format!(
                        "Generation failed for revision {}: {error}. Previous view retained; undo or revise inputs.",
                        result.revision
                    );
                    self.draft_error = Some(self.status.clone());
                }
            }
        }
        latest
    }
    pub fn undo(&mut self, redo: bool) {
        if self.has_drafts() {
            self.request_drafts(super::draft_workflow::DraftRequest::History { redo });
            return;
        }
        self.assets.draft = None;
        if if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        } {
            self.graph_feedback = None;
            self.sync_draft();
            self.heartline_draft = self.editor.project().heartline.clone();
            self.initial_draft = self.editor.project().initial.clone();
            self.metadata_draft = self.editor.project().metadata.clone();
            self.construction_draft = self.editor.project().construction.clone();
            self.regenerate();
        }
    }
    pub fn save(&mut self) {
        if self.has_drafts() {
            self.request_drafts(super::draft_workflow::DraftRequest::Save);
            return;
        }
        if self.path.trim().is_empty() {
            self.file_dialog(super::files::FileAction::SaveAs);
            return;
        }
        self.save_to(&PathBuf::from(&self.path));
    }
    pub fn save_to(&mut self, path: &Path) {
        if self.has_drafts() {
            self.request_drafts(super::draft_workflow::DraftRequest::SaveTo(
                path.to_path_buf(),
            ));
            return;
        }
        let mut warnings = Vec::new();
        let result = self
            .assets
            .package(self.editor.project(), &self.path, path)
            .map_err(qx_document::DocumentError)
            .and_then(|w| {
                warnings = w;
                persistence::save(path, self.editor.project(), &self.catalog)
            });
        match result {
            Ok(()) => {
                self.path = path.to_string_lossy().into_owned();
                self.saved = Some(self.editor.project().clone());
                self.save_failure = None;
                self.status = format!("Project saved (schema 5). {}", warnings.join("; "));
            }
            Err(error) => {
                self.status = format!(
                    "Save failed for {}; document retained: {error}",
                    path.display()
                );
                self.save_failure = Some(self.status.clone());
            }
        }
    }
    pub fn open(&mut self, path: Option<PathBuf>, confirmed: bool) {
        if self.pending_edits() && !confirmed {
            self.confirm_replace = Some(path);
            return;
        }
        let result = match &path {
            Some(path) => persistence::load(path, &self.catalog),
            None => Ok(blank_project()),
        };
        match result.and_then(|p| {
            p.generate(&self.catalog, self.options)?;
            Editor::new(p, self.catalog.clone(), 64)
        }) {
            Ok(editor) => {
                self.graph_view = Default::default();
                self.graph_drag = None;
                self.graph_feedback = None;
                self.pending_action = None;
                self.inspect_input = None;
                self.hovered = None;
                self.viewport_inspection = None;
                self.train_station_m = None;
                self.playback = Default::default();
                self.assets = Default::default();
                self.last_job = None;
                self.draft_request = None;
                self.resolution_error = None;
                self.save_failure = None;
                self.confirm_replace = None;
                self.close_requested = false;
                self.editor = editor;
                self.saved = path.as_ref().map(|_| self.editor.project().clone());
                self.path = path.map_or(String::new(), |p| p.to_string_lossy().into_owned());
                self.assets.remember(self.editor.project(), &self.path);
                self.selected = self.editor.project().sections[0].id;
                self.next_section_id = self
                    .editor
                    .project()
                    .sections
                    .iter()
                    .map(|s| s.id.0)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1);
                self.sync_draft();
                self.heartline_draft = self.editor.project().heartline.clone();
                self.initial_draft = self.editor.project().initial.clone();
                self.metadata_draft = self.editor.project().metadata.clone();
                self.construction_draft = self.editor.project().construction.clone();
                self.regenerate();
                self.frame_revision = Some(self.revision);
            }
            Err(error) => self.status = format!("Open failed; current document retained: {error}"),
        }
    }
    pub fn has_drafts(&self) -> bool {
        self.draft_dirty()
            || self.assets.draft_dirty(self.editor.project())
            || self.heartline_draft != self.editor.project().heartline
            || self.initial_draft != self.editor.project().initial
            || self.metadata_draft != self.editor.project().metadata
            || self.construction_draft != self.editor.project().construction
    }
}
pub(super) fn blank_project() -> Project {
    let mut p = qx_document::sample::foundation_project();
    p.metadata.project_id = format!(
        "qx-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    p.metadata.name = "Untitled".into();
    p.metadata.description.clear();
    p.metadata.author.clear();
    p.sections = vec![Section::straight(1, "Section 1", 10.)];
    p
}
