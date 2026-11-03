//! Replaceable developer sidecar adapter; never reads/writes native projects.
use qx_geometry::{
    construction::{self, Bounds, TrackMesh},
    math::Vec3,
    solver::GeneratedTrack,
};
use qx_rendering::OverlayLine;
use qx_style::{
    construction::TrackConstruction,
    construction_segments::{ConstructionMap, MAX_MAP_BYTES, box_spined_diagnostic_variants},
};
use std::{error::Error, io::Read, path::Path};

pub(super) struct MappedConstruction {
    pub mesh: TrackMesh,
    pub report: serde_json::Value,
    pub markers: Vec<OverlayLine>,
    pub focus_station_m: f64,
}
pub(super) fn build(
    path: &Path,
    track: &GeneratedTrack,
    base: &TrackConstruction,
    root: &Path,
) -> Result<MappedConstruction, Box<dyn Error>> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(MAX_MAP_BYTES as u64 + 1)
        .read_to_string(&mut text)?;
    let map = ConstructionMap::from_json(&text)?;
    let variants = box_spined_diagnostic_variants(base)?;
    let valid = map.validate(track.length_m(), &variants)?;
    let result = construction::segmented::build(
        track,
        &valid,
        Default::default(),
        &qx_rendering::construction_assets::Sources::new(root),
    )?;
    let mut markers = vec![];
    for b in &result.boundaries {
        let s = track.sample_at_distance(b.station_m)?;
        let p = |n, l| s.position_m + s.frame.normal() * n + s.frame.lateral() * l;
        for (a, b) in [
            ((-1.1, -0.85), (0.3, -0.85)),
            ((0.3, -0.85), (0.3, 0.85)),
            ((0.3, 0.85), (-1.1, 0.85)),
            ((-1.1, 0.85), (-1.1, -0.85)),
        ] {
            markers.push(OverlayLine {
                start_m: p(a.0, a.1),
                end_m: p(b.0, b.1),
                color: [1., 0.35, 0.8],
            });
        }
    }
    let report = serde_json::json!({
        "map":map,"diagnostic_only":true,"intervals":"[start,end), final interval includes end; exact coverage; metres on input reference",
        "registered_variants":variants.iter().map(|v|serde_json::json!({"style_id":v.style_id,"variant_id":v.id,"diagnostic_only":v.diagnostic_only})).collect::<Vec<_>>(),
        "boundaries":result.boundaries,"omitted_boundary_hardware":result.omitted_hardware,
        "rail_policy":"one unchanged full-track sweep per running rail; no internal caps or duplicated rail surfaces",
        "support_policy":"centerline/frame reference unchanged; no verified connector hardware for either variant; imported XML positions retained",
        "prototype":"diagnostic-double: two 0.50 x 0.22 m rectangular spines at N=-0.60, L=+-0.32; 0.04 x 0.18 x 1.20 m crossbars at N=-0.18. Test dimensions only; no verified mounts or junctions. Not Box-Spined-200/300.",
    });
    let focus_station_m = valid
        .segments()
        .get(1)
        .map_or(track.length_m() * 0.5, |s| s.assignment.start_m);
    Ok(MappedConstruction {
        mesh: result.mesh,
        report,
        markers,
        focus_station_m,
    })
}

/// Source-only bounds ensure identical comparison cameras despite spine changes.
pub(super) fn comparison_bounds(track: Bounds, supports: Option<Bounds>) -> (Vec3, Vec3) {
    let mut bounds = track;
    if let Some(s) = supports {
        bounds.union(s);
    }
    let margin = Vec3::new(2., 2., 2.);
    (bounds.min - margin, bounds.max + margin)
}
