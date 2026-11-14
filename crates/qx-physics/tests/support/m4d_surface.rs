//! Test-only analytic chart adapter plus preserved sampled M4C control.
//! The analytic chart is a different explicit reference, never a track repair.
use super::{mechanisms::Path, rail_fixtures, reference};
#[path = "m4c_surface.rs"]
pub mod sampled;
use qx_geometry::{math::Vec3, solver::GeneratedTrack};
pub use sampled::{Failure, Jet, Result, Role, Surface};
use std::cell::Cell;

pub struct Rail {
    /// One global analytic chart has only domain endpoints here. This metadata
    /// is used by unchanged M4B curvature's chart guard, NOT for interpolation.
    pub track: GeneratedTrack,
    pub circuit: bool,
    pub path: Option<Path>,
    pub control: Option<sampled::Rail>,
    pub jets: Cell<usize>,
}
impl Rail {
    pub fn analytic(path: Path, length: f64, circuit: bool) -> Result<Self> {
        if !length.is_finite() || length <= 0. || length > 1000. {
            return Err(Failure::Invalid);
        }
        // Only these globally periodic fixtures have a defined smooth seam.
        if circuit
            && (!matches!(path, Path::Turn | Path::Loop)
                || (length - 16. * std::f64::consts::PI).abs() > 1e-12)
        {
            return Err(Failure::Corner);
        }
        Ok(Self {
            track: rail_fixtures::sampled(length, length, |s| {
                let p = path.at(s);
                (p.position_m, p.orientation)
            }),
            circuit,
            path: Some(path),
            control: None,
            jets: Cell::new(0),
        })
    }
    pub fn sampled(track: GeneratedTrack, circuit: bool) -> Result<Self> {
        let control = sampled::Rail::new(track.clone(), circuit)?;
        Ok(Self {
            track,
            circuit,
            path: None,
            control: Some(control),
            jets: Cell::new(0),
        })
    }
    pub fn length(&self) -> f64 {
        self.track.length_m()
    }
    pub fn jet(&self, surface: Surface, station: f64) -> Result<Jet> {
        self.jets.set(self.jets.get() + 1);
        if let Some(r) = &self.control {
            return r.jet(surface, station);
        }
        if !station.is_finite() {
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
        Ok(reference::differential(self.path.unwrap(), surface, s)?.jet)
    }
    /// Fixed 0.025 m bins, 48 safeguarded Newton iterations per bracket, at most
    /// 128 bins / 2 m half-window. Every competing minimum is rejected, even
    /// at unequal distances. This is bounded enumeration, not root certification.
    pub fn project(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
        if let Some(r) = &self.control {
            return r.project(surface, x, hint, half);
        }
        if !x.is_finite() || !hint.is_finite() || !half.is_finite() || half <= 0. || half > 1.6 {
            return Err(Failure::Invalid);
        }
        let (lo, hi) = if self.circuit {
            (hint - half, hint + half)
        } else {
            ((hint - half).max(0.), (hint + half).min(self.length()))
        };
        if hi <= lo {
            return Err(Failure::Inaccessible);
        }
        let bins = ((hi - lo) / 0.025).ceil() as usize;
        if bins == 0 || bins > 128 {
            return Err(Failure::Invalid);
        }
        let mut roots: Vec<(f64, Jet)> = vec![];
        let mut a = lo;
        let mut fa = (x - self.jet(surface, a)?.p).dot(self.jet(surface, a)?.d);
        for i in 1..=bins {
            let b = lo + (hi - lo) * i as f64 / bins as f64;
            let j = self.jet(surface, b)?;
            let fb = (x - j.p).dot(j.d);
            if fa >= 0. && fb <= 0. {
                let (mut l, mut r) = (a, b);
                let mut s = hint.clamp(l, r);
                let mut found = None;
                for _ in 0..48 {
                    let j = self.jet(surface, s)?;
                    let delta = x - j.p;
                    let f = delta.dot(j.d);
                    let d = j.d.dot(j.d) - delta.dot(j.dd);
                    if f.abs() < 2e-13 && d > 1e-8 {
                        found = Some((s, j));
                        break;
                    }
                    if f > 0. {
                        l = s;
                    } else {
                        r = s;
                    }
                    let newton = s + f / d;
                    s = if d > 1e-8 && newton > l && newton < r {
                        newton
                    } else {
                        (l + r) * 0.5
                    };
                }
                let root = found.ok_or(Failure::NoConvergence)?;
                if roots.iter().all(|r| (r.0 - root.0).abs() > 1e-7) {
                    roots.push(root);
                }
            }
            a = b;
            fa = fb;
        }
        if roots.len() > 1 {
            return Err(Failure::Ambiguous);
        }
        let root = roots.first().copied().ok_or(
            if !self.circuit && (lo == 0. || hi == self.length()) {
                Failure::Endpoint
            } else {
                Failure::Inaccessible
            },
        )?;
        if (root.0 - lo).min(hi - root.0) < 1e-7 {
            return Err(Failure::Inaccessible);
        }
        Ok(root)
    }
}

/// Exact one-sided limits from the ACTUAL Hermite/quaternion implementation.
/// Two-sample restrictions preserve each piece's coefficients; no epsilon
/// classification, cross-knot differencing or normal averaging is involved.
pub fn sides(track: &GeneratedTrack, index: usize, surface: Surface) -> Result<(Jet, Jet)> {
    if index == 0 || index >= track.samples.len() - 1 {
        return Err(Failure::Endpoint);
    }
    let piece = |i: usize, end: bool| -> Result<Jet> {
        let a = &track.samples[i];
        let b = &track.samples[i + 1];
        let h = b.distance_m - a.distance_m;
        let pair = rail_fixtures::sampled(h, h, |s| {
            let p = if s == 0. { a } else { b };
            (p.position_m, p.frame.orientation())
        });
        sampled::Rail::new(pair, false)?.jet(surface, if end { h } else { 0. })
    };
    Ok((piece(index - 1, true)?, piece(index, false)?))
}
