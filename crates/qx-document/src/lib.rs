//! Versioned authoring documents and shared transactional editing commands.
pub mod command;
pub mod editing;
pub mod insertion;
pub mod persistence;
pub mod sample;

use qx_geometry::{
    authoring::{InitialConditions, Section, validate_track},
    solver::{GeneratedTrack, GenerationOptions, generate},
};
use qx_style::{Catalog, StyleSelection};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const SCHEMA_VERSION: u32 = 3;
#[derive(Debug)]
pub struct DocumentError(pub String);
impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DocumentError {}
impl From<qx_geometry::GeometryError> for DocumentError {
    fn from(e: qx_geometry::GeometryError) -> Self {
        Self(e.to_string())
    }
}
impl From<qx_style::CatalogError> for DocumentError {
    fn from(e: qx_style::CatalogError) -> Self {
        Self(e.to_string())
    }
}
impl From<std::io::Error> for DocumentError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}
impl From<serde_json::Error> for DocumentError {
    fn from(e: serde_json::Error) -> Self {
        Self(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, DocumentError>;
pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(DocumentError(message.into()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub project_id: String,
    pub name: String,
    pub author: String,
    pub description: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: u32,
    pub metadata: Metadata,
    pub initial: InitialConditions,
    pub style: StyleSelection,
    pub sections: Vec<Section>,
    #[serde(default)]
    pub heartline: HeartlineSettings,
}
/// Nominal configuration, independent of visual track styles and future seat assets.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartlineSettings {
    pub authored_reference: qx_geometry::heartline::AuthoredReference,
    pub rider_offset: qx_geometry::heartline::OffsetProfile,
    pub train_configuration_id: Option<String>,
}
impl HeartlineSettings {
    pub fn validate(&self) -> Result<()> {
        self.rider_offset.validate()?;
        require(
            self.train_configuration_id
                .as_ref()
                .is_none_or(|id| !id.trim().is_empty() && id.len() <= 128),
            "train configuration ID must contain 1..128 bytes",
        )
    }
}
impl Project {
    pub fn validate(&self, catalog: &Catalog) -> Result<()> {
        require(
            self.schema_version == SCHEMA_VERSION,
            "unsupported in-memory schema version; load older versions through persistence migration",
        )?;
        require(
            !self.metadata.project_id.trim().is_empty() && self.metadata.project_id.len() <= 128,
            "invalid project ID",
        )?;
        require(
            !self.metadata.name.trim().is_empty()
                && self.metadata.name.len() <= 1024
                && self.metadata.author.len() <= 1024
                && self.metadata.description.len() <= 16384,
            "invalid project metadata",
        )?;
        catalog.validate()?;
        catalog.validate_selection(&self.style)?;
        validate_track(&self.initial, &self.sections)?;
        self.heartline.validate()?;
        Ok(())
    }
    pub fn generate(
        &self,
        catalog: &Catalog,
        options: GenerationOptions,
    ) -> Result<GeneratedTrack> {
        self.validate(catalog)?;
        Ok(generate(&self.initial, &self.sections, options)?)
    }
    /// Returns rider geometry for track authoring, or constrained physical track
    /// geometry for rider authoring. The authored source remains unmodified.
    pub fn reference_geometry(
        &self,
        generated: &GeneratedTrack,
    ) -> Result<qx_geometry::heartline::ReferenceTrack> {
        Ok(qx_geometry::heartline::generate_reference(
            generated,
            &self.heartline.rider_offset,
            self.heartline.authored_reference,
            self.initial.gravity_mps2,
        )?)
    }
}
