//! Data-only visual catalog. It never depends on the geometry solver.
use serde::{Deserialize, Serialize};
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
    Concept,
    Experimental,
    Released,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub id: String,
    pub name: String,
    pub description: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Style {
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleSelection {
    pub family_id: String,
    pub style_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u32,
    pub families: Vec<Family>,
    pub train_ids: Vec<String>,
    pub styles: Vec<Style>,
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn relative_asset(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 2048
        && !s.starts_with('/')
        && !s.contains('\\')
        && !s.contains(':')
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
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
        require(self.version == 1, "unsupported catalog version")?;
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
        let mut ids = HashSet::new();
        let mut numbers = HashSet::new();
        for s in &self.styles {
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
            if s.status == DevelopmentStatus::Released {
                require(
                    s.track_asset.is_some() && s.thumbnail.is_some() && s.preview.is_some(),
                    "released styles require track asset, thumbnail, and preview references",
                )?;
            }
        }
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
        assert_eq!(c.styles.len(), 3);
        assert_eq!(c.styles_for_family("monorail").count(), 2);
        assert!(
            c.styles
                .iter()
                .all(|s| s.status == DevelopmentStatus::Concept
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
        c.styles[0].status = DevelopmentStatus::Released;
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.styles[0].preview = Some("../secret".into());
        assert!(c.validate().is_err());
        let mut c = base.clone();
        c.styles[0].compatible_train_ids.push("missing".into());
        assert!(c.validate().is_err());
        let mut c = base;
        c.version = 2;
        assert!(c.validate().is_err());
    }
}
