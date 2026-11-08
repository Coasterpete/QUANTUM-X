//! User precision preferences and pure authored-coordinate snapping. No document
//! or rendering state is changed by enabling/disabling these settings.
use qx_geometry::{authoring::Domain, transition::Unit};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Settings {
    pub enabled: bool,
    pub graph: Graph,
    pub spatial: Spatial,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Graph {
    pub enabled: bool,
    pub horizontal: bool,
    pub vertical: bool,
    pub points: bool,
    pub handles: bool,
    pub boundaries: bool,
    pub nearby_keys: bool,
    pub station_m: f64,
    pub fine_station_m: f64,
    pub time_s: f64,
    pub normalized: f64,
    pub force_g: f64,
    pub rate_rad_per_m: f64,
    pub rate_rad_per_s: f64,
    pub rate_rad_per_u: f64,
    pub acceleration_mps2: f64,
    pub key_radius_points: f64,
}
impl Default for Graph {
    fn default() -> Self {
        Self {
            enabled: true,
            horizontal: true,
            vertical: true,
            points: true,
            handles: true,
            boundaries: true,
            nearby_keys: false,
            station_m: 1.,
            fine_station_m: 0.1,
            time_s: 0.1,
            normalized: 0.01,
            force_g: 0.1,
            rate_rad_per_m: 0.005,
            rate_rad_per_s: 0.05,
            rate_rad_per_u: 0.05,
            acceleration_mps2: 0.1,
            key_radius_points: 8.,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct Spatial {
    pub enabled: bool,
    pub world_grid: bool,
    pub distance: bool,
    pub angles: bool,
    pub nearby_points: bool,
    pub track_station: bool,
    pub grid_m: f64,
    pub distance_m: f64,
    pub fine_distance_m: f64,
    pub angle_degrees: f64,
    pub station_m: f64,
    pub nearby_radius_m: f64,
}
impl Default for Spatial {
    fn default() -> Self {
        Self {
            enabled: true,
            world_grid: true,
            distance: true,
            angles: true,
            nearby_points: false,
            track_station: true,
            grid_m: 1.,
            distance_m: 1.,
            fine_distance_m: 0.1,
            angle_degrees: 5.,
            station_m: 1.,
            nearby_radius_m: 0.15,
        }
    }
}

fn valid_step(step: f64) -> bool {
    step.is_finite() && (1e-10..=1e9).contains(&step)
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let g = self.graph;
        let s = self.spatial;
        if [
            g.station_m,
            g.fine_station_m,
            g.time_s,
            g.normalized,
            g.force_g,
            g.rate_rad_per_m,
            g.rate_rad_per_s,
            g.rate_rad_per_u,
            g.acceleration_mps2,
            s.grid_m,
            s.distance_m,
            s.fine_distance_m,
            s.angle_degrees,
            s.station_m,
            s.nearby_radius_m,
        ]
        .into_iter()
        .any(|v| !valid_step(v))
            || !g.key_radius_points.is_finite()
            || !(1. ..=64.).contains(&g.key_radius_points)
            || s.angle_degrees > 360.
        {
            return Err("Snap increments must be finite and positive (1e-10…1e9); angle ≤360°, key radius 1…64 px.".into());
        }
        Ok(())
    }
    pub fn graph_active(self, bypass: bool) -> bool {
        self.enabled && self.graph.enabled && !bypass
    }
    pub fn spatial_active(self, bypass: bool) -> bool {
        self.enabled && self.spatial.enabled && !bypass
    }
}
impl Graph {
    pub fn x_step(self, domain: Domain, fine: bool) -> f64 {
        match domain {
            Domain::Distance if fine => self.fine_station_m,
            Domain::Distance => self.station_m,
            Domain::Time => self.time_s,
            Domain::Normalized => self.normalized,
        }
    }
    pub fn y_step(self, unit: Unit) -> f64 {
        match unit {
            Unit::G => self.force_g,
            Unit::RadiansPerMetre => self.rate_rad_per_m,
            Unit::RadiansPerSecond => self.rate_rad_per_s,
            Unit::RadiansPerNormalized => self.rate_rad_per_u,
            Unit::MetresPerSecondSquared => self.acceleration_mps2,
        }
    }
}

/// Always quantize from the raw gesture, never from the preceding snapped value.
/// Existing-key attraction uses a separate tolerance in authored units (the
/// graph adapter converts logical pixels using the mapping frozen at press).
pub(super) fn scalar(raw: f64, step: Option<f64>, keys: &[f64], radius: f64) -> f64 {
    if !raw.is_finite() {
        return raw;
    }
    let grid = step.filter(|s| valid_step(*s)).and_then(|s| {
        let scaled = raw / s;
        // At this scale a float cannot resolve a single lattice cell.
        (scaled.is_finite() && scaled.abs() < (1u64 << 52) as f64).then(|| scaled.round() * s)
    });
    let mut value = grid.filter(|v| v.is_finite()).unwrap_or(raw);
    let mut error = grid.map_or(f64::INFINITY, |v| (v - raw).abs());
    for &key in keys {
        let distance = (key - raw).abs();
        if key.is_finite() && distance <= radius && distance < error {
            value = key;
            error = distance;
        }
    }
    value
}

#[derive(Clone, Copy)]
pub(super) enum Space {
    World,
    Local,
    Station,
    Angle,
}
#[derive(Clone, Copy)]
pub(super) enum GraphAxis {
    Boundary(Domain),
    PointStation(Domain),
    HandleStation(Domain),
    Value(Unit),
    Handle(Unit),
}
impl Settings {
    pub fn graph_step(self, axis: GraphAxis, fine: bool, bypass: bool) -> Option<f64> {
        if !self.graph_active(bypass) {
            return None;
        }
        let g = self.graph;
        match axis {
            GraphAxis::Boundary(domain) => {
                (g.horizontal && g.boundaries).then(|| g.x_step(domain, fine))
            }
            GraphAxis::PointStation(domain) => {
                (g.horizontal && g.points).then(|| g.x_step(domain, fine))
            }
            GraphAxis::HandleStation(domain) => {
                (g.horizontal && g.handles).then(|| g.x_step(domain, fine))
            }
            GraphAxis::Value(unit) => (g.vertical && g.points).then(|| g.y_step(unit)),
            GraphAxis::Handle(unit) => (g.vertical && g.handles).then(|| g.y_step(unit)),
        }
    }
}
impl Settings {
    pub fn spatial_value(
        self,
        raw: f64,
        space: Space,
        keys: &[f64],
        fine: bool,
        bypass: bool,
    ) -> f64 {
        if !self.spatial_active(bypass) {
            return raw;
        }
        let s = self.spatial;
        let step = match space {
            Space::World => s.world_grid.then_some(s.grid_m),
            Space::Local => s.distance.then_some(s.distance_m),
            Space::Station => s.track_station.then_some(s.station_m),
            Space::Angle => s.angles.then_some(s.angle_degrees.to_radians()),
        }
        .map(|step| {
            if fine && !matches!(space, Space::Angle) {
                s.fine_distance_m
            } else {
                step
            }
        });
        let keys = if s.nearby_points && !matches!(space, Space::Angle) {
            keys
        } else {
            &[]
        };
        scalar(raw, step, keys, s.nearby_radius_m)
    }
}

pub(super) fn unit_label(unit: Unit) -> &'static str {
    match unit {
        Unit::G => "g",
        Unit::RadiansPerMetre => "rad/m",
        Unit::RadiansPerSecond => "rad/s",
        Unit::RadiansPerNormalized => "rad/u",
        Unit::MetresPerSecondSquared => "m/s²",
    }
}
