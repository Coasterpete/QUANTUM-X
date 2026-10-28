//! Author declarations only. Compatibility metadata never certifies dynamic behavior.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleCompatibility {
    pub train_style_id: Option<String>,
    /// Empty means unspecified, not universal compatibility.
    pub track_style_ids: Vec<String>,
    pub required_track_features: Vec<String>,
}
pub fn validate_tags(tags: &[String]) -> std::result::Result<(), String> {
    let mut seen = HashSet::new();
    if tags.len() > 256
        || tags.iter().any(|s| {
            s.is_empty()
                || s.len() > 128
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
                || !seen.insert(s)
        })
    {
        Err("invalid or duplicate compatibility ID/feature".into())
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compatibility {
    Unspecified,
    Declared,
    Incompatible(String),
}
impl VehicleCompatibility {
    pub fn validate(&self) -> std::result::Result<(), String> {
        validate_tags(&self.track_style_ids)?;
        validate_tags(&self.required_track_features)?;
        if let Some(id) = &self.train_style_id {
            validate_tags(std::slice::from_ref(id))?;
        }
        Ok(())
    }
    pub fn assess(&self, style: &super::Style) -> Compatibility {
        if !self.track_style_ids.is_empty() && !self.track_style_ids.contains(&style.id) {
            return Compatibility::Incompatible(
                "track style is outside the declared vehicle list".into(),
            );
        }
        let missing: Vec<_> = self
            .required_track_features
            .iter()
            .filter(|s| !style.vehicle_features.contains(s))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Compatibility::Incompatible(format!(
                "missing declared track features: {}",
                missing.join(", ")
            ));
        }
        if let Some(id) = &self.train_style_id
            && !style.compatible_train_ids.is_empty()
            && !style.compatible_train_ids.contains(id)
        {
            return Compatibility::Incompatible(
                "train style is outside the declared track list".into(),
            );
        }
        if self.track_style_ids.is_empty()
            && self.required_track_features.is_empty()
            && self
                .train_style_id
                .as_ref()
                .is_none_or(|id| !style.compatible_train_ids.contains(id))
        {
            Compatibility::Unspecified
        } else {
            Compatibility::Declared
        }
    }
}
