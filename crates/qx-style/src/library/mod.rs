//! Indexed discovery only: directories never install styles implicitly.
//! All stored paths are relative to an assets root; content paths are relative
//! to the style directory. No manufacturer or mechanical facts are inferred.
mod manifest;
mod resolution;
use crate::{Catalog, DevelopmentStatus, Family, Result, identifier, require};
pub use manifest::{ContentManifest, LibraryKind, PreviewImages, VisualAsset};
pub use resolution::{EntryState, Library, read_limited, resolve_existing};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainStyle {
    pub id: String,
    pub family_id: String,
    /// Numbered and named styles are distinct; no number is invented for a family.
    pub number: Option<u32>,
    pub name: String,
    pub description: String,
    pub directory: String,
    pub status: DevelopmentStatus,
    #[serde(default)]
    pub compatibility: crate::compatibility::VehicleCompatibility,
}

pub(super) fn validate_directory(directory: Option<&str>, kind: &str) -> Result<()> {
    if let Some(path) = directory {
        require(
            crate::relative_asset(path) && path.starts_with(&format!("{kind}/")),
            "directory must be a relative path under tracks/ or trains/ as appropriate",
        )?;
    }
    Ok(())
}

pub(super) fn validate_trains<'a>(
    catalog: &'a Catalog,
    ids: &mut HashSet<&'a String>,
) -> Result<()> {
    require(
        catalog.train_families.len() <= 1024 && catalog.train_styles.len() <= 8192,
        "too many train catalog records",
    )?;
    let mut families = HashSet::new();
    for Family {
        id,
        name,
        description,
        directory,
    } in &catalog.train_families
    {
        require(
            identifier(id) && families.insert(id),
            "invalid or duplicate train family ID",
        )?;
        require(
            !name.trim().is_empty() && name.len() <= 1024 && description.len() <= 8192,
            "invalid train family text",
        )?;
        validate_directory(directory.as_deref(), "trains")?;
    }
    let mut numbers = HashSet::new();
    for style in &catalog.train_styles {
        require(
            identifier(&style.id) && ids.insert(&style.id),
            "invalid or duplicate train style ID",
        )?;
        require(families.contains(&style.family_id), "unknown train family")?;
        require(
            !style.name.trim().is_empty()
                && style.name.len() <= 1024
                && !style.description.trim().is_empty()
                && style.description.len() <= 8192,
            "invalid train style text",
        )?;
        require(
            style
                .number
                .is_none_or(|n| n > 0 && numbers.insert((&style.family_id, n))),
            "invalid or duplicate train style number",
        )?;
        validate_directory(Some(&style.directory), "trains")?;
        require(
            catalog
                .train_families
                .iter()
                .find(|f| f.id == style.family_id)
                .and_then(|f| f.directory.as_ref())
                .is_some_and(|f| style.directory.starts_with(&format!("{f}/"))),
            "train directory must be within its family directory",
        )?;
        style
            .compatibility
            .validate()
            .map_err(crate::CatalogError)?;
        require(
            style
                .compatibility
                .train_style_id
                .as_ref()
                .is_none_or(|id| id == &style.id),
            "train compatibility identity mismatch",
        )?;
    }
    Ok(())
}

pub(super) fn validate_links(catalog: &Catalog) -> Result<()> {
    let mut directories = HashSet::new();
    for dir in catalog
        .styles
        .iter()
        .filter_map(|s| s.directory.as_ref())
        .chain(catalog.train_styles.iter().map(|s| &s.directory))
    {
        require(
            directories.insert(dir.to_ascii_lowercase()),
            "duplicate library style directory",
        )?;
    }
    for train in &catalog.train_styles {
        for track in &train.compatibility.track_style_ids {
            require(
                catalog.styles.iter().any(|s| &s.id == track),
                "train refers to unknown track style",
            )?;
        }
    }
    Ok(())
}

impl Catalog {
    pub fn train_style(&self, id: &str) -> Option<&TrainStyle> {
        self.train_styles.iter().find(|s| s.id == id)
    }

    /// An explicit restriction on either side wins. Empty declarations are unknown.
    pub fn compatibility(
        &self,
        track_id: &str,
        train_id: &str,
    ) -> Result<crate::compatibility::Compatibility> {
        let track = self
            .styles
            .iter()
            .find(|s| s.id == track_id)
            .ok_or_else(|| crate::CatalogError("unknown track style".into()))?;
        let train = self
            .train_style(train_id)
            .ok_or_else(|| crate::CatalogError("unknown train style".into()))?;
        let mut declaration = train.compatibility.clone();
        declaration.train_style_id = Some(train.id.clone());
        Ok(declaration.assess(track))
    }
}

#[cfg(test)]
mod tests;
