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
    let mut track = project
        .generate(catalog, options)
        .map_err(|e| e.to_string())?;
    let reference = match project.reference_geometry(&track) {
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
    Ok(InspectionScene {
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
    preview: bool,
}
type Job = (u64, Project, GenerationOptions, bool);
pub(super) struct Session {
    pub editor: Editor,
    pub catalog: Catalog,
    pub path: String,
    pub selected: SectionId,
    pub draft: Section,
    pub channel: ChannelKey,
    pub transition: usize,
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
    pub graph_min: f64,
    pub graph_max: f64,
    pub derived_graph: usize,
    pub initial_draft: qx_geometry::authoring::InitialConditions,
    pub metadata_draft: qx_document::Metadata,
    saved: Option<Project>,
    jobs: mpsc::SyncSender<Job>,
    queued_job: Option<Job>,
    results: mpsc::Receiver<GenerationResult>,
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
            while let Ok(mut job) = work.recv() {
                while let Ok(newer) = work.try_recv() {
                    job = newer;
                }
                let start = Instant::now();
                let result = scene(&job.1, &worker_catalog, job.2);
                if send
                    .send(GenerationResult {
                        revision: job.0,
                        result,
                        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
                        preview: job.3,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            selected: project.sections[0].id,
            draft: project.sections[0].clone(),
            channel: ChannelKey::Roll,
            transition: 0,
            heartline_draft: project.heartline.clone(),
            initial_draft: project.initial.clone(),
            metadata_draft: project.metadata.clone(),
            status: "Ready. Development controls; authored graphs drive the Rust solver.".into(),
            path: path.map_or(String::new(), |p| p.to_string_lossy().into_owned()),
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
            graph_min: -1.,
            graph_max: 3.,
            derived_graph: 0,
        })
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
            || self.draft_dirty()
            || self.heartline_draft != self.editor.project().heartline
            || self.initial_draft != self.editor.project().initial
            || self.metadata_draft != self.editor.project().metadata
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
    }
    pub fn select(&mut self, id: SectionId) {
        if self.draft_dirty() {
            self.status =
                "Apply or cancel the section draft before selecting another section.".into();
            return;
        }
        self.selected = id;
        self.sync_draft();
    }
    pub fn execute(&mut self, commands: Vec<Command>) {
        let previous_section = self
            .editor
            .project()
            .sections
            .iter()
            .find(|s| s.id == self.selected)
            .cloned();
        let previous_heartline = self.editor.project().heartline.clone();
        let previous_initial = self.editor.project().initial.clone();
        let previous_metadata = self.editor.project().metadata.clone();
        let draft = self.draft.clone();
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
                self.regenerate();
            }
            Err(error) => {
                self.status = format!("Edit rejected; document/history retained: {error}")
            }
        }
    }
    pub fn apply_draft(&mut self) {
        self.execute(vec![Command::ReplaceSection {
            section: self.draft.clone(),
        }]);
    }
    pub fn regenerate(&mut self) {
        self.queue_generation(self.editor.project().clone(), false);
    }
    pub fn preview(&mut self) {
        let mut candidate = self.editor.project().clone();
        if let Some(section) = candidate
            .sections
            .iter_mut()
            .find(|s| s.id == self.selected)
        {
            *section = self.draft.clone();
        }
        candidate.heartline = self.heartline_draft.clone();
        self.queue_generation(candidate, true);
    }
    fn queue_generation(&mut self, project: Project, preview: bool) {
        self.revision += 1;
        self.queued_job = None;
        match self
            .jobs
            .try_send((self.revision, project, self.options, preview))
        {
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
                    self.status = format!(
                        "{} revision {} generated in {:.2} ms; {:.3} m, {} samples",
                        if result.preview {
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
                    )
                }
            }
        }
        latest
    }
    pub fn undo(&mut self, redo: bool) {
        if self.has_drafts() {
            self.status = "Apply or cancel drafts before undo/redo.".into();
            return;
        }
        if if redo {
            self.editor.redo()
        } else {
            self.editor.undo()
        } {
            self.sync_draft();
            self.heartline_draft = self.editor.project().heartline.clone();
            self.initial_draft = self.editor.project().initial.clone();
            self.metadata_draft = self.editor.project().metadata.clone();
            self.regenerate();
        }
    }
    pub fn save(&mut self) {
        if self.has_drafts() {
            self.status = "Apply or cancel drafts before saving.".into();
            return;
        }
        if self.path.trim().is_empty() {
            self.status = "Enter a project save path first.".into();
            return;
        }
        match persistence::save(Path::new(&self.path), self.editor.project(), &self.catalog) {
            Ok(()) => {
                self.saved = Some(self.editor.project().clone());
                self.status = "Project saved (schema 3).".into();
            }
            Err(error) => self.status = format!("Save failed; document retained: {error}"),
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
        match result.and_then(|p| Editor::new(p, self.catalog.clone(), 64)) {
            Ok(editor) => {
                self.editor = editor;
                self.saved = path.as_ref().map(|_| self.editor.project().clone());
                self.path = path.map_or(String::new(), |p| p.to_string_lossy().into_owned());
                self.selected = self.editor.project().sections[0].id;
                self.sync_draft();
                self.heartline_draft = self.editor.project().heartline.clone();
                self.initial_draft = self.editor.project().initial.clone();
                self.metadata_draft = self.editor.project().metadata.clone();
                self.regenerate();
            }
            Err(error) => self.status = format!("Open failed; current document retained: {error}"),
        }
    }
    pub fn has_drafts(&self) -> bool {
        self.draft_dirty()
            || self.heartline_draft != self.editor.project().heartline
            || self.initial_draft != self.editor.project().initial
            || self.metadata_draft != self.editor.project().metadata
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
