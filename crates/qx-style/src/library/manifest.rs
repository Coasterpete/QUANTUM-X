use crate::{Result, construction::TrackConstruction, identifier, relative_asset, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    Track,
    Train,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualAsset {
    pub id: String,
    /// GLB reference only. The rendering adapter validates model bytes on import.
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewImages {
    pub thumbnail: Option<String>,
    pub large: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
}

/// `style.json` within the explicitly indexed directory. The index owns names,
/// IDs, status and compatibility; this manifest describes actual content only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifest {
    pub version: u32,
    pub kind: LibraryKind,
    pub style_id: String,
    #[serde(default)]
    pub visuals: Vec<VisualAsset>,
    #[serde(default)]
    pub previews: PreviewImages,
    #[serde(default)]
    pub construction: Option<TrackConstruction>,
    /// A standalone qx-document Train JSON, never derived from the directory.
    #[serde(default)]
    pub mechanical_definition: Option<String>,
}
impl ContentManifest {
    pub fn from_json(json: &str) -> Result<Self> {
        require(
            json.len() <= 4 * 1024 * 1024,
            "style manifest exceeds 4 MiB",
        )?;
        let manifest: Self =
            serde_json::from_str(json).map_err(|e| crate::CatalogError(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<()> {
        require(
            self.version == 1 && identifier(&self.style_id),
            "unsupported manifest version or invalid style ID",
        )?;
        require(
            self.visuals.len() <= 1024 && self.previews.references.len() <= 64,
            "too many visual/preview references",
        )?;
        let mut ids = std::collections::HashSet::new();
        for visual in &self.visuals {
            require(
                identifier(&visual.id)
                    && ids.insert(&visual.id)
                    && visual.path.to_ascii_lowercase().ends_with(".glb"),
                "invalid/duplicate visual ID or unsupported visual format (GLB required)",
            )?;
        }
        for path in self.paths() {
            require(
                relative_asset(path),
                "manifest paths must be style-relative without traversal",
            )?;
        }
        match self.kind {
            LibraryKind::Track => {
                require(
                    self.mechanical_definition.is_none(),
                    "track manifest cannot contain train mechanics",
                )?;
                if let Some(c) = &self.construction {
                    c.validate()?;
                }
            }
            LibraryKind::Train => require(
                self.construction.is_none(),
                "train manifest cannot contain track construction",
            )?,
        }
        Ok(())
    }
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.visuals
            .iter()
            .map(|v| v.path.as_str())
            .chain(self.previews.thumbnail.as_deref())
            .chain(self.previews.large.as_deref())
            .chain(self.previews.references.iter().map(String::as_str))
            .chain(self.mechanical_definition.as_deref())
    }
}
