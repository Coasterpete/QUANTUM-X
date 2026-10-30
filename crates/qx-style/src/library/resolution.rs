use super::{ContentManifest, LibraryKind};
use crate::{Catalog, CatalogError, DevelopmentStatus, Result, relative_asset, require};
use serde::Serialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

/// Canonical containment also rejects junction/symlink escapes on Windows.
pub fn resolve_existing(root: &Path, relative: &str) -> Result<PathBuf> {
    require(relative_asset(relative), "invalid relative library path")?;
    let base = root
        .canonicalize()
        .map_err(|e| CatalogError(format!("library root {}: {e}", root.display())))?;
    let path = base
        .join(relative)
        .canonicalize()
        .map_err(|e| CatalogError(format!("{relative}: {e}")))?;
    require(
        path.starts_with(&base),
        "library path escapes its root through a link",
    )?;
    Ok(path)
}
pub fn read_limited(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|e| CatalogError(format!("{}: {e}", path.display())))?;
    require(bytes.len() <= 4 * 1024 * 1024, "library JSON exceeds 4 MiB")?;
    String::from_utf8(bytes).map_err(|e| CatalogError(e.to_string()))
}

#[derive(Clone, Debug, Serialize)]
pub struct EntryState {
    pub kind: LibraryKind,
    pub style_id: String,
    pub status: DevelopmentStatus,
    /// Content resolves; this is not physical validation or asset certification.
    pub content_ready: bool,
    pub issues: Vec<String>,
    #[serde(skip)]
    pub manifest: Option<ContentManifest>,
    #[serde(skip)]
    pub directory: Option<PathBuf>,
}
#[derive(Clone, Debug)]
pub struct Library {
    pub catalog: Catalog,
    pub root: PathBuf,
    /// Sorted by kind and stable ID, independent of directory enumeration order.
    pub entries: Vec<EntryState>,
}
impl Library {
    pub fn open(assets_root: &Path) -> Result<Self> {
        let index = resolve_existing(assets_root, "catalog/styles.json")?;
        Self::from_catalog(Catalog::from_json(&read_limited(&index)?)?, assets_root)
    }
    pub fn from_catalog(catalog: Catalog, assets_root: &Path) -> Result<Self> {
        catalog.validate()?;
        let mut entries: Vec<_> = catalog
            .styles
            .iter()
            .map(|s| {
                inspect(
                    assets_root,
                    LibraryKind::Track,
                    &s.id,
                    s.directory.as_deref(),
                    &s.status,
                )
            })
            .chain(catalog.train_styles.iter().map(|s| {
                inspect(
                    assets_root,
                    LibraryKind::Train,
                    &s.id,
                    Some(&s.directory),
                    &s.status,
                )
            }))
            .collect();
        entries.sort_by(|a, b| (a.kind, &a.style_id).cmp(&(b.kind, &b.style_id)));
        Ok(Self {
            catalog,
            root: assets_root.to_path_buf(),
            entries,
        })
    }
    pub fn entry(&self, kind: LibraryKind, id: &str) -> Option<&EntryState> {
        self.entries
            .iter()
            .find(|e| e.kind == kind && e.style_id == id)
    }
    /// Resolve a specific visual/preview/definition reference from a validated manifest.
    pub fn asset(&self, kind: LibraryKind, id: &str, relative: &str) -> Result<PathBuf> {
        let entry = self
            .entry(kind, id)
            .ok_or_else(|| CatalogError("unknown library style".into()))?;
        require(
            entry
                .manifest
                .as_ref()
                .is_some_and(|m| m.paths().any(|p| p == relative)),
            "asset is not declared by the style manifest",
        )?;
        resolve_existing(
            entry
                .directory
                .as_deref()
                .ok_or_else(|| CatalogError("style directory unavailable".into()))?,
            relative,
        )
    }
}

fn inspect(
    root: &Path,
    kind: LibraryKind,
    id: &str,
    directory: Option<&str>,
    status: &DevelopmentStatus,
) -> EntryState {
    let mut entry = EntryState {
        kind,
        style_id: id.into(),
        status: status.clone(),
        content_ready: false,
        issues: vec![],
        manifest: None,
        directory: None,
    };
    let load = || -> Result<(PathBuf, ContentManifest)> {
        let directory = directory.ok_or_else(|| {
            CatalogError("legacy metadata only; no content directory registered".into())
        })?;
        let path = resolve_existing(root, directory)?;
        require(path.is_dir(), "style directory is not a directory")?;
        let manifest = resolve_existing(&path, "style.json")?;
        let content = ContentManifest::from_json(&read_limited(&manifest)?)?;
        require(
            content.kind == kind && content.style_id == id,
            "manifest kind/style ID does not match catalog",
        )?;
        Ok((path, content))
    };
    match load() {
        Err(e) => entry.issues.push(e.to_string()),
        Ok((directory, manifest)) => {
            for path in manifest.paths() {
                match resolve_existing(&directory, path) {
                    Ok(p) if p.is_file() => {}
                    Ok(_) => entry.issues.push(format!("{path}: expected a file")),
                    Err(e) => entry.issues.push(e.to_string()),
                }
            }
            if manifest.previews.thumbnail.is_none() || manifest.previews.large.is_none() {
                entry
                    .issues
                    .push("representative thumbnail and large preview pending".into());
            }
            if kind == LibraryKind::Track && manifest.construction.is_none() {
                entry
                    .issues
                    .push("supported procedural construction definition pending".into());
            }
            if kind == LibraryKind::Train && manifest.mechanical_definition.is_none() {
                entry.issues.push("mechanical definition pending".into());
            }
            entry.directory = Some(directory);
            entry.manifest = Some(manifest);
        }
    }
    entry.content_ready = *status == DevelopmentStatus::Available && entry.issues.is_empty();
    if *status != DevelopmentStatus::Available {
        entry.issues.insert(
            0,
            format!("{}; unavailable for application", status.label()),
        );
    }
    entry
}
