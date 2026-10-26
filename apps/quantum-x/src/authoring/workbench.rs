//! Presentation preferences only: no document, history, solver or support objects.
use egui_tiles::{Container, Linear, LinearDir, Tile, TileId, Tiles, Tree};
use qx_rendering::WindowPreferences;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum Profile {
    #[default]
    Track,
    Train,
}
impl Profile {
    pub const fn index(self) -> usize {
        match self {
            Self::Track => 0,
            Self::Train => 1,
        }
    }
    pub fn panes(self) -> &'static [Pane] {
        match self {
            Self::Track => &[
                Pane::Track,
                Pane::Viewport,
                Pane::Support,
                Pane::Transition,
                Pane::View,
                Pane::Diagnostics,
            ],
            Self::Train => &[Pane::Configuration, Pane::Preview, Pane::Summary],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum Pane {
    Track,
    Viewport,
    Support,
    Transition,
    View,
    Diagnostics,
    Configuration,
    Preview,
    Summary,
}
impl Pane {
    pub fn title(self) -> &'static str {
        match self {
            Self::Track => "Track Workspace",
            Self::Viewport => "3D Viewport",
            Self::Support => "Support Workspace",
            Self::Transition => "Transition Editor",
            Self::View => "View",
            Self::Diagnostics => "Diagnostics",
            Self::Configuration => "Configuration",
            Self::Preview => "Preview",
            Self::Summary => "Summary",
        }
    }
    pub fn is_viewport(self) -> bool {
        matches!(self, Self::Viewport | Self::Preview)
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FloatingPane {
    pub pane: Pane,
    pub position: [f32; 2],
    pub size: [f32; 2],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Layout {
    pub tree: Tree<Pane>,
    pub floating: Vec<FloatingPane>,
}
impl Layout {
    pub fn new(profile: Profile) -> Self {
        let mut tiles = Tiles::default();
        let root = if profile == Profile::Track {
            let left = tab(&mut tiles, &[Pane::Track]);
            let center = tab(&mut tiles, &[Pane::Viewport]);
            let right = tab(&mut tiles, &[Pane::Support, Pane::View, Pane::Diagnostics]);
            let mut row = Linear::new(LinearDir::Horizontal, vec![left, center, right]);
            for (id, share) in [(left, 0.22), (center, 0.56), (right, 0.22)] {
                row.shares.set_share(id, share);
            }
            let row = tiles.insert_container(row);
            let bottom = tab(&mut tiles, &[Pane::Transition]);
            tiles.insert_container(Linear::new_binary(LinearDir::Vertical, [row, bottom], 0.60))
        } else {
            let left = tab(&mut tiles, &[Pane::Configuration]);
            let center = tab(&mut tiles, &[Pane::Preview]);
            let right = tab(&mut tiles, &[Pane::Summary]);
            let mut row = Linear::new(LinearDir::Horizontal, vec![left, center, right]);
            for (id, share) in [(left, 0.24), (center, 0.52), (right, 0.24)] {
                row.shares.set_share(id, share);
            }
            tiles.insert_container(row)
        };
        Self {
            tree: Tree::new(
                egui::Id::unique(("workbench", profile.index())),
                root,
                tiles,
            ),
            floating: vec![],
        }
    }
    pub fn pane_id(&self, pane: Pane) -> Option<TileId> {
        self.tree
            .tiles
            .iter()
            .find_map(|(id, tile)| (tile == &Tile::Pane(pane)).then_some(*id))
    }
    pub fn float(&mut self, pane: Pane) {
        if pane.is_viewport() || self.floating.iter().any(|f| f.pane == pane) {
            return;
        }
        if let Some(id) = self.pane_id(pane) {
            self.tree.set_visible(id, false);
            self.floating.push(FloatingPane {
                pane,
                position: [80., 100.],
                size: [560., 420.],
            });
        }
    }
    pub fn dock(&mut self, pane: Pane) {
        self.floating.retain(|f| f.pane != pane);
        if let Some(id) = self.pane_id(pane) {
            self.tree.set_visible(id, true);
        }
        self.tree.make_active(|_, tile| tile == &Tile::Pane(pane));
    }
    fn validate(&mut self, profile: Profile) -> Result<(), String> {
        fn walk(
            layout: &Layout,
            id: TileId,
            seen: &mut HashSet<TileId>,
            panes: &mut HashSet<Pane>,
            depth: usize,
        ) -> Result<(), String> {
            if depth > 24 || !seen.insert(id) {
                return Err("layout cycle, duplicate or excessive depth".into());
            }
            match layout.tree.tiles.get(id).ok_or("missing layout tile")? {
                Tile::Pane(pane) => {
                    if !panes.insert(*pane) {
                        return Err("duplicate pane".into());
                    }
                }
                Tile::Container(c) => {
                    if c.children().count() == 0 {
                        return Err("empty layout container".into());
                    }
                    if let Container::Linear(l) = c
                        && l.shares
                            .iter()
                            .any(|(_, s)| !s.is_finite() || *s <= 0. || *s > 1e6)
                    {
                        return Err("invalid panel proportions".into());
                    }
                    if let Container::Tabs(t) = c
                        && t.active.is_some_and(|a| !t.children.contains(&a))
                    {
                        return Err("invalid focused tab".into());
                    }
                    if matches!(c, Container::Grid(_)) {
                        return Err("unsupported layout container".into());
                    }
                    for child in c.children() {
                        walk(layout, *child, seen, panes, depth + 1)?;
                    }
                }
            }
            Ok(())
        }
        if self.tree.tiles.len() > 64 || self.tree.tiles.tile_ids().any(|id| id.0 >= (1 << 48)) {
            return Err("layout has too many tiles or an out-of-range tile ID".into());
        }
        let mut seen = HashSet::new();
        let mut panes = HashSet::new();
        walk(
            self,
            self.tree.root.ok_or("layout has no root")?,
            &mut seen,
            &mut panes,
            0,
        )?;
        if seen.len() != self.tree.tiles.len() || panes != profile.panes().iter().copied().collect()
        {
            return Err("layout pane set is invalid".into());
        }
        let mut floats = HashSet::new();
        for f in &self.floating {
            if f.pane.is_viewport()
                || !panes.contains(&f.pane)
                || !floats.insert(f.pane)
                || f.position.iter().chain(&f.size).any(|v| !v.is_finite())
            {
                return Err("invalid floating pane".into());
            }
        }
        for pane in profile.panes() {
            if let Some(id) = self.pane_id(*pane) {
                self.tree.set_visible(id, !floats.contains(pane));
            }
        }
        self.tree.tiles.recompute_next_tile_id();
        // Do not trust serialized UI IDs or stale cached window extents.
        self.tree = Tree::new(
            egui::Id::unique(("workbench", profile.index())),
            self.tree.root.unwrap(),
            std::mem::take(&mut self.tree.tiles),
        );
        Ok(())
    }
}
fn tab(tiles: &mut Tiles<Pane>, panes: &[Pane]) -> TileId {
    let children = panes.iter().map(|p| tiles.insert_pane(*p)).collect();
    tiles.insert_tab_tile(children)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Workbench {
    pub version: u32,
    pub active: Profile,
    pub layouts: [Layout; 2],
    pub window: WindowPreferences,
}
impl Default for Workbench {
    fn default() -> Self {
        Self {
            version: 1,
            active: Profile::Track,
            layouts: [Layout::new(Profile::Track), Layout::new(Profile::Train)],
            window: WindowPreferences::default(),
        }
    }
}
impl Workbench {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 256 * 1024 {
            return Err("workbench preferences exceed 256 KiB".into());
        }
        let mut value: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if value.version != 1 {
            return Err("unsupported workbench preferences version".into());
        }
        value.window.sanitize();
        for p in [Profile::Track, Profile::Train] {
            value.layouts[p.index()].validate(p)?;
        }
        Ok(value)
    }
    pub fn reset_layout(&mut self) {
        self.layouts[self.active.index()] = Layout::new(self.active);
    }
}

pub(super) struct PreferenceStore {
    path: Option<PathBuf>,
    previous: Vec<u8>,
    checked: Instant,
    pub error: Option<String>,
    writable: bool,
    writer: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
    write_results: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    in_flight: usize,
}
impl Default for PreferenceStore {
    fn default() -> Self {
        Self {
            path: None,
            previous: vec![],
            checked: Instant::now(),
            error: None,
            writable: true,
            writer: None,
            write_results: None,
            in_flight: 0,
        }
    }
}
impl PreferenceStore {
    pub fn load() -> (Workbench, Self) {
        let path = std::env::var_os("QX_WORKBENCH_PREFS")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(|p| PathBuf::from(p).join("QUANTUM-X/workbench-v1.json"))
            });
        Self::load_path(path)
    }
    fn load_path(path: Option<PathBuf>) -> (Workbench, Self) {
        let mut store = Self {
            path,
            ..Default::default()
        };
        let value = match store.path.as_ref().map(fs::metadata) {
            Some(Ok(metadata)) if metadata.len() > 256 * 1024 => {
                Err("workbench preferences exceed 256 KiB".into())
            }
            Some(Ok(_)) => (|| -> Result<Workbench, String> {
                let file =
                    fs::File::open(store.path.as_ref().unwrap()).map_err(|e| e.to_string())?;
                let mut bytes = vec![];
                file.take(256 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                Workbench::decode(&bytes)
            })(),
            Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(Workbench::default()),
        };
        match value {
            Ok(value) => (value, store),
            Err(e) => {
                store.error = Some(format!(
                    "Preferences: {e}. Defaults recovered; original file retained. Reset Window/Layout enables replacement."
                ));
                store.writable = false;
                (Workbench::default(), store)
            }
        }
    }
    pub fn allow_reset(&mut self) {
        self.writable = true;
        self.error = None;
        self.previous.clear();
    }
    fn result(&mut self, result: Result<(), String>) {
        self.in_flight = self.in_flight.saturating_sub(1);
        match result {
            Ok(()) => self.error = None,
            Err(e) => {
                self.previous.clear();
                self.error = Some(format!("Cannot save workbench preferences: {e}"));
            }
        }
    }
    pub fn save(&mut self, value: &Workbench, force: bool) {
        // Polling and serialization are bounded. File writes/fsync run on a worker;
        // only application shutdown waits for the final durable result.
        while let Some(result) = self.write_results.as_ref().and_then(|r| r.try_recv().ok()) {
            self.result(result);
        }
        if !self.writable
            || self.path.is_none()
            || (!force && (self.in_flight > 0 || self.checked.elapsed() < Duration::from_secs(1)))
        {
            return;
        }
        self.checked = Instant::now();
        let result = (|| -> Result<(), String> {
            let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
            if bytes.len() > 256 * 1024 {
                return Err("workbench preferences exceed 256 KiB".into());
            }
            // Hash map serialization ordering is immaterial to change detection.
            let unchanged = !self.previous.is_empty()
                && serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                    == serde_json::from_slice(&self.previous).ok();
            if !unchanged {
                if self.writer.is_none() {
                    let path = self.path.clone().unwrap();
                    let (send, receive) = std::sync::mpsc::sync_channel::<Vec<u8>>(1);
                    let (results, output) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        while let Ok(bytes) = receive.recv() {
                            if results.send(write_preferences(&path, &bytes)).is_err() {
                                break;
                            }
                        }
                    });
                    self.writer = Some(send);
                    self.write_results = Some(output);
                }
                if force {
                    self.writer
                        .as_ref()
                        .unwrap()
                        .send(bytes.clone())
                        .map_err(|e| e.to_string())?;
                } else {
                    self.writer
                        .as_ref()
                        .unwrap()
                        .try_send(bytes.clone())
                        .map_err(|e| e.to_string())?;
                }
                self.in_flight += 1;
                self.previous = bytes;
            }
            if force {
                while self.in_flight > 0 {
                    let result = self
                        .write_results
                        .as_ref()
                        .unwrap()
                        .recv()
                        .map_err(|e| e.to_string())?;
                    self.result(result);
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(format!("Cannot save workbench preferences: {e}"));
        }
    }
}
fn write_preferences(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = path.with_extension(format!("{}.{nonce}.tmp", std::process::id()));
    let mut created = false;
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        created = true;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(&temp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layouts_focused_tabs_floats_and_window_preferences_roundtrip() {
        let mut value = Workbench {
            active: Profile::Train,
            ..Default::default()
        };
        value.window.custom_size = [1536., 864.];
        value.window.position = Some([-1536, 100]);
        value.window.last_size = Some([1200., 760.]);
        value.window.maximized = true;
        value.window.ui_scale = 1.25;
        value.layouts[0]
            .tree
            .make_active(|_, t| t == &Tile::Pane(Pane::Diagnostics));
        value.layouts[0].float(Pane::Transition);
        value.layouts[1].float(Pane::Summary);
        let bytes = serde_json::to_vec(&value).unwrap();
        let mut reopened = Workbench::decode(&bytes).unwrap();
        assert_eq!(reopened.active, Profile::Train);
        assert_eq!(reopened.window, value.window);
        assert_eq!(reopened.layouts[0].floating[0].pane, Pane::Transition);
        let id = reopened.layouts[0].pane_id(Pane::Transition).unwrap();
        assert!(!reopened.layouts[0].tree.is_visible(id));
        reopened.layouts[0].dock(Pane::Transition);
        assert!(reopened.layouts[0].tree.is_visible(id));
        let train = serde_json::to_value(&reopened.layouts[1]).unwrap();
        reopened.active = Profile::Track;
        reopened.reset_layout();
        assert_eq!(serde_json::to_value(&reopened.layouts[1]).unwrap(), train);
    }
    #[test]
    fn corrupt_cycles_duplicates_and_unsupported_versions_recover_without_execution() {
        let mut value = Workbench::default();
        let root = value.layouts[0].tree.root.unwrap();
        if let Some(Tile::Container(Container::Linear(l))) =
            value.layouts[0].tree.tiles.get_mut(root)
        {
            l.children.push(root);
        }
        assert!(Workbench::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        let mut value = Workbench::default();
        let root = value.layouts[0].tree.root.unwrap();
        let tile = value.layouts[0].tree.tiles.remove(root).unwrap();
        value.layouts[0].tree.tiles.insert(TileId(u64::MAX), tile);
        value.layouts[0].tree.root = Some(TileId(u64::MAX));
        assert!(Workbench::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        let mut value = Workbench::default();
        value.layouts[0].floating.push(FloatingPane {
            pane: Pane::Viewport,
            position: [0., 0.],
            size: [300., 300.],
        });
        assert!(Workbench::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        let value = Workbench {
            version: 100,
            ..Default::default()
        };
        assert!(Workbench::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn preference_file_saves_replaces_and_retains_invalid_original_until_reset() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "qx-workbench-test-{}-{nonce}.json",
            std::process::id()
        ));
        let (mut value, mut store) = PreferenceStore::load_path(Some(path.clone()));
        store.save(&value, true);
        assert!(store.error.is_none());
        value.active = Profile::Train;
        value.layouts[1].float(Pane::Summary);
        store.save(&value, true);
        let (reopened, store) = PreferenceStore::load_path(Some(path.clone()));
        assert!(store.error.is_none());
        assert_eq!(reopened.active, Profile::Train);
        assert_eq!(reopened.layouts[1].floating.len(), 1);
        fs::write(&path, b"invalid original preferences").unwrap();
        let (value, mut store) = PreferenceStore::load_path(Some(path.clone()));
        assert!(store.error.is_some());
        store.save(&value, true);
        assert_eq!(fs::read(&path).unwrap(), b"invalid original preferences");
        store.allow_reset();
        store.save(&value, true);
        assert!(Workbench::decode(&fs::read(&path).unwrap()).is_ok());
        fs::remove_file(path).unwrap();
    }
}
