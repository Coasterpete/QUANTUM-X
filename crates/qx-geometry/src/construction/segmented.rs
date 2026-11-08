//! Segmentation adapter over the existing sweep/repeater engine. Rails are built
//! once, structural runs are clipped in derived memory, source samples immutable.
use super::*;
use qx_style::construction_segments::ValidatedConstructionMap;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BoundaryDiagnostic {
    pub station_m: f64,
    pub before_segment: String,
    pub after_segment: String,
    pub message: String,
}
#[derive(Debug)]
pub struct SegmentedMesh {
    pub mesh: TrackMesh,
    pub boundaries: Vec<BoundaryDiagnostic>,
    /// Rigid hardware that would straddle a changed structural boundary is
    /// omitted, never cut/stretched. Global pitch is preserved elsewhere.
    pub omitted_hardware: Vec<(String, f64)>,
}

pub fn build(
    track: &GeneratedTrack,
    map: &ValidatedConstructionMap<'_>,
    limits: MeshLimits,
    assets: &dyn ConstructionAssets,
) -> Result<SegmentedMesh> {
    validate_samples(track)?;
    ensure(
        track.length_m() == map.length_m(),
        "construction map validated for another track length",
    )?;
    let segments = map.segments();
    let mut budget = Budget {
        limits,
        vertices: 0,
        indices: 0,
        instances: 0,
        bytes: 0,
    };
    let mut omitted_hardware = vec![];
    // Exact original rail sample sequence: no internal caps, extra surfaces,
    // changed normals, UV restart or duplicated rings at construction changes.
    let mut components = build_components(
        track,
        &track.samples,
        &segments[0].variant.definition,
        Part::Running,
        None,
        assets,
        &mut budget,
        &mut omitted_hardware,
    )?;
    let mut boundaries = vec![];
    for pair in segments.windows(2) {
        if pair[0].variant.definition != pair[1].variant.definition {
            boundaries.push(BoundaryDiagnostic {
                station_m:pair[1].assignment.start_m,
                before_segment:pair[0].assignment.id.clone(), after_segment:pair[1].assignment.id.clone(),
                message:"PROVISIONAL capped structural termination; explicit transition asset/approved junction specification required. No structural certification; overlapping cross-sections and mechanical connectivity unverified.".into(),
            });
        }
    }
    let mut materials = Vec::new();
    for s in segments {
        for m in &s.variant.definition.materials {
            if !materials.iter().any(|old: &Material| old.id == m.id) {
                materials.push(m.clone());
            }
        }
    }
    let mut i = 0;
    while i < segments.len() {
        // Identical adjacent definitions form one run. No coincident internal
        // caps or rephased ties just because the assignment IDs differ.
        let mut end = i + 1;
        while end < segments.len()
            && segments[end].variant.definition == segments[i].variant.definition
        {
            end += 1;
        }
        let start_m = segments[i].assignment.start_m;
        let end_m = segments[end - 1].assignment.end_m;
        ensure(
            end_m - start_m >= 1e-6,
            "construction structural run is shorter than 1 micrometre mesh resolution",
        )?;
        let a = track.samples.partition_point(|s| s.distance_m <= start_m);
        let b = track.samples.partition_point(|s| s.distance_m < end_m);
        let mut samples = allocated(b.saturating_sub(a) + 2)?;
        // Exact existing endpoint samples bypass interpolation roundoff.
        let at = |station| -> Result<CenterlineSample> {
            let index = track.samples.partition_point(|s| s.distance_m < station);
            if let Some(s) = track.samples.get(index)
                && s.distance_m == station
            {
                return Ok(s.clone());
            }
            track.sample_at_distance(station)
        };
        samples.push(at(start_m)?);
        samples.extend_from_slice(&track.samples[a..b]);
        samples.push(at(end_m)?);
        let window = HardwareWindow {
            start: start_m,
            end: end_m,
            final_interval: end == segments.len(),
        };
        let mut parts = build_components(
            track,
            &samples,
            &segments[i].variant.definition,
            Part::Structure,
            Some(window),
            assets,
            &mut budget,
            &mut omitted_hardware,
        )?;
        for c in &mut parts {
            c.id = format!("{}::{}", segments[i].assignment.id, c.id);
        }
        components.extend(parts);
        i = end;
    }
    Ok(SegmentedMesh {
        mesh: finish_mesh(components, materials, budget, track.samples[0].position_m)?,
        boundaries,
        omitted_hardware,
    })
}

/// Known geometric reference only: original centerline/frame at a station.
/// A variant does not imply a verified connector/mount offset or hardware solid.
pub fn attachment_reference(
    track: &GeneratedTrack,
    map: &ValidatedConstructionMap<'_>,
    station_m: f64,
    hardware_id: Option<&str>,
) -> Result<CenterlineSample> {
    let segment = map
        .at_station(station_m)
        .map_err(|e| crate::GeometryError(e.to_string()))?;
    ensure(
        track.length_m() == map.length_m(),
        "attachment map length mismatch",
    )?;
    if let Some(id) = hardware_id {
        return Err(crate::GeometryError(format!(
            "support hardware {id} is not verified for {}/{}; centerline/frame reference only",
            segment.variant.style_id, segment.variant.id
        )));
    }
    track.sample_at_distance(station_m)
}
