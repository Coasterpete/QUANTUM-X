//! M4C test-only piece-aware projection. Does not smooth or replace authored data.
use std::{cell::Cell, ops::Deref};

#[path = "m4b_surface.rs"]
pub mod reference;
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    solver::GeneratedTrack,
};
pub use reference::{Failure, Jet, Result, Role, Surface};

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub projections: usize,
    pub jets: usize,
    pub brackets: usize,
    pub iterations: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub station: f64,
    pub point: Vec3,
    pub surface: Surface,
    pub f_left: f64,
    pub f_right: f64,
    pub derivative_jump: f64,
    pub normal_jump: f64,
}
struct Piece {
    start: f64,
    h: f64,
    p: Vec3,
    t: Vec3,
    c2: Vec3,
    c3: Vec3,
    q0: Quaternion,
    q1: Quaternion,
    rv: Vec3,
    angle: f64,
    dot: f64,
}
pub struct Rail {
    inner: reference::Rail,
    pieces: Vec<Piece>,
    /// Unchanged M4B projection/jet path for paired numerical/timing controls.
    pub use_reference: bool,
    pub counts: Cell<Counts>,
    pub corner: Cell<Option<Corner>>,
}
impl Deref for Rail {
    type Target = reference::Rail;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl Rail {
    pub fn new(track: GeneratedTrack, circuit: bool) -> Result<Self> {
        let inner = reference::Rail::new(track, circuit)?;
        let pieces = inner
            .track
            .samples
            .windows(2)
            .map(|pair| {
                let (a, b) = (&pair[0], &pair[1]);
                let h = b.distance_m - a.distance_m;
                let t = a.frame.tangent() * h;
                let t1 = b.frame.tangent() * h;
                let delta = b.position_m - a.position_m;
                let q0 = a.frame.orientation();
                let q1 = b.frame.orientation();
                let rv = q1
                    .compose(q0.conjugate())
                    .rotation_vector()
                    .map_err(|_| Failure::Invalid)?;
                let angle = rv.length();
                Ok(Piece {
                    start: a.distance_m,
                    h,
                    p: a.position_m,
                    t,
                    c2: delta * 3. - t * 2. - t1,
                    c3: delta * -2. + t + t1,
                    q0,
                    q1,
                    rv,
                    angle,
                    dot: (angle * 0.5).cos(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            inner,
            pieces,
            use_reference: false,
            counts: Cell::default(),
            corner: Cell::default(),
        })
    }
    pub fn reset_counts(&self) {
        self.counts.set(Counts::default());
        self.corner.set(None);
    }
    pub fn reference_jet(&self, surface: Surface, station: f64) -> Result<Jet> {
        self.inner.jet(surface, station)
    }
    fn locate(&self, station: f64) -> Result<(usize, f64, f64)> {
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
        let i = self
            .track
            .samples
            .partition_point(|p| p.distance_m <= s)
            .saturating_sub(1)
            .min(self.pieces.len() - 1);
        Ok((i, s, station - s))
    }
    fn at(&self, surface: Surface, i: usize, s: f64) -> Result<Jet> {
        let mut counts = self.counts.get();
        counts.jets += 1;
        self.counts.set(counts);
        let p = &self.pieces[i];
        let u = ((s - p.start) / p.h).clamp(0., 1.);
        let (k, dk) = if p.dot > 0.9995 && p.angle > 0. {
            let norm2 = 1. - 2. * u * (1. - u) * (1. - p.dot);
            let k = p.rv * (2. * (p.angle * 0.5).sin() / (p.angle * p.h * norm2));
            (k, k * (2. * (1. - 2. * u) * (1. - p.dot) / (p.h * norm2)))
        } else {
            (p.rv / p.h, Vec3::ZERO)
        };
        let frame = Frame::new(p.q0.slerp(p.q1, u).map_err(|_| Failure::Invalid)?)
            .map_err(|_| Failure::Invalid)?;
        let offset = frame.local_to_world(surface.offset);
        let position = p.p + (p.t + (p.c2 + p.c3 * u) * u) * u + offset;
        let d = (p.t + p.c2 * (2. * u) + p.c3 * (3. * u * u)) / p.h + k.cross(offset);
        let dd = (p.c2 * 2. + p.c3 * (6. * u)) / (p.h * p.h)
            + dk.cross(offset)
            + k.cross(k.cross(offset));
        let width = match surface.role {
            Role::Running => frame.lateral(),
            Role::Side => frame.normal(),
        };
        let dw = k.cross(width);
        let (b, db) = match surface.role {
            Role::Running => (width.cross(d), dw.cross(d) + width.cross(dd)),
            Role::Side => (d.cross(width), dd.cross(width) + d.cross(dw)),
        };
        if b.length() < 1e-8 || d.length() < 1e-8 {
            return Err(Failure::Singular);
        }
        let n = b / b.length();
        Ok(Jet {
            p: position,
            d,
            dd,
            n,
            dn: (db - n * n.dot(db)) / b.length(),
            frame,
            k,
        })
    }
    pub fn jet(&self, surface: Surface, station: f64) -> Result<Jet> {
        if self.use_reference {
            return self.inner.jet(surface, station);
        }
        if !surface.offset.is_finite() || surface.rail.is_empty() || surface.face.is_empty() {
            return Err(Failure::Invalid);
        }
        let (i, s, _) = self.locate(station)?;
        self.at(surface, i, s)
    }
    /// Exact one-sided jets at an authored interpolation boundary; no epsilon
    /// probe that could accidentally select the same side twice.
    pub fn knot(&self, surface: Surface, index: usize, x: Vec3) -> Result<Corner> {
        if index == 0 || index >= self.track.samples.len() - 1 {
            return Err(Failure::Endpoint);
        }
        let s = self.track.samples[index].distance_m;
        self.jump(surface, index - 1, index, s, s, s, x)
    }
    #[allow(clippy::too_many_arguments)]
    fn jump(
        &self,
        surface: Surface,
        left: usize,
        right: usize,
        sl: f64,
        sr: f64,
        station: f64,
        x: Vec3,
    ) -> Result<Corner> {
        let a = self.at(surface, left, sl)?;
        let b = self.at(surface, right, sr)?;
        Ok(Corner {
            station,
            point: x,
            surface,
            f_left: (x - a.p).dot(a.d),
            f_right: (x - b.p).dot(b.d),
            derivative_jump: (a.d - b.d).length(),
            normal_jump: (a.n - b.n).length(),
        })
    }
    /// Enumerates each interpolation piece separately, one bracket per piece.
    /// Safeguarded Newton never traverses a knot. This is bounded sampling, NOT
    /// a certificate that arbitrarily narrow pairs of roots cannot be missed.
    pub fn project(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
        if self.use_reference {
            return self.inner.project(surface, x, hint, half);
        }
        self.corner.set(None);
        let mut counts = self.counts.get();
        counts.projections += 1;
        self.counts.set(counts);
        if !x.is_finite()
            || !hint.is_finite()
            || !half.is_finite()
            || half <= 0.
            || half > 2.
            || !surface.offset.is_finite()
            || surface.rail.is_empty()
            || surface.face.is_empty()
        {
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
        // (station, jet, squared distance, optional nonsmooth witness).
        let mut roots: Vec<(f64, Jet, f64, Option<Corner>)> = vec![];
        let mut a = lo;
        let (mut i, _, mut lap) = self.locate(lo)?;
        for piece_count in 0..2048 {
            let s = a - lap;
            let end = self.track.samples[i + 1].distance_m;
            let b = hi.min(end + lap);
            if b <= a {
                return Err(Failure::Invalid);
            }
            let ja = self.at(surface, i, s)?;
            let jb = self.at(surface, i, b - lap)?;
            let fa = (x - ja.p).dot(ja.d);
            let fb = (x - jb.p).dot(jb.d);
            if fa >= 0. && fb <= 0. {
                let mut counts = self.counts.get();
                counts.brackets += 1;
                self.counts.set(counts);
                let (mut l, mut r) = (a, b);
                let mut t = (hint.clamp(l, r) - l).max(0.);
                t += l;
                let mut solved = None;
                for _ in 0..48 {
                    let mut counts = self.counts.get();
                    counts.iterations += 1;
                    self.counts.set(counts);
                    let j = self.at(surface, i, t - lap)?;
                    let delta = x - j.p;
                    let f = delta.dot(j.d);
                    let d = j.d.dot(j.d) - delta.dot(j.dd);
                    if f.abs() < 2e-13 && d > 1e-8 {
                        solved = Some((t, j, delta.dot(delta), None));
                        break;
                    }
                    if f > 0. {
                        l = t;
                    } else {
                        r = t;
                    }
                    let newton = t + f / d;
                    t = if d > 1e-8 && newton > l && newton < r {
                        newton
                    } else {
                        (l + r) * 0.5
                    };
                }
                let root = solved.ok_or(Failure::NoConvergence)?;
                if roots.iter().all(|r| (r.0 - root.0).abs() > 1e-7) {
                    roots.push(root);
                }
            }
            // A distance minimum can occur at the derivative jump itself, with
            // NO smooth root. Preserve it as a failing candidate, not a fake root.
            if b == end + lap && (i + 1 < self.pieces.len() || self.circuit) {
                let right = (i + 1) % self.pieces.len();
                let sr = if right == 0 { 0. } else { end };
                let witness = self.jump(surface, i, right, end, sr, b, x)?;
                if (witness.derivative_jump > 1e-6 || witness.normal_jump > 1e-6)
                    && witness.f_left >= -2e-13
                    && witness.f_right <= 2e-13
                {
                    roots.retain(|r| (r.0 - b).abs() > 1e-7);
                    roots.push((b, jb, (x - jb.p).dot(x - jb.p), Some(witness)));
                }
            }
            if b == hi {
                break;
            }
            a = b;
            i += 1;
            if i == self.pieces.len() {
                i = 0;
                lap += self.length();
            }
            if piece_count == 2047 {
                return Err(Failure::Invalid);
            }
        }
        roots.sort_by(|a, b| a.2.total_cmp(&b.2));
        let Some(&(s, j, _, corner)) = roots.first() else {
            return Err(if !self.circuit && (lo == 0. || hi == self.length()) {
                Failure::Endpoint
            } else {
                Failure::Inaccessible
            });
        };
        if let Some(witness) = corner {
            self.corner.set(Some(witness));
            return Err(Failure::Corner);
        }
        // Unequal competing minima also cannot authorize a local branch hop.
        if roots.len() > 1 {
            return Err(Failure::Ambiguous);
        }
        if (s - lo).min(hi - s) < 1e-7 {
            return Err(Failure::Inaccessible);
        }
        Ok((s, j))
    }
}
