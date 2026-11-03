//! Station assignments are independent of Regions, vehicles, documents and GPUs.
//! Intervals are [start,end), except the last interval also owns the final station.
use crate::{
    CatalogError, Result,
    construction::{Role, TrackConstruction},
    identifier,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const MAX_SEGMENTS: usize = 1024;
pub const MAX_MAP_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionSegment {
    pub id: String,
    pub start_m: f64,
    pub end_m: f64,
    pub style_id: String,
    pub variant_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstructionMap {
    pub version: u32,
    pub segments: Vec<ConstructionSegment>,
}

/// Explicit registration under a parent style, never a universal style variant.
/// Callers supply verified definitions (e.g. the installed library manifest).
#[derive(Clone, Debug)]
pub struct ConstructionVariant {
    pub style_id: String,
    pub id: String,
    pub diagnostic_only: bool,
    pub definition: TrackConstruction,
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedSegment<'a> {
    pub assignment: &'a ConstructionSegment,
    pub variant: &'a ConstructionVariant,
}

/// Private fields keep coverage/compatibility valid for the entire borrow.
#[derive(Debug)]
pub struct ValidatedConstructionMap<'a> {
    segments: Vec<ResolvedSegment<'a>>,
    length_m: f64,
}
impl ConstructionMap {
    pub fn from_json(text: &str) -> Result<Self> {
        if text.len() > MAX_MAP_BYTES {
            return Err(CatalogError("construction map exceeds 1 MiB".into()));
        }
        serde_json::from_str(text).map_err(|e| CatalogError(format!("construction map JSON: {e}")))
    }

    pub fn validate<'a>(
        &'a self,
        length_m: f64,
        variants: &'a [ConstructionVariant],
    ) -> Result<ValidatedConstructionMap<'a>> {
        let fail = |message: String| CatalogError(format!("construction map: {message}"));
        if self.version != 1 || self.segments.is_empty() || self.segments.len() > MAX_SEGMENTS {
            return Err(fail("expected version 1 and 1..1024 segments".into()));
        }
        if !length_m.is_finite() || length_m <= 0. {
            return Err(fail("track length must be positive finite metres".into()));
        }
        let mut registry = BTreeMap::new();
        for v in variants {
            if !identifier(&v.style_id)
                || !identifier(&v.id)
                || registry.insert((&v.style_id, &v.id), v).is_some()
            {
                return Err(fail(
                    "invalid or duplicate style/variant registration".into(),
                ));
            }
            v.definition.validate()?;
        }
        let mut ordered: Vec<_> = self.segments.iter().collect();
        ordered.sort_by(|a, b| a.start_m.total_cmp(&b.start_m).then(a.id.cmp(&b.id)));
        let mut ids = HashSet::new();
        let mut resolved: Vec<ResolvedSegment<'a>> = Vec::with_capacity(ordered.len());
        let mut previous_end = 0.;
        let mut materials = BTreeMap::new();
        for s in ordered {
            if !identifier(&s.id)
                || !ids.insert(&s.id)
                || !s.start_m.is_finite()
                || !s.end_m.is_finite()
                || s.start_m < 0.
                || s.end_m > length_m
                || s.end_m <= s.start_m
            {
                return Err(fail(format!(
                    "segment {}: invalid ID or interval [{}, {}] for length {length_m}",
                    s.id, s.start_m, s.end_m
                )));
            }
            if s.start_m != previous_end {
                return Err(fail(format!(
                    "segment {}: {} between {previous_end} and {} m; coverage must be exact",
                    s.id,
                    if s.start_m < previous_end {
                        "overlap"
                    } else {
                        "gap"
                    },
                    s.start_m
                )));
            }
            let v = registry.get(&(&s.style_id, &s.variant_id)).ok_or_else(|| {
                fail(format!(
                    "segment {}: variant {}/{} is not registered for this style",
                    s.id, s.style_id, s.variant_id
                ))
            })?;
            if let Some(prev) = resolved.last() {
                let a = &prev.variant.definition;
                let b = &v.definition;
                // Conservative equality includes mounting offsets, gauge, count,
                // profiles, winding/UV settings and appearance. No rail morphing.
                if prev.variant.style_id != v.style_id
                    || a.strategy != b.strategy
                    || !a
                        .continuous
                        .iter()
                        .filter(|c| c.role == Role::Running)
                        .eq(b.continuous.iter().filter(|c| c.role == Role::Running))
                {
                    return Err(fail(format!(
                        "boundary {} -> {} at {} m: incompatible running rails/style (count, gauge, profile or mounting); explicit adapter specification required",
                        prev.assignment.id, s.id, s.start_m
                    )));
                }
            }
            for m in &v.definition.materials {
                if let Some(old) = materials.insert(&m.id, m)
                    && old != m
                {
                    return Err(fail(format!(
                        "conflicting material slot {}; use distinct IDs for different finishes",
                        m.id
                    )));
                }
            }
            resolved.push(ResolvedSegment {
                assignment: s,
                variant: v,
            });
            previous_end = s.end_m;
        }
        if previous_end != length_m {
            return Err(fail(format!(
                "final coverage ends at {previous_end} m, expected {length_m} m"
            )));
        }
        Ok(ValidatedConstructionMap {
            segments: resolved,
            length_m,
        })
    }
}
impl<'a> ValidatedConstructionMap<'a> {
    pub fn segments(&self) -> &[ResolvedSegment<'a>] {
        &self.segments
    }
    pub fn length_m(&self) -> f64 {
        self.length_m
    }
    pub fn at_station(&self, station_m: f64) -> Result<ResolvedSegment<'a>> {
        if !station_m.is_finite() || station_m < 0. || station_m > self.length_m {
            return Err(CatalogError("construction station outside track".into()));
        }
        let i = self
            .segments
            .partition_point(|s| s.assignment.start_m <= station_m)
            .saturating_sub(1);
        Ok(self.segments[i])
    }
}

/// Development prototype only. Two separate rectangular spines and a simple
/// crossbar, with exactly the supplied Box-Spined-100 running rails. Dimensions
/// are test inputs, not manufacturer specifications or style 200/300 activation.
pub fn box_spined_diagnostic_variants(
    base: &TrackConstruction,
) -> Result<Vec<ConstructionVariant>> {
    use crate::construction::{HardwareShape, Profile, Role, Sweep};
    if base.id != "box-spined-100" {
        return Err(CatalogError(
            "diagnostic pair requires verified box-spined-100 construction".into(),
        ));
    }
    base.validate()?;
    let mut double = base.clone();
    double.id = "box-spined-100-diagnostic-double".into();
    double.continuous.retain(|c| c.role == Role::Running);
    for (id, lateral) in [
        ("diagnostic-spine-left", -0.32),
        ("diagnostic-spine-right", 0.32),
    ] {
        double.continuous.push(Sweep {
            id: id.into(),
            role: Role::Structure,
            profile: Profile::Box {
                height_m: 0.50,
                width_m: 0.22,
            },
            offset_m: [0., -0.60, lateral],
            rotation_rad: 0.,
            material: "structure".into(),
            visible: true,
            cap_ends: true,
        });
    }
    for tie in &mut double.repeated {
        tie.id = "diagnostic-crossbar".into();
        tie.shape = HardwareShape::Box {
            size_m: [0.04, 0.18, 1.20],
        };
        tie.offset_m = [0., -0.18, 0.];
        tie.rotation_rad = [0.; 3];
    }
    double.validate()?;
    Ok(vec![
        ConstructionVariant {
            style_id: base.id.clone(),
            id: "source-single".into(),
            diagnostic_only: false,
            definition: base.clone(),
        },
        ConstructionVariant {
            style_id: base.id.clone(),
            id: "diagnostic-double".into(),
            diagnostic_only: true,
            definition: double,
        },
    ])
}
