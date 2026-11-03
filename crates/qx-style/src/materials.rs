//! Reusable, renderer-independent track material metadata. Paths are relative
//! to the material library directory; assignments never depend on family names.
use crate::{Result, identifier, relative_asset, require};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    #[serde(default = "unit_uv")]
    pub uv_scale: [f32; 2],
    #[serde(default)]
    pub uv_offset: [f32; 2],
    /// Linear RGB tint, independent of the library finish and texture maps.
    #[serde(default = "white_tint", skip_serializing_if = "is_white_tint")]
    pub tint: [f32; 3],
}
fn white_tint() -> [f32; 3] {
    [1.; 3]
}
fn is_white_tint(tint: &[f32; 3]) -> bool {
    *tint == white_tint()
}
fn unit_uv() -> [f32; 2] {
    [1.; 2]
}
impl Binding {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            uv_scale: unit_uv(),
            uv_offset: [0.; 2],
            tint: white_tint(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        require(
            identifier(&self.id)
                && self
                    .tint
                    .iter()
                    .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                && self
                    .uv_scale
                    .iter()
                    .all(|v| v.is_finite() && v.abs() >= 1e-6 && v.abs() <= 1e4)
                && self
                    .uv_offset
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e6),
            "invalid material ID or UV transform",
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    R,
    G,
    B,
    A,
}
impl Channel {
    pub fn index(self) -> usize {
        match self {
            Self::R => 0,
            Self::G => 1,
            Self::B => 2,
            Self::A => 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarMap {
    pub path: String,
    #[serde(default)]
    pub channel: Channel,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalConvention {
    #[default]
    OpenGl,
    DirectX,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Maps {
    pub base_color: Option<String>,
    pub normal: Option<String>,
    pub metallic: Option<ScalarMap>,
    /// Linear roughness data, used directly. Map semantics come from this field,
    /// never the source filename. Mutually exclusive with `smoothness`.
    pub roughness: Option<ScalarMap>,
    /// Linear smoothness data. The renderer converts to roughness exactly once
    /// as `1 - sample`, before multiplying by the roughness factor.
    pub smoothness: Option<ScalarMap>,
    pub ambient_occlusion: Option<ScalarMap>,
    pub emissive: Option<String>,
    /// Preserved metadata only: M1 does not displace geometry or apply edge masks.
    pub height: Option<String>,
    pub edge: Option<String>,
}
impl Maps {
    /// Semantic labels and color spaces are metadata, never filename guesses.
    pub fn entries(&self) -> Vec<(&'static str, &str, &'static str, bool)> {
        let mut entries = Vec::new();
        for (label, path, space, rendered) in [
            ("Base color", self.base_color.as_deref(), "sRGB", true),
            ("Normal", self.normal.as_deref(), "Linear", true),
            (
                "Metallic",
                self.metallic.as_ref().map(|m| m.path.as_str()),
                "Linear",
                true,
            ),
            (
                "Roughness",
                self.roughness.as_ref().map(|m| m.path.as_str()),
                "Linear",
                true,
            ),
            (
                "Smoothness",
                self.smoothness.as_ref().map(|m| m.path.as_str()),
                "Linear",
                true,
            ),
            (
                "Ambient occlusion",
                self.ambient_occlusion.as_ref().map(|m| m.path.as_str()),
                "Linear",
                true,
            ),
            ("Emission", self.emissive.as_deref(), "sRGB", true),
            ("Height", self.height.as_deref(), "Metadata only", false),
            ("Edge", self.edge.as_deref(), "Metadata only", false),
        ] {
            if let Some(path) = path {
                entries.push((label, path, space, rendered));
            }
        }
        entries
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub maps: Maps,
    /// Linear factors. White preserves the original base-color texture.
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    #[serde(default)]
    pub normal_convention: NormalConvention,
    #[serde(default = "one")]
    pub normal_scale: f32,
    #[serde(default = "one")]
    pub occlusion_strength: f32,
    #[serde(default)]
    pub emissive: [f32; 3],
}
fn one() -> f32 {
    1.
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub version: u32,
    pub materials: Vec<Definition>,
}
impl Library {
    pub fn validate(&self) -> Result<()> {
        require(
            self.version == 1 && self.materials.len() <= 256,
            "unsupported material library version or excessive material count",
        )?;
        let mut ids = HashSet::new();
        for m in &self.materials {
            require(
                identifier(&m.id) && ids.insert(&m.id) && !m.name.is_empty() && m.name.len() <= 256,
                "invalid/duplicate material ID or name",
            )?;
            require(
                m.base_color
                    .iter()
                    .chain([&m.metallic, &m.roughness, &m.occlusion_strength])
                    .all(|v| v.is_finite() && (0. ..=1.).contains(v))
                    && m.normal_scale.is_finite()
                    && (0. ..=4.).contains(&m.normal_scale)
                    && m.emissive
                        .iter()
                        .all(|v| v.is_finite() && (0. ..=100.).contains(v)),
                "invalid PBR factors",
            )?;
            require(
                m.maps.roughness.is_none() || m.maps.smoothness.is_none(),
                "choose roughness OR smoothness, not both",
            )?;
            for p in [
                &m.maps.base_color,
                &m.maps.normal,
                &m.maps.emissive,
                &m.maps.height,
                &m.maps.edge,
            ]
            .into_iter()
            .flatten()
            .chain(
                [
                    &m.maps.metallic,
                    &m.maps.roughness,
                    &m.maps.smoothness,
                    &m.maps.ambient_occlusion,
                ]
                .into_iter()
                .flatten()
                .map(|m| &m.path),
            ) {
                require(
                    relative_asset(p) && !p.contains('\\') && p.len() <= 512,
                    "material map must use a bounded relative path with forward slashes",
                )?;
            }
        }
        Ok(())
    }
}
