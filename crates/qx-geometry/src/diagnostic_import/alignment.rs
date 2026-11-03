use super::{ImportedSupports, ImportedTrack};
use crate::{Result, ensure, math::Vec3};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ConnectorResidual {
    pub node_id: String,
    pub track_index: u32,
    pub xml_station_m: f64,
    pub nearest_station_m: f64,
    pub nearest_distance_m: f64,
    pub local_offset_t_up_right_m: Vec3,
    pub csv_nearest_position_m: Vec3,
    pub connector_position_m: Vec3,
    /// Only meaningful for the imported (index-0) track; no extrapolation.
    pub station_position_distance_m: Option<f64>,
    /// XML supplies no orientation frame. Never substitute beam direction.
    pub connector_orientation_residual_rad: Option<f64>,
    pub csv_tangent_to_chord_angle_rad: f64,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AlignmentReport {
    pub residuals: Vec<ConnectorResidual>,
    pub matched_track_connectors: usize,
    pub unavailable_track_connectors: usize,
    pub max_nearest_distance_m: f64,
    pub rms_nearest_distance_m: f64,
}
pub fn alignment(track: &ImportedTrack, supports: &ImportedSupports) -> Result<AlignmentReport> {
    let count: usize = supports.connectors.iter().map(|c| c.node_ids.len()).sum();
    ensure(
        count.saturating_mul(track.samples.len()) <= 20_000_000,
        "alignment comparison budget exceeded",
    )?;
    let generated = track.construction_track();
    let mut out = AlignmentReport {
        residuals: vec![],
        matched_track_connectors: 0,
        unavailable_track_connectors: 0,
        max_nearest_distance_m: 0.,
        rms_nearest_distance_m: 0.,
    };
    let mut sum = 0.;
    for c in &supports.connectors {
        for id in &c.node_ids {
            let p = supports.nodes[id].position_m;
            let mut best = (f64::INFINITY, 0., Vec3::ZERO, 0, 0.);
            for (i, pair) in track.samples.windows(2).enumerate() {
                let a = &pair[0];
                let b = &pair[1];
                let delta = b.position_m - a.position_m;
                let t = ((p - a.position_m).dot(delta) / delta.dot(delta)).clamp(0., 1.);
                let q = a.position_m + delta * t;
                let d = (p - q).length();
                if d < best.0 {
                    best = (d, a.distance_m + (b.distance_m - a.distance_m) * t, q, i, t);
                }
            }
            let (d, s, q, i, t) = best;
            let a = &track.samples[i];
            let b = &track.samples[i + 1];
            let f =
                crate::math::Frame::new(a.frame.orientation().slerp(b.frame.orientation(), t)?)?;
            let chord = (b.position_m - a.position_m).normalized()?;
            let angle = f
                .tangent()
                .cross(chord)
                .length()
                .atan2(f.tangent().dot(chord));
            let station = if c.track_index == 0 && c.station_m <= generated.length_m() {
                Some((p - generated.sample_at_distance(c.station_m)?.position_m).length())
            } else {
                None
            };
            if c.track_index == 0 {
                out.matched_track_connectors += 1;
                sum += d * d;
                out.max_nearest_distance_m = out.max_nearest_distance_m.max(d);
            } else {
                out.unavailable_track_connectors += 1;
            }
            out.residuals.push(ConnectorResidual {
                node_id: id.clone(),
                track_index: c.track_index,
                xml_station_m: c.station_m,
                nearest_station_m: s,
                nearest_distance_m: d,
                local_offset_t_up_right_m: f.world_to_local(p - q),
                csv_nearest_position_m: q,
                connector_position_m: p,
                station_position_distance_m: station,
                connector_orientation_residual_rad: None,
                csv_tangent_to_chord_angle_rad: angle,
            });
        }
    }
    if out.matched_track_connectors > 0 {
        out.rms_nearest_distance_m = (sum / out.matched_track_connectors as f64).sqrt();
    }
    Ok(out)
}
