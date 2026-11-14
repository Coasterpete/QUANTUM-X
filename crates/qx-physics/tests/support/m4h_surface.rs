//! M4H test-only immutable snapshot adapter; original M4F evaluator is retained.
use super::rail_fixtures;
#[allow(dead_code)]
#[path = "m4f_surface.rs"]
mod original;
pub use original::{Failure, Jet, Result, Role, Surface};
use qx_geometry::{
    math::Vec3,
    solver::prepared::{Evaluation, SourceIdentity, dense::ContinuousGraph},
    transition::Side,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ops::Deref,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct Differential {
    pub jet: Jet,
    pub ddd: Vec3,
    pub ddn: Vec3,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StationKey {
    document: u128,
    generation: u64,
    chart: usize,
    station: u64,
    left: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DatumKey {
    station: StationKey,
    rail: &'static str,
    face: &'static str,
    offset: [u64; 3],
    running: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ProjectionKey {
    datum: DatumKey,
    point: [u64; 3],
    hint: u64,
    half: u64,
    circuit: bool,
}
#[derive(Default)]
struct Cache {
    source: BTreeMap<StationKey, Evaluation>,
    datum: BTreeMap<DatumKey, Differential>,
    projection: BTreeMap<ProjectionKey, (f64, Jet)>,
}
// Entries are bounded in EACH map; overflow recomputes instead of approximating.
const CAP: usize = 8192;
pub struct Rail {
    // No DerefMut: source, generation and chart data cannot change under caches.
    original: original::Rail,
    pub reuse: Cell<bool>,
    pub profiling: Cell<bool>,
    cache: RefCell<Cache>,
    /// source queries/hits, datum queries/hits, projections/hits, validations, root iterations.
    pub counts: Cell<[u64; 8]>,
    /// Source incl lookup, datum excl source, search excl datum/validation, interval validation.
    pub costs: Cell<[Duration; 4]>,
}
impl Deref for Rail {
    type Target = original::Rail;
    fn deref(&self) -> &Self::Target {
        &self.original
    }
}
impl Rail {
    pub fn new(source: ContinuousGraph, identity: SourceIdentity) -> Result<Self> {
        Ok(Self {
            original: original::Rail::new(source, identity)?,
            reuse: Cell::new(false),
            profiling: Cell::new(false),
            cache: RefCell::default(),
            counts: Cell::new([0; 8]),
            costs: Cell::new([Duration::ZERO; 4]),
        })
    }
    /// Call before each complete mechanical step or separate diagnostic phase.
    /// Clears results even on a rejected step; there is no hint/state rollback dependency.
    pub fn begin(&self) {
        *self.cache.borrow_mut() = Cache::default();
        self.counts.set([0; 8]);
        self.costs.set([Duration::ZERO; 4]);
    }
    fn count(&self, k: usize) {
        let mut a = self.counts.get();
        a[k] += 1;
        self.counts.set(a);
    }
    fn cost(&self, k: usize, d: Duration) {
        let mut a = self.costs.get();
        a[k] += d;
        self.costs.set(a);
    }
    fn station_key(&self, s: f64) -> Result<StationKey> {
        if !s.is_finite() || s < 0. || s > self.length() {
            return Err(Failure::Invalid);
        }
        let left = s == self.length();
        let segments = self.source.segments();
        let i = segments
            .partition_point(|c| {
                if left {
                    c.end_station_m < s
                } else {
                    c.end_station_m <= s
                }
            })
            .min(segments.len() - 1);
        Ok(StationKey {
            document: self.identity.document,
            generation: self.identity.generation,
            chart: segments[i].chart,
            station: s.to_bits(),
            left,
        })
    }
    fn datum_key(&self, surface: Surface, s: f64) -> Result<DatumKey> {
        if !surface.offset.is_finite()
            || surface.offset.length() > 2.
            || surface.rail.is_empty()
            || surface.face.is_empty()
        {
            return Err(Failure::Invalid);
        }
        Ok(DatumKey {
            station: self.station_key(s)?,
            rail: surface.rail,
            face: surface.face,
            offset: [
                surface.offset.x.to_bits(),
                surface.offset.y.to_bits(),
                surface.offset.z.to_bits(),
            ],
            running: surface.role == Role::Running,
        })
    }
    fn evaluate_source(&self, key: StationKey, s: f64) -> Result<Evaluation> {
        self.count(0);
        let clock = self.profiling.get().then(Instant::now);
        let result = (|| {
            if self.reuse.get()
                && let Some(e) = self.cache.borrow().source.get(&key)
            {
                self.count(1);
                return Ok(e.clone());
            }
            let e = self
                .source
                .evaluate(
                    self.identity,
                    s,
                    if key.left { Side::Left } else { Side::Right },
                )
                .map_err(|_| Failure::Invalid)?;
            if e.source != self.identity || e.chart != key.chart {
                return Err(Failure::Invalid);
            }
            if self.reuse.get() {
                let mut c = self.cache.borrow_mut();
                if c.source.len() < CAP {
                    c.source.insert(key, e.clone());
                }
            }
            Ok(e)
        })();
        if let Some(clock) = clock {
            self.cost(0, clock.elapsed());
        }
        result
    }
    pub fn differential(&self, surface: Surface, s: f64) -> Result<Differential> {
        if !self.reuse.get() && !self.profiling.get() {
            return self
                .original
                .differential(surface, s)
                .map(|d| Differential {
                    jet: d.jet,
                    ddd: d.ddd,
                    ddn: d.ddn,
                });
        }
        self.count(2);
        let clock = self.profiling.get().then(Instant::now);
        let before = self.costs.get()[0];
        let result = (|| {
            let key = self.datum_key(surface, s)?;
            if self.reuse.get()
                && let Some(d) = self.cache.borrow().datum.get(&key)
            {
                self.count(3);
                return Ok(*d);
            }
            let d = self.differential_inner(surface, s, key.station)?;
            if self.reuse.get() {
                let mut c = self.cache.borrow_mut();
                if c.datum.len() < CAP {
                    c.datum.insert(key, d);
                }
            }
            Ok(d)
        })();
        if let Some(clock) = clock {
            self.cost(
                1,
                clock.elapsed().saturating_sub(self.costs.get()[0] - before),
            );
        }
        result
    }
    fn differential_inner(
        &self,
        surface: Surface,
        s: f64,
        key: StationKey,
    ) -> Result<Differential> {
        let e = self.evaluate_source(key, s)?;
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
    fn validate(&self, lo: f64, hi: f64) -> Result<()> {
        self.count(6);
        let clock = self.profiling.get().then(Instant::now);
        let result = self
            .source
            .validate_interval(self.identity, lo, hi)
            .map_err(|_| Failure::Corner);
        if let Some(clock) = clock {
            self.cost(3, clock.elapsed());
        }
        result
    }
    pub fn project(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
        if !self.reuse.get() && !self.profiling.get() {
            return self.original.project(surface, x, hint, half);
        }
        self.count(4);
        let clock = self.profiling.get().then(Instant::now);
        let before = self.costs.get();
        let result = (|| {
            // Validate before a lookup; no cached success can hide a bad query.
            if !x.is_finite() || !hint.is_finite() || !half.is_finite() || half <= 0. || half > 1.6
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
            let datum = self.datum_key(surface, lo)?;
            let key = ProjectionKey {
                datum,
                point: [x.x.to_bits(), x.y.to_bits(), x.z.to_bits()],
                hint: hint.to_bits(),
                half: half.to_bits(),
                circuit: self.circuit,
            };
            if self.reuse.get()
                && let Some(value) = self.cache.borrow().projection.get(&key)
            {
                self.validate(lo, hi)?;
                self.count(5);
                return Ok(*value);
            }
            let value = self.project_inner(surface, x, hint, half)?;
            if self.reuse.get() {
                let mut c = self.cache.borrow_mut();
                if c.projection.len() < CAP {
                    c.projection.insert(key, value);
                }
            }
            Ok(value)
        })();
        if let Some(clock) = clock {
            let now = self.costs.get();
            let nested = (now[0] - before[0]) + (now[1] - before[1]) + (now[3] - before[3]);
            self.cost(2, clock.elapsed().saturating_sub(nested));
        }
        result
    }
    fn project_inner(&self, surface: Surface, x: Vec3, hint: f64, half: f64) -> Result<(f64, Jet)> {
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
        self.validate(lo, hi)?;
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
                    self.count(7);
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
