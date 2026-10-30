//! Track/train catalog and content discovery, independent of UI, GPU and physics.
use serde::{Deserialize, Serialize};
pub mod compatibility;
pub mod construction;
pub mod library;
use std::{collections::HashSet, fmt};

#[derive(Debug)]
pub struct CatalogError(pub String);
impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CatalogError {}
type Result<T> = std::result::Result<T, CatalogError>;
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(CatalogError(message.into()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentStatus {
    #[serde(alias = "concept")]
    Planned,
    #[serde(alias = "experimental")]
    InDevelopment,
    #[serde(alias = "released")]
    Available,
}
impl DevelopmentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Planned => "Planned",
            Self::InDevelopment => "In Development",
            Self::Available => "Available",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Relative to the library's assets root; absent in legacy catalogs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vehicle_features: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<construction::TrackConstruction>,
    pub id: String,
    pub family_id: String,
    pub number: u32,
    pub name: String,
    pub description: String,
    pub compatible_train_ids: Vec<String>,
    pub track_asset: Option<String>,
    pub thumbnail: Option<String>,
    pub preview: Option<String>,
    pub reference_images: Vec<String>,
    pub status: DevelopmentStatus,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleSelection {
    pub family_id: String,
    pub style_id: String,
}
impl StyleSelection {
    /// Both IDs empty means deliberately unassigned. A partial pair is invalid.
    /// Retains the existing JSON object shape without inventing a catalog style.
    pub fn is_unassigned(&self) -> bool {
        self.family_id.is_empty() && self.style_id.is_empty()
    }
    pub fn validate(&self, catalog: &Catalog) -> Result<()> {
        if self.is_unassigned() {
            Ok(())
        } else {
            catalog.validate_selection(self).map(|_| ())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub families: Vec<Family>,
    pub train_ids: Vec<String>,
    pub styles: Vec<Style>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub train_families: Vec<Family>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub train_styles: Vec<library::TrainStyle>,
}
pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.as_bytes()[s.len() - 1].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn relative_asset(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 2048
        && !s.starts_with('/')
        && !s.contains('\\')
        && !s.contains(':')
        && !s.chars().any(char::is_control)
        && !s.contains(['*', '?', '"', '<', '>', '|'])
        && s.split('/').all(|p| {
            let stem = p.split('.').next().unwrap_or("").to_ascii_uppercase();
            !p.is_empty()
                && p != "."
                && p != ".."
                && !p.ends_with(['.', ' '])
                && !matches!(
                    stem.as_str(),
                    "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
                )
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
}
impl Catalog {
    pub fn from_json(json: &str) -> Result<Self> {
        require(json.len() <= 4 * 1024 * 1024, "catalog exceeds 4 MiB")?;
        let catalog: Self = serde_json::from_str(json).map_err(|e| CatalogError(e.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn built_in() -> Result<Self> {
        Self::from_json(include_str!("../../../assets/catalog/styles.json"))
    }
    pub fn validate(&self) -> Result<()> {
        require(matches!(self.version, 1 | 2), "unsupported catalog version")?;
        require(
            !self.families.is_empty()
                && self.families.len() <= 1024
                && self.styles.len() <= 8192
                && self.train_ids.len() <= 8192,
            "invalid catalog counts",
        )?;
        let mut families = HashSet::new();
        for f in &self.families {
            require(
                identifier(&f.id) && families.insert(&f.id),
                "invalid or duplicate family ID",
            )?;
            library::validate_directory(f.directory.as_deref(), "tracks")?;
            require(
                !f.name.trim().is_empty() && f.name.len() <= 1024 && f.description.len() <= 8192,
                "invalid family text",
            )?;
        }
        let mut trains = HashSet::new();
        for t in &self.train_ids {
            require(
                identifier(t) && trains.insert(t),
                "invalid or duplicate train ID",
            )?;
        }
        library::validate_trains(self, &mut trains)?;
        let mut ids = HashSet::new();
        let mut numbers = HashSet::new();
        for s in &self.styles {
            library::validate_directory(s.directory.as_deref(), "tracks")?;
            if let Some(directory) = &s.directory {
                let family = self.families.iter().find(|f| f.id == s.family_id);
                require(
                    family
                        .and_then(|f| f.directory.as_ref())
                        .is_some_and(|f| directory.starts_with(&format!("{f}/"))),
                    "track directory must be within its family directory",
                )?;
            }
            compatibility::validate_tags(&s.vehicle_features).map_err(CatalogError)?;
            if let Some(geometry) = &s.geometry {
                geometry.validate()?;
            }
            require(
                identifier(&s.id) && ids.insert(&s.id),
                "invalid or duplicate style ID",
            )?;
            require(
                families.contains(&s.family_id)
                    && s.number > 0
                    && numbers.insert((&s.family_id, s.number)),
                "unknown family or duplicate style number",
            )?;
            require(
                !s.name.trim().is_empty()
                    && s.name.len() <= 1024
                    && !s.description.trim().is_empty()
                    && s.description.len() <= 8192,
                "invalid style text",
            )?;
            let mut compatible = HashSet::new();
            require(
                s.compatible_train_ids
                    .iter()
                    .all(|t| trains.contains(t) && compatible.insert(t)),
                "unknown or duplicate compatible train ID",
            )?;
            require(s.reference_images.len() <= 64, "too many reference images")?;
            for p in s
                .track_asset
                .iter()
                .chain(s.thumbnail.iter())
                .chain(s.preview.iter())
                .chain(s.reference_images.iter())
            {
                require(
                    relative_asset(p),
                    "asset references must be relative paths without traversal",
                )?;
            }
            if s.status == DevelopmentStatus::Available && s.directory.is_none() {
                require(
                    s.track_asset.is_some() && s.thumbnail.is_some() && s.preview.is_some(),
                    "released styles require track asset, thumbnail, and preview references",
                )?;
            }
        }
        library::validate_links(self)?;
        Ok(())
    }
    pub fn validate_selection(&self, selection: &StyleSelection) -> Result<&Style> {
        self.styles
            .iter()
            .find(|s| s.id == selection.style_id && s.family_id == selection.family_id)
            .ok_or_else(|| CatalogError("unknown style or style/family mismatch".into()))
    }
    pub fn styles_for_family<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Style> {
        self.styles.iter().filter(move |s| s.family_id == id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concepts_do_not_promise_assets() {
        let c = Catalog::built_in().unwrap();
        assert_eq!(c.styles.len(), 17);
        assert_eq!(c.styles_for_family("monorail").count(), 2);
        assert!(
            c.styles
                .iter()
                .all(|s| s.status == DevelopmentStatus::Planned
                    && s.preview.is_none()
                    && s.track_asset.is_none())
        );
    }
    #[test]
    fn rejects_catalog_inconsistencies() {
        let base = Catalog::built_in().unwrap();
        let mut c = base.clone();
        c.styles.push(c.styles[0].clone());
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.styles[0].status = DevelopmentStatus::Available;
        c.styles[0].directory = None;
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.styles[0].preview = Some("../secret".into());
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.styles[0].compatible_train_ids.push("missing".into());
        assert!(c.validate().is_err());
        let mut c = base;
        c.version = 99;
        assert!(c.validate().is_err());
    }
}
