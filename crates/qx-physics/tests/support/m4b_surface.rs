//! Synthetic swept-datum normal coordinates. Test-only, SI/f64. The zero-gap
//! chart is an ideal bilateral guide, not exact tread/rail contact distance.
use qx_geometry::{
    math::{Frame, Vec3},
    rail_geometry::PhysicalRailGeometry,
    solver::GeneratedTrack,
    topology::LayoutMode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Invalid,
    Endpoint,
    Inaccessible,
    Ambiguous,
    Corner,
    Singular,
    BearingLimit,
    NoConvergence,
    Residual,
}
pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Running,
    Side,
}
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub rail: &'static str,
    pub face: &'static str,
    /// Origin on an authored swept plane, not a mesh-derived wheel center.
    pub offset: Vec3,
    pub role: Role,
}
#[derive(Clone, Copy, Debug)]
pub struct Jet {
    pub p: Vec3,
    pub d: Vec3,
    pub dd: Vec3,
    pub n: Vec3,
    pub dn: Vec3,
    pub frame: Frame,
    pub k: Vec3,
}
pub struct Rail {
    pub track: GeneratedTrack,
    pub circuit: bool,
}
impl Rail {
    pub fn new(track: GeneratedTrack, circuit: bool) -> Result<Self> {
        PhysicalRailGeometry::new(
            &track,
            if circuit {
                LayoutMode::Circuit
            } else {
                LayoutMode::Open
            },
        )
        .map_err(|_| Failure::Invalid)?;
        if circuit {
            let first = &track.samples[0];
            let last = track.samples.last().unwrap();
            let angle = first
                .frame
                .orientation()
                .conjugate()
                .compose(last.frame.orientation())
                .rotation_vector()
                .map_err(|_| Failure::Invalid)?
                .length();
            if (first.position_m - last.position_m).length() > 1e-9 || angle > 1e-9 {
                return Err(Failure::Corner);
            }
        }
        Ok(Self { track, circuit })
    }
    pub fn length(&self) -> f64 {
        self.track.length_m()
    }
    /// Exact Hermite + shortest quaternion interpolation jet, including the
    /// normalized-lerp branch used by Frame/PhysicalRailGeometry.
    pub fn jet(&self, surface: Surface, station: f64) -> Result<Jet> {
        if !station.is_finite()
            || !surface.offset.is_finite()
            || surface.rail.is_empty()
            || surface.face.is_empty()
        {
            return Err(Failure::Invalid);
        }
        let s = if self.circuit {
            station.rem_euclid(self.length())
        } else {
            station
        };
        if s < 0. || s > self.length() {
            return Err(Failure::Endpoint);
        }
        let i = self
            .track
            .samples
            .partition_point(|p| p.distance_m <= s)
            .saturating_sub(1)
            .min(self.track.samples.len() - 2);
        let a = &self.track.samples[i];
        let b = &self.track.samples[i + 1];
        let h = b.distance_m - a.distance_m;
        let u = (s - a.distance_m) / h;
        let t0 = a.frame.tangent() * h;
        let t1 = b.frame.tangent() * h;
        let delta = b.position_m - a.position_m;
        let c2 = delta * 3. - t0 * 2. - t1;
        let c3 = delta * -2. + t0 + t1;
        let q0 = a.frame.orientation();
        let q1 = b.frame.orientation();
        let rv = q1
            .compose(q0.conjugate())
            .rotation_vector()
            .map_err(|_| Failure::Invalid)?;
        let angle = rv.length();
        let dot = (angle * 0.5).cos();
        let (k, dk) = if dot > 0.9995 && angle > 0. {
            let norm2 = 1. - 2. * u * (1. - u) * (1. - dot);
            let k = rv * (2. * (angle * 0.5).sin() / (angle * h * norm2));
            (k, k * (2. * (1. - 2. * u) * (1. - dot) / (h * norm2)))
        } else {
            (rv / h, Vec3::ZERO)
        };
        let frame = Frame::new(q0.slerp(q1, u).map_err(|_| Failure::Invalid)?)
            .map_err(|_| Failure::Invalid)?;
        let offset = frame.local_to_world(surface.offset);
        let p = a.position_m + (t0 + (c2 + c3 * u) * u) * u + offset;
        let d = (t0 + c2 * (2. * u) + c3 * (3. * u * u)) / h + k.cross(offset);
        let dd = (c2 * 2. + c3 * (6. * u)) / (h * h) + dk.cross(offset) + k.cross(k.cross(offset));
        // The normal belongs to the swept surface, not simply R*Y or R*Z.
        let width = match surface.role {
            Role::Running => frame.lateral(),
            Role::Side => frame.normal(),
        };
        let dw = k.cross(width);
        let (cross, dcross) = match surface.role {
            Role::Running => (width.cross(d), dw.cross(d) + width.cross(dd)),
            Role::Side => (d.cross(width), dd.cross(width) + d.cross(dw)),
        };
        if cross.length() < 1e-8 || d.length() < 1e-8 {
            return Err(Failure::Singular);
        }
        let n = cross / cross.length();
        let dn = (dcross - n * n.dot(dcross)) / cross.length();
        Ok(Jet {
            p,
            d,
            dd,
            n,
            dn,
            frame,
            k,
        })
    }
    /// Bounded LOCAL branch search; no global nearest-rail claim. The root
    /// enumeration is sampled (64 bins), then bisected, with explicit ambiguity.
    pub fn project(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
        if !x.is_finite() || !hint.is_finite() || !half.is_finite() || half <= 0. || half > 2. {
            return Err(Failure::Invalid);
        }
        let lo = if self.circuit {
            hint - half
        } else {
            (hint - half).max(0.)
        };
        let hi = if self.circuit {
            hint + half
        } else {
            (hint + half).min(self.length())
        };
        if hi <= lo {
            return Err(Failure::Inaccessible);
        }
        let f = |s| -> Result<f64> {
            let j = self.jet(surface, s)?;
            Ok((x - j.p).dot(j.d))
        };
        let mut roots: Vec<(f64, Jet, f64)> = vec![];
        let mut a = lo;
        let mut fa = f(a)?;
        for i in 1..=64 {
            let b = lo + (hi - lo) * i as f64 / 64.;
            let fb = f(b)?;
            if fa >= 0. && fb <= 0. {
                let (mut l, mut r) = (a, b);
                for _ in 0..40 {
                    let mid = (l + r) * 0.5;
                    if f(mid)? > 0. {
                        l = mid;
                    } else {
                        r = mid;
                    }
                }
                let s = (l + r) * 0.5;
                let j = self.jet(surface, s)?;
                let delta = x - j.p;
                if delta.dot(j.d).abs() > 1e-9 {
                    return Err(Failure::Corner);
                }
                if j.d.dot(j.d) - delta.dot(j.dd) <= 1e-8 {
                    return Err(Failure::Singular);
                }
                // Never differentiate across a knot with different one-sided jets.
                let left = self.jet(surface, s - 1e-8);
                let right = self.jet(surface, s + 1e-8);
                if let (Ok(l), Ok(r)) = (left, right)
                    && ((l.d - r.d).length() > 1e-6 || (l.n - r.n).length() > 1e-6)
                {
                    return Err(Failure::Corner);
                }
                if roots.iter().all(|r| (r.0 - s).abs() > 1e-7) {
                    roots.push((s, j, delta.dot(delta)));
                }
            }
            a = b;
            fa = fb;
        }
        roots.sort_by(|a, b| a.2.total_cmp(&b.2));
        let Some(&(s, j, d2)) = roots.first() else {
            return Err(if !self.circuit && (lo == 0. || hi == self.length()) {
                Failure::Endpoint
            } else {
                Failure::Inaccessible
            });
        };
        if roots.iter().skip(1).any(|r| (r.2 - d2).abs() < 1e-10) {
            return Err(Failure::Ambiguous);
        }
        if (s - lo).min(hi - s) < 1e-7 {
            return Err(Failure::Inaccessible);
        }
        Ok((s, j))
    }
}
