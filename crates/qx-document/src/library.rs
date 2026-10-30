//! Library-to-document adapter. Directory names never construct vehicle mechanics.
//! Visual sources stay outside saved documents until the application imports and
//! packages them through its existing GLB asset service.
use crate::{DocumentError, Result, train::Train};
use qx_style::library::{Library, LibraryKind, read_limited};
use std::{collections::BTreeMap, path::PathBuf};

pub struct ResolvedTrain {
    pub definition: Train,
    /// Manifest-relative source names to canonical GLB files; no GPU state.
    pub visual_sources: BTreeMap<String, PathBuf>,
}
pub fn load_train_definition(library: &Library, style_id: &str) -> Result<ResolvedTrain> {
    let entry = library
        .entry(LibraryKind::Train, style_id)
        .ok_or_else(|| DocumentError("unknown train library style".into()))?;
    crate::require(
        entry.content_ready,
        "train content is unavailable; inspect catalog diagnostics",
    )?;
    let manifest = entry
        .manifest
        .as_ref()
        .ok_or_else(|| DocumentError("missing train manifest".into()))?;
    let reference = manifest
        .mechanical_definition
        .as_deref()
        .ok_or_else(|| DocumentError("missing train mechanical definition".into()))?;
    let path = library.asset(LibraryKind::Train, style_id, reference)?;
    let mut definition: Train = serde_json::from_str(&read_limited(&path)?)?;
    definition.validate()?;
    let style = library
        .catalog
        .train_style(style_id)
        .ok_or_else(|| DocumentError("unknown train style".into()))?;
    let mut declaration = style.compatibility.clone();
    declaration.train_style_id = Some(style.id.clone());
    crate::require(
        definition.compatibility == Default::default() || definition.compatibility == declaration,
        "mechanical definition conflicts with catalog compatibility declaration",
    )?;
    definition.compatibility = declaration;
    let mut visual_sources = BTreeMap::new();
    for component in &definition.components {
        if let Some(asset) = &component.asset {
            crate::require(
                manifest.visuals.iter().any(|v| v.path == asset.path),
                "component visual is not declared in the style manifest",
            )?;
            visual_sources.insert(
                asset.path.clone(),
                library.asset(LibraryKind::Train, style_id, &asset.path)?,
            );
        }
    }
    Ok(ResolvedTrain {
        definition,
        visual_sources,
    })
}
