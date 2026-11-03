//! Contextual construction and operation drafts in the existing Editor.
use super::session::Session;
use qx_document::{Project, command::Command, operations::Operations};
use qx_geometry::{
    supports::Structure,
    topology::{Completion, CompletionSettings},
};

type TrackCache = (
    qx_geometry::authoring::InitialConditions,
    Vec<qx_geometry::authoring::Section>,
    qx_geometry::solver::GenerationOptions,
    Result<std::sync::Arc<qx_geometry::solver::GeneratedTrack>, String>,
);
pub(super) struct State {
    support_track_cache: Option<(u64, std::sync::Arc<qx_geometry::solver::GeneratedTrack>)>,
    track_cache: Option<TrackCache>,
    pub engineering_cache: Option<(
        u64,
        f64,
        std::sync::Arc<Vec<qx_geometry::engineering::EngineeringRow>>,
    )>,
    pub completion_job: Option<CompletionJob>,
    pub operations: Operations,
    pub supports: Vec<Structure>,
    pub completion: Option<(
        Vec<qx_geometry::authoring::Section>,
        qx_geometry::authoring::InitialConditions,
        Completion,
    )>,
    pub completion_settings: CompletionSettings,
    pub message: Option<String>,
    pub structure: Option<u64>,
    pub node: Option<u64>,
    pub member: Option<u64>,
    pub device: Option<u64>,
    pub curve_device: Option<u64>,
    pub curve_transition: usize,
    pub timber: qx_geometry::supports::TimberRecipe,
    pub engineering: bool,
    pub engineering_channel: usize,
    pub whole_track: bool,
    pub selected_station_m: Option<f64>,
    pub base_operations: Operations,
    pub base_supports: Vec<Structure>,
}
pub(super) struct CompletionJob {
    source: Vec<qx_geometry::authoring::Section>,
    initial: qx_geometry::authoring::InitialConditions,
    receiver: std::sync::mpsc::Receiver<Result<Completion, String>>,
}
impl State {
    pub fn new(p: &Project) -> Self {
        Self {
            support_track_cache: None,
            track_cache: None,
            engineering_cache: None,
            completion_job: None,
            operations: p.operations.clone(),
            supports: p.supports.clone(),
            base_operations: p.operations.clone(),
            base_supports: p.supports.clone(),
            completion: None,
            completion_settings: Default::default(),
            message: None,
            structure: None,
            node: None,
            member: None,
            device: None,
            curve_device: None,
            curve_transition: 0,
            timber: Default::default(),
            engineering: false,
            engineering_channel: 0,
            whole_track: true,
            selected_station_m: None,
        }
    }
    pub fn committed_track(
        &mut self,
        p: &Project,
        options: qx_geometry::solver::GenerationOptions,
    ) -> Result<std::sync::Arc<qx_geometry::solver::GeneratedTrack>, String> {
        if self
            .track_cache
            .as_ref()
            .is_none_or(|(i, s, o, _)| i != &p.initial || s != &p.sections || *o != options)
        {
            let result = qx_geometry::solver::generate(&p.initial, &p.sections, options)
                .map(std::sync::Arc::new)
                .map_err(|e| e.to_string());
            self.track_cache = Some((p.initial.clone(), p.sections.clone(), options, result));
        }
        self.track_cache.as_ref().unwrap().3.clone()
    }
    pub fn attachment_track(
        &mut self,
        scene: &qx_rendering::InspectionScene,
        revision: u64,
    ) -> Result<std::sync::Arc<qx_geometry::solver::GeneratedTrack>, String> {
        if self
            .support_track_cache
            .as_ref()
            .is_none_or(|(r, _)| *r != revision)
        {
            let track = if scene.authored_reference
                == qx_geometry::heartline::AuthoredReference::RiderReference
            {
                qx_geometry::supports::physical_attachment_track(
                    &scene.track,
                    scene
                        .reference
                        .as_ref()
                        .ok_or("physical reference unavailable")?,
                )
                .map_err(|e| e.to_string())?
            } else {
                scene.track.clone()
            };
            self.support_track_cache = Some((revision, std::sync::Arc::new(track)));
        }
        Ok(self.support_track_cache.as_ref().unwrap().1.clone())
    }
    pub fn dirty(&self, p: &Project) -> bool {
        self.operations != p.operations
            || self.supports != p.supports
            || self.completion.is_some()
            || self.completion_job.is_some()
    }
    pub fn sync(&mut self, p: &Project) {
        if self.base_operations != p.operations {
            self.operations = p.operations.clone();
            self.base_operations = p.operations.clone();
        }
        if self.base_supports != p.supports {
            self.supports = p.supports.clone();
            self.base_supports = p.supports.clone();
        }
        if self
            .completion
            .as_ref()
            .is_some_and(|(s, i, _)| s != &p.sections || i != &p.initial)
        {
            self.completion = None;
            self.message = Some("Track changed; solve a new connector candidate.".into());
        }
    }
    pub fn commands(&self, p: &Project) -> Result<Vec<Command>, String> {
        if self.completion_job.is_some() {
            return Err("Wait for the bounded completion solve, or Cancel it.".into());
        }
        let mut out = vec![];
        if let Some((source, initial, c)) = &self.completion {
            if source != &p.sections || initial != &p.initial {
                return Err("connector candidate is stale; solve again".into());
            }
            out.push(Command::InsertSection {
                after: p.sections.last().map(|s| s.id),
                section: c.section.clone(),
            });
        }
        if self.operations != p.operations {
            out.push(Command::SetOperations(self.operations.clone()));
        }
        if self.supports != p.supports {
            out.push(Command::SetSupports(self.supports.clone()));
        }
        Ok(out)
    }
}
impl Session {
    pub fn start_completion(&mut self) {
        if self.draft_dirty() || self.initial_draft != self.editor.project().initial {
            self.expansion.message =
                Some("Apply or cancel the track draft before solving a connector.".into());
            return;
        }
        let p = self.editor.project();
        let Some(id) = p
            .sections
            .iter()
            .map(|s| s.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
        else {
            self.expansion.message = Some("Region IDs exhausted".into());
            return;
        };
        let source = p.sections.clone();
        let initial = p.initial.clone();
        let (send, receiver) = std::sync::mpsc::channel();
        let (sections, conditions, settings, mode, options) = (
            source.clone(),
            initial.clone(),
            self.expansion.completion_settings,
            self.expansion.operations.layout,
            self.options,
        );
        std::thread::spawn(move || {
            let result = qx_geometry::topology::complete(
                &conditions,
                &sections,
                mode,
                id,
                settings,
                options,
            )
            .map_err(|e| e.to_string());
            let _ = send.send(result);
        });
        self.expansion.completion_job = Some(CompletionJob {
            source,
            initial,
            receiver,
        });
        self.expansion.message = Some("Searching up to 41 connector candidates…".into());
    }
    pub fn poll_completion(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.expansion.completion_job else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
                return;
            }
            Err(_) => Err("Completion worker stopped; layout retained.".into()),
        };
        let job = self.expansion.completion_job.take().unwrap();
        if job.source != self.editor.project().sections
            || job.initial != self.editor.project().initial
            || self.draft_dirty()
        {
            self.expansion.message =
                Some("Track changed during solve; candidate discarded. Solve again.".into());
            return;
        }
        match result {
            Ok(c) => {
                self.expansion.message = Some(format!(
                    "Preview: {:.3} m connector · gap {:.8} m · frame {:.8} rad. Apply keeps it; Cancel removes it. G1 pose closure; inspect join curvature.",
                    c.length_m, c.closure.position_gap_m, c.closure.frame_gap_rad
                ));
                self.expansion.completion = Some((job.source, job.initial, c));
                self.expansion.operations.layout = qx_geometry::topology::LayoutMode::Circuit;
                self.refresh_view();
            }
            Err(e) => self.expansion.message = Some(e),
        }
    }
    pub fn apply_expansion(&mut self) -> bool {
        let commands = match self.expansion.commands(self.editor.project()) {
            Ok(c) => c,
            Err(e) => {
                self.expansion.message = Some(e);
                return false;
            }
        };
        if self.expansion.completion.is_some()
            && (self.draft_dirty() || self.initial_draft != self.editor.project().initial)
        {
            self.expansion.message =
                Some("Track draft changed; cancel it or solve a new connector.".into());
            return false;
        }
        let mut candidate = self.editor.project().clone();
        candidate.operations = self.expansion.operations.clone();
        candidate.supports = self.expansion.supports.clone();
        if let Some((_, _, c)) = &self.expansion.completion {
            candidate.sections.push(c.section.clone());
        }
        if let Err(e) = super::session::scene(&candidate, &self.catalog, self.options) {
            self.expansion.message = Some(format!("Nothing applied: {e}"));
            return false;
        }
        if self.execute(commands) {
            self.expansion.completion = None;
            self.expansion.sync(self.editor.project());
            self.refresh_view();
            self.expansion.message = Some("Changes applied as one undo step.".into());
            true
        } else {
            self.expansion.message = Some(self.status.clone());
            false
        }
    }
    pub fn cancel_expansion(&mut self) {
        self.expansion.completion_job = None;
        self.expansion.operations = self.editor.project().operations.clone();
        self.expansion.supports = self.editor.project().supports.clone();
        self.expansion.completion = None;
        self.expansion.message = None;
        self.refresh_view();
    }
}
pub(super) fn actions(s: &mut Session, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let dirty = s.expansion.dirty(s.editor.project()) || s.expansion.completion_job.is_some();
        if ui
            .add_enabled(
                dirty && s.expansion.completion_job.is_none(),
                egui::Button::new("Apply construction / operations"),
            )
            .clicked()
        {
            s.apply_expansion();
        }
        if ui.add_enabled(dirty, egui::Button::new("Cancel")).clicked() {
            s.cancel_expansion();
        }
    });
    if let Some(e) = &s.expansion.message {
        ui.label(e);
    }
}
