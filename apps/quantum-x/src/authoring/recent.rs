//! Recent paths are application state, separate from projects and layout v1.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc,
};

const MAX_BYTES: u64 = 64 * 1024;
const MAX_ENTRIES: usize = 24;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub path: PathBuf,
    pub name: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    projects: Vec<Entry>,
}

#[derive(Default)]
pub(super) struct Recent {
    pub entries: Vec<Entry>,
    pub error: Option<String>,
    writer: Option<mpsc::Sender<Vec<u8>>>,
    results: Option<mpsc::Receiver<Result<(), String>>>,
    pending: usize,
}
impl Recent {
    pub fn load() -> Self {
        let path = std::env::var_os("QX_RECENT_PROJECTS")
            .map(PathBuf::from)
            .or_else(|| {
                // Isolated workbench runs must not touch the owner's normal recents.
                std::env::var_os("QX_WORKBENCH_PREFS")
                    .map(|p| PathBuf::from(p).with_extension("recent-v1.json"))
            })
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join("QUANTUM-X/recent-v1.json"))
            });
        Self::load_path(path)
    }
    pub fn load_path(path: Option<PathBuf>) -> Self {
        let mut store = Self::default();
        let Some(path) = path else {
            return store;
        };
        let loaded = (|| -> Result<Vec<Entry>, String> {
            let file = match fs::File::open(&path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
                Err(e) => return Err(e.to_string()),
            };
            let mut bytes = Vec::new();
            file.take(MAX_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_BYTES {
                return Err("recent-project file exceeds 64 KiB".into());
            }
            let record: Record = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if record.version != 1
                || record.projects.len() > MAX_ENTRIES
                || record
                    .projects
                    .iter()
                    .any(|e| e.name.len() > 1024 || !e.path.is_absolute())
            {
                return Err("invalid or unsupported recent-project record".into());
            }
            Ok(record.projects)
        })();
        match loaded {
            Ok(entries) => store.entries = entries,
            Err(error) => {
                store.error = Some(format!(
                    "Recent projects: {error}. Original file retained; history is session-only."
                ));
                return store;
            }
        }
        let (send, receive) = mpsc::channel::<Vec<u8>>();
        let (results, output) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(bytes) = receive.recv() {
                if results
                    .send(super::workbench::write_preferences(&path, &bytes))
                    .is_err()
                {
                    break;
                }
            }
        });
        store.writer = Some(send);
        store.results = Some(output);
        store
    }
    pub fn remember(&mut self, path: &Path, name: &str) {
        let path = fs::canonicalize(path).unwrap_or_else(|_| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            }
        });
        self.entries.retain(|e| e.path != path);
        self.entries.insert(
            0,
            Entry {
                path,
                name: name.chars().take(256).collect(),
            },
        );
        self.entries.truncate(MAX_ENTRIES);
        self.save();
    }
    pub fn remove(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
            self.save();
        }
    }
    fn save(&mut self) {
        let Some(writer) = &self.writer else {
            return;
        };
        let record = Record {
            version: 1,
            projects: self.entries.clone(),
        };
        match serde_json::to_vec_pretty(&record) {
            Ok(bytes) if bytes.len() as u64 <= MAX_BYTES => {
                if writer.send(bytes).is_ok() {
                    self.pending += 1;
                } else {
                    self.error =
                        Some("Recent-project writer stopped; history is session-only.".into());
                }
            }
            _ => {
                self.error =
                    Some("Recent-project history exceeds 64 KiB; changes were not saved.".into())
            }
        }
    }
    pub fn poll(&mut self, wait: bool) {
        while self.pending > 0 {
            let result = if wait {
                self.results.as_ref().unwrap().recv().ok()
            } else {
                self.results.as_ref().unwrap().try_recv().ok()
            };
            let Some(result) = result else {
                break;
            };
            self.pending -= 1;
            if let Err(error) = result {
                self.error = Some(format!("Cannot save recent projects: {error}"));
            }
        }
    }
}
