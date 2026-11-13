//! Development-only physical source datum/normal adapter, not wheel contact.
use super::rail_fixtures;
use qx_geometry::{
    math::Vec3,
    solver::{
        GeneratedTrack,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
#[allow(dead_code)]
#[path = "m4b_surface.rs"]
mod types;
use std::cell::Cell;
pub use types::{Failure, Jet, Result, Role, Surface};

pub struct Differential {
    pub jet: Jet,
    pub ddd: Vec3,
    pub ddn: Vec3,
}
pub struct Rail {
    /// Endpoints of ONE qualified connected C3 interval, solely for unchanged
    /// M4B branch guards. Never sampled to obtain a pose, datum or normal.
    pub track: GeneratedTrack,
    pub circuit: bool,
    pub source: ContinuousGraph,
    pub identity: SourceIdentity,
    pub jets: Cell<usize>,
}
impl Rail {
    pub fn new(source: ContinuousGraph, identity: SourceIdentity) -> Result<Self> {
        source
            .validate_interval(identity, 0., source.length_m())
            .map_err(|_| Failure::Corner)?;
        let track = rail_fixtures::sampled(source.length_m(), source.length_m(), |s| {
            let e = source
                .evaluate(
                    identity,
                    s,
                    if s == source.length_m() {
                        Side::Left
                    } else {
                        Side::Right
                    },
                )
                .unwrap();
            (e.position_m, e.frame.orientation())
        });
        Ok(Self {
            track,
            circuit: false,
            source,
            identity,
            jets: Cell::new(0),
        })
    }
    pub fn length(&self) -> f64 {
        self.source.length_m()
    }
    pub fn differential(&self, surface: Surface, s: f64) -> Result<Differential> {
        self.jets.set(self.jets.get() + 1);
        if !s.is_finite()
            || !surface.offset.is_finite()
            || surface.offset.length() > 2.
            || surface.rail.is_empty()
            || surface.face.is_empty()
        {
            return Err(Failure::Invalid);
        }
        let e = self
            .source
            .evaluate(
                self.identity,
                s,
                if s == self.length() {
                    Side::Left
                } else {
                    Side::Right
                },
            )
            .map_err(|_| Failure::Invalid)?;
        let [p, d, dd, ddd] = e
            .constant_offset(surface.offset)
            .map_err(|_| Failure::Singular)?;
        let [k, dk, _] = e.angular_derivatives;
        let w = if surface.role == Role::Running {
            e.frame.lateral()
        } else {
            e.frame.normal()
        };
        let dw = k.cross(w);
        let ddw = dk.cross(w) + k.cross(dw);
        let cross = |a: Vec3, b: Vec3| {
            if surface.role == Role::Running {
                a.cross(b)
            } else {
                b.cross(a)
            }
        };
        let b = cross(w, d);
        let db = cross(dw, d) + cross(w, dd);
        let ddb = cross(ddw, d) + cross(dw, dd) * 2. + cross(w, ddd);
        let length = b.length();
        if !length.is_finite() || length < 1e-10 {
            return Err(Failure::Singular);
        }
        let n = b / length;
        let dl = n.dot(db);
        let dn = (db - n * dl) / length;
        let ddl = dn.dot(db) + n.dot(ddb);
        let ddn = (ddb - n * ddl - dn * (2. * dl)) / length;
        if !dn.is_finite() || !ddn.is_finite() {
            return Err(Failure::Invalid);
        }
        Ok(Differential {
            jet: Jet {
                p,
                d,
                dd,
                n,
                dn,
                frame: e.frame,
                k,
            },
            ddd,
            ddn,
        })
    }
    pub fn jet(&self, surface: Surface, s: f64) -> Result<Jet> {
        Ok(self.differential(surface, s)?.jet)
    }
    /// Fixed 0.025 m bins, 48 safeguarded Newton iterations per bracket, at most
    /// 128 bins / 2 m half-window. Every competing minimum is rejected, even
    /// at unequal distances. This is bounded enumeration, not root certification.
    pub fn project(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
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
        self.source
            .validate_interval(self.identity, lo, hi)
            .map_err(|_| Failure::Corner)?;
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
