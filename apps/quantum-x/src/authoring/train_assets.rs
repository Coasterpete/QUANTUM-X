//! Transient import library and project-relative packaging. All expensive file,
//! parse and image work runs on one worker; assignments are document commands.
use super::session::Session;
use qx_document::{Project, train::Asset};
use qx_geometry::math::{Frame, Quaternion, Vec3};
use qx_rendering::asset::{self, AssetInstance, Model};
use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

pub(super) struct Entry {
    pub model: Arc<Model>,
    pub bytes: Arc<Vec<u8>>,
}
struct Complete {
    key: Option<String>,
    path: PathBuf,
    result: Result<Entry, String>,
}
pub(super) struct State {
    pub library: BTreeMap<String, Entry>,
    pub errors: BTreeMap<String, String>,
    sources: BTreeMap<String, PathBuf>,
    pending: HashSet<String>,
    jobs: mpsc::Sender<(Option<String>, PathBuf)>,
    results: mpsc::Receiver<Complete>,
    pub importing: bool,
    pub selected: Option<(String, String)>,
    pub chosen: String,
    pub draft: Option<Asset>,
    pub wireframe: bool,
    pub bogies: bool,
    pub wheels: bool,
    pub joints: bool,
    pub couplers: bool,
    pub fit: bool,
    pub focus: bool,
    pub fallback: Arc<Model>,
}
impl Default for State {
    fn default() -> Self {
        let (jobs, rx) = mpsc::channel::<(Option<String>, PathBuf)>();
        let (tx, results) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok((key, path)) = rx.recv() {
                let result = load(&path);
                if tx.send(Complete { key, path, result }).is_err() {
                    break;
                }
            }
        });
        Self {
            library: BTreeMap::new(),
            errors: BTreeMap::new(),
            sources: BTreeMap::new(),
            pending: HashSet::new(),
            jobs,
            results,
            importing: false,
            selected: None,
            chosen: String::new(),
            draft: None,
            wireframe: true,
            bogies: true,
            wheels: true,
            joints: true,
            couplers: true,
            fit: false,
            focus: false,
            fallback: Arc::new(Model::development_box()),
        }
    }
}
fn load(path: &Path) -> Result<Entry, String> {
    let bytes = asset::read_glb(path)?;
    let model = asset::import_glb(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Entry {
        model: Arc::new(model),
        bytes: Arc::new(bytes),
    })
}
pub(super) fn root(path: &str) -> Option<PathBuf> {
    if path.trim().is_empty() {
        None
    } else {
        Some(
            Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .to_path_buf(),
        )
    }
}
pub(super) fn identity(path: String) -> Asset {
    Asset {
        path,
        translation_m: Vec3::ZERO,
        rotation: Quaternion::IDENTITY,
        scale: Vec3::new(1., 1., 1.),
        pivot_m: Vec3::ZERO,
    }
}
impl State {
    pub fn draft_dirty(&self, project: &Project) -> bool {
        let current = self
            .selected
            .as_ref()
            .and_then(|(t, c)| {
                project
                    .trains
                    .iter()
                    .find(|v| &v.id == t)
                    .and_then(|t| t.components.iter().find(|v| &v.id == c))
            })
            .and_then(|c| c.asset.as_ref());
        self.draft.as_ref().is_some_and(|a| Some(a) != current)
    }
    pub fn remember(&mut self, project: &Project, project_path: &str) {
        if let Some(base) = root(project_path) {
            for a in project
                .trains
                .iter()
                .flat_map(|t| &t.components)
                .filter_map(|c| c.asset.as_ref())
            {
                self.sources
                    .entry(a.path.clone())
                    .or_insert_with(|| base.join(&a.path));
            }
        }
    }
    fn insert(&mut self, key: String, entry: Entry) -> Result<(), String> {
        let used: usize = self
            .library
            .values()
            .map(|e| e.bytes.len() + e.model.cpu_bytes())
            .sum();
        let added = entry.bytes.len() + entry.model.cpu_bytes();
        if self.library.len() >= 64 || used + added > 256 * 1024 * 1024 {
            return Err(
                "Train asset library exceeds 64 assets or 256 MiB; open a smaller project".into(),
            );
        }
        self.errors.remove(&key);
        self.library.insert(key, entry);
        Ok(())
    }
    fn imported(
        &mut self,
        path: &Path,
        entry: Entry,
        project_path: &str,
    ) -> Result<String, String> {
        if let Some((key, _)) = self.library.iter().find(|(_, e)| e.bytes == entry.bytes) {
            return Ok(key.clone());
        }
        let stem: String = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(48)
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let hash = entry.bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ (*b as u64)).wrapping_mul(0x100000001b3)
        });
        let key = format!("assets/train/{stem}-{hash:016x}.glb");
        if let Some(base) = root(project_path) {
            let target = base.join(&key);
            if target.exists() && asset::read_glb(&target)? != *entry.bytes {
                return Err(
                    "Asset name collision on disk; rename the source GLB and import again".into(),
                );
            }
        }
        self.insert(key.clone(), entry)?;
        Ok(key)
    }
    pub fn start_import(&mut self, path: PathBuf) {
        if self.importing {
            return;
        }
        self.importing = true;
        if self.jobs.send((None, path)).is_err() {
            self.importing = false;
            self.errors.insert(
                "Import".into(),
                "Asset worker stopped; reopen the project".into(),
            );
        }
    }
    pub fn poll(&mut self, project: &Project, project_path: &str) -> Option<String> {
        self.remember(project, project_path);
        let mut message = None;
        while let Ok(done) = self.results.try_recv() {
            let imported = done.key.is_none();
            if imported {
                self.importing = false;
            } else {
                self.pending.remove(done.key.as_ref().unwrap());
            }
            let key = done.key.clone().unwrap_or_else(|| "Import".into());
            let result = done.result.and_then(|entry| {
                let ms = entry.model.import_ms;
                let warnings = entry.model.warnings.join("; ");
                let relative = if let Some(key) = done.key {
                    self.insert(key.clone(), entry)?;
                    key
                } else {
                    self.imported(&done.path, entry, project_path)?
                };
                Ok((relative, ms, warnings))
            });
            match result {
                Ok((key, ms, warnings)) => {
                    if imported {
                        self.chosen = key.clone();
                        self.errors.remove("Import");
                        message = Some(format!(
                            "Imported {key} in {ms:.3} ms. Select a component and assign. {warnings}"
                        ));
                    }
                }
                Err(e) => {
                    self.errors.insert(key, e.clone());
                    message = Some(format!(
                        "GLB load failed: {e}. Assignment retained; development references shown."
                    ));
                }
            }
        }
        for a in project
            .trains
            .iter()
            .flat_map(|t| &t.components)
            .filter_map(|c| c.asset.as_ref())
        {
            if self.library.contains_key(&a.path)
                || self.errors.contains_key(&a.path)
                || self.pending.contains(&a.path)
            {
                continue;
            }
            if let Some(source) = root(project_path)
                .map(|base| base.join(&a.path))
                .or_else(|| self.sources.get(&a.path).cloned())
            {
                self.pending.insert(a.path.clone());
                if self.jobs.send((Some(a.path.clone()), source)).is_err() {
                    self.pending.remove(&a.path);
                    self.errors
                        .insert(a.path.clone(), "Asset worker stopped".into());
                }
            } else {
                self.errors.insert(
                    a.path.clone(),
                    "Save the project or replace this unresolved project-relative asset".into(),
                );
            }
        }
        message
    }
    pub fn busy(&self) -> bool {
        self.importing || !self.pending.is_empty()
    }
    pub fn load_blocking(&mut self, project: &Project, project_path: &str) {
        for a in project
            .trains
            .iter()
            .flat_map(|t| &t.components)
            .filter_map(|c| c.asset.as_ref())
        {
            if self.library.contains_key(&a.path) {
                continue;
            }
            let result = root(project_path)
                .ok_or_else(|| "Project has no asset directory".to_string())
                .and_then(|base| load(&base.join(&a.path)))
                .and_then(|entry| self.insert(a.path.clone(), entry));
            if let Err(e) = result {
                self.errors.insert(a.path.clone(), e);
            }
        }
    }
    pub fn import_now(&mut self, path: &Path, project_path: &str) -> Result<String, String> {
        let entry = load(path)?;
        self.imported(path, entry, project_path)
    }
    /// Publish assets before the atomic project write. Never overwrite different
    /// bytes. Missing references remain intact and are returned as explicit warnings.
    pub fn package(
        &self,
        project: &Project,
        old_path: &str,
        destination: &Path,
    ) -> Result<Vec<String>, String> {
        let base = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let canonical = base
            .canonicalize()
            .map_err(|e| format!("Project directory {} is unavailable: {e}", base.display()))?;
        let mut paths: HashSet<_> = self.library.keys().cloned().collect();
        paths.extend(self.sources.keys().cloned());
        paths.extend(
            project
                .trains
                .iter()
                .flat_map(|t| &t.components)
                .filter_map(|c| c.asset.as_ref().map(|a| a.path.clone())),
        );
        let mut warnings = vec![];
        for path in paths {
            let data = if let Some(entry) = self.library.get(&path) {
                Some(entry.bytes.as_ref().clone())
            } else {
                root(old_path)
                    .and_then(|r| asset::read_glb(&r.join(&path)).ok())
                    .or_else(|| {
                        self.sources
                            .get(&path)
                            .and_then(|source| asset::read_glb(source).ok())
                    })
            };
            let Some(data) = data else {
                warnings.push(format!("Missing asset preserved: {path}"));
                continue;
            };
            let target = base.join(&path);
            let parent = target.parent().ok_or("Asset path has no directory")?;
            // Check every existing ancestor before creating directories, so a
            // symlink cannot redirect packaging outside the chosen project root.
            let mut ancestor = parent;
            while !ancestor.exists() {
                ancestor = ancestor.parent().ok_or("Invalid asset directory")?;
            }
            if !ancestor
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&canonical)
            {
                return Err("Asset directory escapes the project through a symlink".into());
            }
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            if !parent
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&canonical)
            {
                return Err("Asset directory is outside project root".into());
            }
            if target.exists() {
                if asset::read_glb(&target)? != data {
                    return Err(format!(
                        "Refusing to replace different asset bytes at {}; import the replacement separately",
                        target.display()
                    ));
                }
                continue;
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            let written = file.write_all(&data).and_then(|_| file.sync_all());
            drop(file);
            if let Err(error) = written {
                let _ = std::fs::remove_file(&target);
                return Err(format!("Asset copy failed; project retained: {error}"));
            }
        }
        Ok(warnings)
    }
}
/// Compose in f64: component * T * R * S * T(-pivot). The physics pose is read only.
pub(super) fn transform(position: Vec3, rotation: Quaternion, asset: &Asset) -> [[f64; 4]; 4] {
    let world = Frame::new(rotation).expect("validated inspection quaternion");
    let local = Frame::new(asset.rotation).expect("validated asset quaternion");
    let x = world.local_to_world(local.local_to_world(Vec3::X * asset.scale.x));
    let y = world.local_to_world(local.local_to_world(Vec3::Y * asset.scale.y));
    let z = world.local_to_world(local.local_to_world(Vec3::Z * asset.scale.z));
    let t = position + world.local_to_world(asset.translation_m)
        - x * asset.pivot_m.x
        - y * asset.pivot_m.y
        - z * asset.pivot_m.z;
    [
        [x.x, x.y, x.z, 0.],
        [y.x, y.y, y.z, 0.],
        [z.x, z.y, z.z, 0.],
        [t.x, t.y, t.z, 1.],
    ]
}
pub(super) fn renderable(a: &Asset) -> bool {
    [a.scale.x, a.scale.y, a.scale.z]
        .iter()
        .all(|s| (1e-6..=1e6).contains(s))
        && [
            a.translation_m.x,
            a.translation_m.y,
            a.translation_m.z,
            a.pivot_m.x,
            a.pivot_m.y,
            a.pivot_m.z,
        ]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1e9)
}
impl Session {
    pub fn assign_train_asset(
        &mut self,
        train_id: &str,
        component_id: &str,
        asset: Option<Asset>,
    ) -> bool {
        if let Some(a) = &asset {
            if let Err(e) = a.validate() {
                self.status = e.to_string();
                return false;
            }
            if !renderable(a) {
                self.status="Visual correction exceeds renderer limits: scale must be 0.000001..1000000; pivot/translation within ±1e9 m".into();
                return false;
            }
            if !self.assets.library.contains_key(&a.path) {
                self.status = "Import this GLB successfully before assigning it".into();
                return false;
            }
        }
        let mut trains = self.editor.project().trains.clone();
        let Some(c) = trains
            .iter_mut()
            .find(|t| t.id == train_id)
            .and_then(|t| t.components.iter_mut().find(|c| c.id == component_id))
        else {
            self.status = "Selected train component no longer exists".into();
            return false;
        };
        c.asset = asset;
        self.execute(vec![qx_document::command::Command::SetTrains(trains)])
    }
}
pub(super) fn instance(
    model: Arc<Model>,
    component_id: String,
    position: Vec3,
    rotation: Quaternion,
    asset: &Asset,
) -> AssetInstance {
    AssetInstance {
        model,
        component_id,
        transform: transform(position, rotation, asset),
    }
}
