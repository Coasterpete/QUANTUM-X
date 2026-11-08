//! Transient physical rail references, not contact surfaces or rendered meshes.
//! Callers must supply the derived PHYSICAL track (not a rider heartline).
//! All offsets use the track's T/N/L axes, in metres. No surface is inferred.
mod nearest;
pub use nearest::{NearestPath, PathCandidate, SearchOptions, SearchStatus, StationWindow};

use crate::{
    Result, ensure,
    math::{Frame, Quaternion, Vec3},
    solver::GeneratedTrack,
    topology::{self, LayoutMode},
};
use qx_style::{
    construction::{Role, TrackConstruction},
    construction_segments::ValidatedConstructionMap,
};

#[derive(Clone, Debug, PartialEq)]
pub struct RailReference {
    pub id: String,
    /// Geometric origin of the running member's profile, NOT a running surface.
    pub offset_m: Vec3,
}
impl RailReference {
    /// Select an explicit running sweep. Profile rotation/shape/material do not
    /// identify a contact surface; a separate wheel-center offset is mandatory.
    pub fn from_construction(definition: &TrackConstruction, rail_id: &str) -> Result<Self> {
        definition
            .validate()
            .map_err(|e| crate::GeometryError(e.to_string()))?;
        let rail = definition
            .continuous
            .iter()
            .find(|s| s.id == rail_id && s.role == Role::Running)
            .ok_or_else(|| crate::GeometryError(format!("unavailable running rail {rail_id}")))?;
        Ok(Self {
            id: rail.id.clone(),
            offset_m: Vec3::new(rail.offset_m[0], rail.offset_m[1], rail.offset_m[2]),
        })
    }

    /// Validated segmented maps guarantee identical running sweeps across all
    /// structural variants. Stations must already be in physical arc metres.
    pub fn from_map(
        map: &ValidatedConstructionMap<'_>,
        physical_length_m: f64,
        rail_id: &str,
    ) -> Result<Self> {
        ensure(
            map.length_m() == physical_length_m,
            "rail map uses another station domain/length",
        )?;
        Self::from_construction(&map.segments()[0].variant.definition, rail_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WheelCenterPath {
    pub rail: RailReference,
    /// Explicit reference-to-wheel-center offset, T/N/L metres. This is a
    /// declared test locus, not an inferred radius, tread, clearance or contact.
    pub center_offset_m: Vec3,
}
impl WheelCenterPath {
    fn offset(&self) -> Result<Vec3> {
        let result = self.rail.offset_m + self.center_offset_m;
        ensure(
            !self.rail.id.is_empty()
                && self.rail.id.len() <= 128
                && self.rail.offset_m.is_finite()
                && self.center_offset_m.is_finite()
                && self.rail.offset_m.length() <= 1000.
                && self.center_offset_m.length() <= 1000.,
            "invalid rail identity or declared wheel-center offset",
        )?;
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathSample {
    /// Unwrapped station for Circuit queries; physical centerline arc coordinate.
    pub station_m: f64,
    pub rail_center_m: Vec3,
    pub wheel_center_m: Vec3,
    /// d(wheel_center)/d(centerline station), not generally unit length.
    pub derivative: Vec3,
    /// Outgoing tangent at a knot, incoming at the final open end.
    pub tangent: Vec3,
    /// A differing incoming tangent at an exact knot/seam. A continuous frame
    /// can still produce a corner in its offset path.
    pub incoming_tangent: Option<Vec3>,
    pub track_frame: Frame,
}

/// Validated immutable adapter. Construction is O(samples), queries are bounded
/// and opt-in. No wheel queries are installed in simulation or assembly loops.
pub struct PhysicalRailGeometry<'a> {
    track: &'a GeneratedTrack,
    circuit: bool,
}
impl<'a> PhysicalRailGeometry<'a> {
    pub fn new(physical_track: &'a GeneratedTrack, layout: LayoutMode) -> Result<Self> {
        let samples = &physical_track.samples;
        ensure(
            (2..=1_000_000).contains(&samples.len()),
            "rail geometry requires 2..1000000 physical samples",
        )?;
        ensure(
            samples[0].distance_m == 0.,
            "physical rail stations must start at zero",
        )?;
        for s in samples {
            ensure(
                s.distance_m.is_finite() && s.position_m.is_finite(),
                "nonfinite physical rail geometry",
            )?;
        }
        for pair in samples.windows(2) {
            ensure(
                pair[1].distance_m - pair[0].distance_m >= 1e-9,
                "unordered or tiny physical rail interval",
            )?;
            let rotation = pair[0]
                .frame
                .orientation()
                .conjugate()
                .compose(pair[1].frame.orientation())
                .rotation_vector()?
                .length();
            ensure(
                rotation <= 0.75,
                "physical rail frame interval exceeds 0.75 rad; refine samples",
            )?;
        }
        ensure(
            physical_track.connections.iter().all(|c| {
                c.position_gap_m.is_finite()
                    && c.position_gap_m <= topology::POSITION_TOLERANCE_M
                    && c.tangent_angle_rad.is_finite()
                    && c.tangent_angle_rad <= topology::ANGLE_TOLERANCE_RAD
                    && c.orientation_angle_rad.is_finite()
                    && c.orientation_angle_rad <= topology::ANGLE_TOLERANCE_RAD
            }),
            "discontinuous physical track join: rail target unavailable",
        )?;
        let circuit = layout == LayoutMode::Circuit;
        if circuit {
            ensure(
                topology::diagnose(physical_track)?.valid,
                "Circuit rail targets require valid authored closure",
            )?;
        }
        Ok(Self {
            track: physical_track,
            circuit,
        })
    }

    pub fn length_m(&self) -> f64 {
        self.track.length_m()
    }

    pub fn sample(&self, path: &WheelCenterPath, station_m: f64) -> Result<PathSample> {
        let offset = path.offset()?;
        ensure(station_m.is_finite(), "nonfinite rail station")?;
        let station = if self.circuit {
            station_m.rem_euclid(self.length_m())
        } else {
            station_m
        };
        ensure(
            station >= 0. && station <= self.length_m(),
            "rail station outside open/shuttle track",
        )?;
        let i = self
            .track
            .samples
            .partition_point(|s| s.distance_m <= station)
            .saturating_sub(1)
            .min(self.track.samples.len() - 2);
        let p = self.piece(i)?;
        let (position, derivative, frame) = p.evaluate(station, offset)?;
        let rail_center_m = position + frame.local_to_world(path.rail.offset_m);
        let wheel_center_m = position + frame.local_to_world(offset);
        ensure(
            rail_center_m.is_finite() && wheel_center_m.is_finite() && derivative.is_finite(),
            "rail target overflow",
        )?;
        let tangent = derivative.normalized()?;
        let prior = if station == self.track.samples[i].distance_m && i > 0 {
            Some((i - 1, station))
        } else if self.circuit && station == 0. {
            Some((self.track.samples.len() - 2, self.length_m()))
        } else {
            None
        };
        let incoming_tangent = prior
            .map(|(index, s)| self.piece(index)?.evaluate(s, offset)?.1.normalized())
            .transpose()?
            .filter(|incoming| (*incoming - tangent).length() > 1e-6);
        Ok(PathSample {
            station_m,
            rail_center_m,
            wheel_center_m,
            derivative,
            tangent,
            incoming_tangent,
            track_frame: frame,
        })
    }

    fn piece(&self, index: usize) -> Result<Piece> {
        let a = &self.track.samples[index];
        let b = &self.track.samples[index + 1];
        let h = b.distance_m - a.distance_m;
        let t0 = a.frame.tangent() * h;
        let t1 = b.frame.tangent() * h;
        // Difference-based Hermite coefficients avoid cancellation at large origins.
        let delta = b.position_m - a.position_m;
        let c2 = delta * 3. - t0 * 2. - t1;
        let c3 = delta * -2. + t0 + t1;
        let q0 = a.frame.orientation();
        let mut q1 = b.frame.orientation();
        let mut dot = q0.w * q1.w + q0.x * q1.x + q0.y * q1.y + q0.z * q1.z;
        if dot < 0. {
            q1 = Quaternion {
                w: -q1.w,
                x: -q1.x,
                y: -q1.y,
                z: -q1.z,
            };
            dot = -dot;
        }
        let rv = q1.compose(q0.conjugate()).rotation_vector()?;
        Ok(Piece {
            start: a.distance_m,
            h,
            origin: a.position_m,
            t0,
            c2,
            c3,
            q0,
            q1,
            dot: dot.clamp(0., 1.),
            rv,
        })
    }
}

struct Piece {
    start: f64,
    h: f64,
    origin: Vec3,
    t0: Vec3,
    c2: Vec3,
    c3: Vec3,
    q0: Quaternion,
    q1: Quaternion,
    dot: f64,
    rv: Vec3,
}
impl Piece {
    fn evaluate(&self, station: f64, offset: Vec3) -> Result<(Vec3, Vec3, Frame)> {
        let u = ((station - self.start) / self.h).clamp(0., 1.);
        let frame = Frame::new(self.q0.slerp(self.q1, u)?)?;
        let position = self.origin + (self.t0 + (self.c2 + self.c3 * u) * u) * u;
        let dp = (self.t0 + self.c2 * (2. * u) + self.c3 * (3. * u * u)) / self.h;
        // Quaternion::slerp uses normalized lerp for small angles. Its angular
        // speed varies with u; differentiating a constant-rate slerp is wrong.
        let omega = if self.dot > 0.9995 {
            let sin_half = (self.rv.length() * 0.5).sin();
            let angle = self.rv.length();
            let norm2 = 1. - 2. * u * (1. - u) * (1. - self.dot);
            if angle == 0. {
                Vec3::ZERO
            } else {
                self.rv * (2. * sin_half / (angle * self.h * norm2))
            }
        } else {
            self.rv / self.h
        };
        let derivative = dp + omega.cross(frame.local_to_world(offset));
        Ok((position, derivative, frame))
    }

    /// Bound on |P''(s)| over one interpolation piece. Includes frame twist and
    /// the small-angle normalized-lerp angular acceleration. M*ds²/8 bounds
    /// chord deviation in exact arithmetic; the search adds a roundoff margin.
    fn second_derivative_bound(&self, offset: Vec3) -> f64 {
        let position = (self.c2 * 2.)
            .length()
            .max((self.c2 * 2. + self.c3 * 6.).length())
            / (self.h * self.h);
        let (speed, acceleration) = if self.dot > 0.9995 {
            let sin_half = (self.rv.length() * 0.5).sin();
            let minimum_norm2 = (1. + self.dot) * 0.5;
            (
                2. * sin_half / (self.h * minimum_norm2),
                4. * sin_half * (1. - self.dot) / (self.h * self.h * minimum_norm2 * minimum_norm2),
            )
        } else {
            (self.rv.length() / self.h, 0.)
        };
        position + offset.length() * (speed * speed + acceleration)
    }
}
