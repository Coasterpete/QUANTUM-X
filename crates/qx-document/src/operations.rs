//! Persisted physical commands. SI units, distinct from authored design speed.
use crate::{Result, require};
use qx_geometry::{
    solver::GeneratedTrack,
    topology::LayoutMode,
    transition::{Channel, Function, Transition, Unit},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalSettings {
    pub initial_speed_mps: f64,
    pub gravity_mps2: f64,
    pub rolling_resistance_coefficient: f64,
    pub air_density_kg_m3: f64,
    /// Total train drag coefficient times frontal area, m².
    pub drag_area_m2: f64,
}
impl Default for PhysicalSettings {
    fn default() -> Self {
        Self {
            initial_speed_mps: 0.,
            gravity_mps2: 9.81,
            rolling_resistance_coefficient: 0.,
            air_density_kg_m3: 1.225,
            drag_area_m2: 0.,
        }
    }
}
impl PhysicalSettings {
    pub fn validate(&self) -> Result<()> {
        require(
            self.initial_speed_mps.is_finite()
                && self.initial_speed_mps.abs() <= 100.
                && self.gravity_mps2.is_finite()
                && (0. ..=50.).contains(&self.gravity_mps2)
                && self.rolling_resistance_coefficient.is_finite()
                && (0. ..=0.2).contains(&self.rolling_resistance_coefficient)
                && self.air_density_kg_m3.is_finite()
                && (0. ..=10.).contains(&self.air_density_kg_m3)
                && self.drag_area_m2.is_finite()
                && (0. ..=100.).contains(&self.drag_area_m2),
            "invalid simulation speed, gravity or resistance settings",
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Launch,
    Brake,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccelerationKey {
    pub distance_m: f64,
    pub acceleration_mps2: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackDevice {
    pub id: u64,
    pub name: String,
    pub kind: DeviceKind,
    pub enabled: bool,
    pub start_m: f64,
    pub end_m: f64,
    pub acceleration_mps2: f64,
    pub maximum_force_n: f64,
    /// Launch direction relative to track tangent. Brakes always oppose motion.
    pub reverse_launch: bool,
    /// Legacy piecewise-linear magnitude profile in device-local metres.
    pub profile: Vec<AccelerationKey>,
    /// Transition Editor profile. Mutually exclusive with legacy keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceleration_curve: Option<Channel>,
}
impl TrackDevice {
    pub fn new(id: u64, kind: DeviceKind, start_m: f64, end_m: f64) -> Self {
        Self {
            id,
            name: format!("{kind:?} {id}"),
            kind,
            enabled: true,
            start_m,
            end_m,
            acceleration_mps2: 2.,
            maximum_force_n: 100_000.,
            reverse_launch: false,
            profile: vec![],
            acceleration_curve: None,
        }
    }
    pub fn validate(&self) -> Result<()> {
        require(
            self.id > 0 && !self.name.trim().is_empty() && self.name.len() <= 1024,
            "invalid track device identity",
        )?;
        require(
            [
                self.start_m,
                self.end_m,
                self.acceleration_mps2,
                self.maximum_force_n,
            ]
            .iter()
            .all(|v| v.is_finite())
                && self.start_m >= 0.
                && self.end_m >= 0.
                && self.start_m != self.end_m
                && (0. ..=100.).contains(&self.acceleration_mps2)
                && (0. ..=1e9).contains(&self.maximum_force_n)
                && self.profile.len() <= 1024,
            "invalid device interval, acceleration or force limit",
        )?;
        if !self.profile.is_empty() {
            require(
                self.profile.len() >= 2 && self.profile[0].distance_m == 0.,
                "device profile must start at local 0 and have at least two keys",
            )?;
            require(
                self.profile.iter().all(|k| {
                    k.distance_m.is_finite()
                        && k.distance_m >= 0.
                        && k.acceleration_mps2.is_finite()
                        && (0. ..=100.).contains(&k.acceleration_mps2)
                }) && self
                    .profile
                    .windows(2)
                    .all(|k| k[0].distance_m < k[1].distance_m),
                "invalid acceleration profile keys",
            )?;
        }
        if let Some(curve) = &self.acceleration_curve {
            require(
                self.profile.is_empty(),
                "device has both legacy keys and a transition curve",
            )?;
            validate_acceleration_curve(curve)?;
        }
        Ok(())
    }
    pub fn length(&self, length: f64, circuit: bool) -> Result<f64> {
        self.validate()?;
        require(
            length.is_finite() && length > 0. && self.start_m < length && self.end_m <= length,
            "device stations exceed current track; revise its start/end positions",
        )?;
        require(
            self.start_m < self.end_m || circuit,
            "seam-crossing device requires a valid authored circuit",
        )?;
        let span = if self.end_m > self.start_m {
            self.end_m - self.start_m
        } else {
            length - self.start_m + self.end_m
        };
        require(
            self.profile
                .last()
                .is_none_or(|k| (k.distance_m - span).abs() <= 1e-7),
            "profile must cover the exact device-local length; explicitly fit it after resizing",
        )?;
        require(
            self.acceleration_curve
                .as_ref()
                .is_none_or(|c| (c.length() - span).abs() <= 1e-7),
            "acceleration curve must cover the exact device-local length; explicitly fit it after resizing",
        )?;
        Ok(span)
    }
    pub fn local_distance(&self, station: f64, length: f64, circuit: bool) -> Option<f64> {
        if !station.is_finite() || station < 0. || station > length {
            return None;
        }
        let s = if circuit && station == length {
            0.
        } else {
            station
        };
        if self.start_m < self.end_m {
            (s >= self.start_m && s < self.end_m).then_some(s - self.start_m)
        } else if circuit && (s >= self.start_m || s < self.end_m) {
            Some((s - self.start_m).rem_euclid(length))
        } else {
            None
        }
    }
    pub fn command(&self, local: f64) -> f64 {
        if let Some(curve) = &self.acceleration_curve {
            return curve
                .evaluate(local.clamp(0., curve.length()))
                .map_or(0., |e| e.value.clamp(0., 100.));
        }
        if self.profile.is_empty() {
            return self.acceleration_mps2;
        }
        let i = self
            .profile
            .partition_point(|k| k.distance_m < local)
            .clamp(1, self.profile.len() - 1);
        let a = &self.profile[i - 1];
        let b = &self.profile[i];
        a.acceleration_mps2
            + (b.acceleration_mps2 - a.acceleration_mps2)
                * ((local - a.distance_m) / (b.distance_m - a.distance_m)).clamp(0., 1.)
    }
    pub fn fit_profile(&mut self, length: f64, circuit: bool) -> Result<()> {
        let mut plain = self.clone();
        plain.profile.clear();
        plain.acceleration_curve = None;
        let span = plain.length(length, circuit)?;
        if let Some(curve) = &mut self.acceleration_curve {
            let scale = span / curve.length();
            for t in &mut curve.transitions {
                t.length *= scale;
                if let Function::Hermite {
                    start_slope,
                    end_slope,
                } = &mut t.function
                {
                    *start_slope /= scale;
                    *end_slope /= scale;
                }
            }
        }
        if let Some(last) = self.profile.last() {
            let old = last.distance_m;
            require(old > 0., "profile has zero length")?;
            for key in &mut self.profile {
                key.distance_m *= span / old;
            }
        }
        Ok(())
    }
    /// Exact legacy conversion; choosing the editor never reshapes existing keys.
    pub fn editable_curve(&self, span: f64) -> Channel {
        if let Some(curve) = &self.acceleration_curve {
            return curve.clone();
        }
        let transitions = if self.profile.is_empty() {
            vec![Transition {
                function: Function::Linear {},
                ..Transition::constant(1, span, self.acceleration_mps2)
            }]
        } else {
            self.profile
                .windows(2)
                .enumerate()
                .map(|(i, k)| Transition {
                    id: i as u64 + 1,
                    length: k[1].distance_m - k[0].distance_m,
                    start: k[0].acceleration_mps2,
                    end: k[1].acceleration_mps2,
                    amplitude: 0.,
                    function: Function::Linear {},
                })
                .collect()
        };
        Channel {
            unit: Unit::MetresPerSecondSquared,
            transitions,
        }
    }
}

/// Positive-weight NURBS stay in their control hull. Polynomial functions are
/// converted exactly to that basis; inconclusive hulls are subdivided. Cosines
/// use analytic trigonometric bounds plus the polynomial hull. Sampling alone
/// cannot prove a nonnegative command between samples.
pub fn validate_acceleration_curve(curve: &Channel) -> Result<()> {
    require(
        curve.unit == Unit::MetresPerSecondSquared && curve.transitions.len() <= 1024,
        "acceleration curve requires m/s² and at most 1024 regions",
    )?;
    curve.validate()?;
    fn bounded(t: &Transition, depth: u8) -> bool {
        let bounds = if let Ok(n) = t.to_nurbs() {
            let Function::Nurbs { profile } = n.function else {
                unreachable!()
            };
            (
                profile.values.iter().copied().fold(f64::INFINITY, f64::min),
                profile
                    .values
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max),
            )
        } else {
            let (offset, scale, lo, hi, polynomial) = match t.function {
                Function::Cosine {} => (
                    (t.start + t.end) * 0.5,
                    (t.start - t.end) * 0.5,
                    0.,
                    std::f64::consts::PI,
                    [
                        0.,
                        0.,
                        16. * t.amplitude,
                        -32. * t.amplitude,
                        16. * t.amplitude,
                    ],
                ),
                Function::CosineSpan {
                    offset,
                    scale,
                    phase_start,
                    phase_end,
                    polynomial,
                } => (offset, scale, phase_start, phase_end, polynomial),
                _ => return false,
            };
            let p = qx_geometry::profile::NurbsProfile::polynomial(&polynomial, 4);
            let (mut a, mut b) = (lo.cos(), hi.cos());
            // Cosine restrictions originate in [0, pi]. Reject unbounded phases
            // conservatively rather than trusting a sampled extremum.
            if lo < 0. || hi > std::f64::consts::PI {
                a = -1.;
                b = 1.;
            }
            let (a, b) = ((scale * a).min(scale * b), (scale * a).max(scale * b));
            (
                offset + a + p.values.iter().copied().fold(f64::INFINITY, f64::min),
                offset + b + p.values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            )
        };
        if bounds.0 >= -1e-10 && bounds.1 <= 100. + 1e-10 {
            return true;
        }
        if depth == 0 || t.length < 2e-8 {
            return false;
        }
        t.split(0.5, if t.id == u64::MAX { 1 } else { t.id + 1 })
            .is_ok_and(|(a, b)| bounded(&a, depth - 1) && bounded(&b, depth - 1))
    }
    for t in &curve.transitions {
        require(
            bounded(t, 10),
            "acceleration curve leaves 0…100 m/s²; revise its values, amplitude, slopes or handles",
        )?;
    }
    Ok(())
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operations {
    pub layout: LayoutMode,
    pub physical: PhysicalSettings,
    pub devices: Vec<TrackDevice>,
}
impl Operations {
    pub fn validate(&self) -> Result<()> {
        self.physical.validate()?;
        require(self.devices.len() <= 1024, "too many devices")?;
        let mut ids = std::collections::HashSet::new();
        for d in &self.devices {
            d.validate()?;
            require(ids.insert(d.id), "duplicate track device ID")?;
        }
        Ok(())
    }
    pub fn circuit(&self, track: &GeneratedTrack) -> Result<bool> {
        Ok(self.layout == LayoutMode::Circuit && qx_geometry::topology::diagnose(track)?.valid)
    }
    pub fn validate_track(&self, track: &GeneratedTrack) -> Result<()> {
        let circuit = self.circuit(track)?;
        for d in &self.devices {
            d.length(track.length_m(), circuit)?;
        }
        Ok(())
    }
}
