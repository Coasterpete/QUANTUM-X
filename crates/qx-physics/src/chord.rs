//! Initial-condition chord roots, never vehicle motion. See the M2A report for
//! bounds and floating-point qualifications of the first-root search.
use crate::{
    Result,
    assembly_diagnostics::{ChordDiagnostics, ChordTermination},
};
use qx_geometry::{math::Vec3, solver::GeneratedTrack};

const MAX_INTERVALS: u32 = 4096;
const MAX_EVALUATIONS: u32 = 131_072;
const ROOT_STEPS: u32 = 64;

#[derive(Default)]
struct Work {
    intervals: u32,
    evaluations: u32,
    bisections: u32,
    extrema: u32,
    exhausted: bool,
}
impl Work {
    fn value(&mut self, p: &[f64; 7], degree: usize, x: f64) -> Result<f64> {
        if self.evaluations >= MAX_EVALUATIONS {
            self.exhausted = true;
            return Err("chord polynomial evaluation budget exhausted".into());
        }
        self.evaluations += 1;
        let v = p[..=degree]
            .iter()
            .rev()
            .fold(0_f64, |v, c| v.mul_add(x, *c));
        if !v.is_finite() {
            return Err("nonfinite chord polynomial".into());
        }
        Ok(v)
    }

    fn bisect(&mut self, p: &[f64; 7], degree: usize, mut a: f64, mut b: f64) -> Result<f64> {
        let mut fa = self.value(p, degree, a)?;
        for _ in 0..ROOT_STEPS {
            let x = a + (b - a) * 0.5;
            if x == a || x == b || b - a <= 8. * f64::EPSILON {
                return Ok(x);
            }
            self.bisections += 1;
            let fx = self.value(p, degree, x)?;
            if fx == 0. {
                return Ok(x);
            }
            if fx.is_sign_negative() == fa.is_sign_negative() {
                a = x;
                fa = fx;
            } else {
                b = x;
            }
        }
        Err("chord root refinement did not converge".into())
    }

    /// Recursively partition by derivative roots. Between consecutive critical
    /// points a polynomial is monotone: sign brackets contain one crossing, and
    /// a repeated root can occur only at a critical point. Degree is at most 5
    /// here, recursion at most 5, and every bracket has a fixed iteration bound.
    fn zeros(&mut self, p: &[f64; 7], mut degree: usize) -> Result<Vec<f64>> {
        while degree > 0 && p[degree] == 0. {
            degree -= 1;
        }
        if degree == 0 {
            return Ok(Vec::new());
        }
        if degree == 1 {
            let x = -p[0] / p[1];
            return Ok(if x.is_finite() && (0. ..=1.).contains(&x) {
                vec![x]
            } else {
                Vec::new()
            });
        }
        let critical = self.zeros(&derivative(p, degree), degree - 1)?;
        let mut roots = Vec::with_capacity(degree);
        let mut a = 0.;
        let mut fa = self.value(p, degree, a)?;
        // Relative roundoff bound only for derivative-root multiplicities. It
        // never replaces the independent metre residual test on a chord.
        let zero = 64. * f64::EPSILON * p[..=degree].iter().map(|c| c.abs()).sum::<f64>();
        if fa.abs() <= zero {
            roots.push(a);
        }
        for b in critical.into_iter().chain([1.]) {
            let fb = self.value(p, degree, b)?;
            if fa != 0. && fb != 0. && fa.is_sign_negative() != fb.is_sign_negative() {
                roots.push(self.bisect(p, degree, a, b)?);
            }
            if fb.abs() <= zero {
                roots.push(b);
            }
            a = b;
            fa = fb;
        }
        roots.sort_by(f64::total_cmp);
        roots.dedup_by(|a, b| (*a - *b).abs() <= 16. * f64::EPSILON);
        Ok(roots)
    }
}

fn derivative(p: &[f64; 7], degree: usize) -> [f64; 7] {
    let mut d = [0.; 7];
    for i in 1..=degree {
        d[i - 1] = p[i] * i as f64;
    }
    d
}

/// One side of the symmetric chord. Offset keeps circuit stations unwrapped,
/// so ordering is never inferred from wrapped station numbers or proximity.
struct Cursor {
    index: usize,
    offset: f64,
    rear: bool,
}
impl Cursor {
    fn new(track: &GeneratedTrack, center: f64, rear: bool, circuit: bool) -> Self {
        if rear && center == 0. && circuit {
            return Self {
                index: track.samples.len() - 2,
                offset: -track.length_m(),
                rear,
            };
        }
        let i = track.samples.partition_point(|p| {
            if rear {
                p.distance_m < center
            } else {
                p.distance_m <= center
            }
        });
        Self {
            index: i.saturating_sub(1).min(track.samples.len() - 2),
            offset: 0.,
            rear,
        }
    }

    fn boundary(&self, track: &GeneratedTrack, center: f64) -> f64 {
        let i = self.index + usize::from(!self.rear);
        let s = track.samples[i].distance_m + self.offset;
        if self.rear {
            2. * (center - s)
        } else {
            2. * (s - center)
        }
    }

    fn advance(&mut self, track: &GeneratedTrack, circuit: bool) -> Result<()> {
        if self.rear {
            if self.index > 0 {
                self.index -= 1;
            } else if circuit {
                self.index = track.samples.len() - 2;
                self.offset -= track.length_m();
            } else {
                return Err("chord search crossed an open endpoint".into());
            }
        } else if self.index + 2 < track.samples.len() {
            self.index += 1;
        } else if circuit {
            self.index = 0;
            self.offset += track.length_m();
        } else {
            return Err("chord search crossed an open endpoint".into());
        }
        Ok(())
    }

    /// Compose the exact Hermite position polynomial used by
    /// GeneratedTrack::sample_at_distance with station = start + delta*x.
    /// Endpoint differences avoid forming coefficients from large world origins.
    fn polynomial(&self, track: &GeneratedTrack, start: f64, delta: f64) -> Result<[Vec3; 4]> {
        let a = &track.samples[self.index];
        let b = &track.samples[self.index + 1];
        let h = b.distance_m - a.distance_m;
        if !h.is_finite() || h <= 0. || !a.position_m.is_finite() || !b.position_m.is_finite() {
            return Err("invalid generated centerline interval for chord search".into());
        }
        let c1 = a.frame.tangent() * h;
        let c2 =
            (b.position_m - a.position_m) * 3. - (a.frame.tangent() * 2. + b.frame.tangent()) * h;
        let c3 = (a.position_m - b.position_m) * 2. + (a.frame.tangent() + b.frame.tangent()) * h;
        let u = ((start - self.offset) - a.distance_m) / h;
        let k = delta / h;
        let c = [
            a.position_m + (c1 + (c2 + c3 * u) * u) * u,
            (c1 + c2 * (2. * u) + c3 * (3. * u * u)) * k,
            (c2 + c3 * (3. * u)) * (k * k),
            c3 * (k * k * k),
        ];
        if c.iter().any(|v| !v.is_finite()) {
            return Err("chord centerline polynomial overflow".into());
        }
        Ok(c)
    }
}

pub(crate) fn solve<const TRACE: bool>(
    track: &GeneratedTrack,
    center: f64,
    length: f64,
    circuit: bool,
    diagnostics: &mut ChordDiagnostics,
) -> Result<(f64, f64)> {
    let mut work = Work::default();
    if TRACE {
        diagnostics.center_m = center;
        diagnostics.attachment_distance_m = length;
        diagnostics.circuit = circuit;
        diagnostics.termination = ChordTermination::TrackQueryFailed;
    }
    let result = search::<TRACE>(track, center, length, circuit, &mut work, diagnostics);
    if TRACE {
        diagnostics.bracket_probes = work.intervals;
        diagnostics.bisection_iterations = work.bisections;
        diagnostics.polynomial_evaluations = work.evaluations;
        diagnostics.extrema_tested = work.extrema;
        if work.exhausted {
            diagnostics.termination = ChordTermination::SearchBudgetExceeded;
        }
    }
    result
}

fn search<const TRACE: bool>(
    track: &GeneratedTrack,
    center: f64,
    length: f64,
    circuit: bool,
    work: &mut Work,
    diagnostics: &mut ChordDiagnostics,
) -> Result<(f64, f64)> {
    if !length.is_finite() || length < 1e-8 || !(4. * length).is_finite() {
        if TRACE {
            diagnostics.termination = ChordTermination::InvalidLength;
        }
        return Err("rigid chord length must be finite, at least 1e-8 m, and not overflow".into());
    }
    let total = track.length_m();
    if track.samples.len() < 2
        || !total.is_finite()
        || total <= 0.
        || track.samples[0].distance_m != 0.
        || !center.is_finite()
        || !(0. ..=total).contains(&center)
    {
        return Err("chord center outside a nondegenerate generated track".into());
    }
    let center = if circuit && center == total {
        0.
    } else {
        center
    };
    let max = (4. * length).min(if circuit {
        total
    } else {
        2. * center.min(total - center)
    });
    let mut rear = Cursor::new(track, center, true, circuit);
    let mut front = Cursor::new(track, center, false, circuit);
    let origin = track
        .sample_at_distance(center)
        .map_err(|e| e.to_string())?
        .position_m;
    if !origin.is_finite() {
        return Err("nonfinite chord centerline position".into());
    }
    // Roundoff allowance, not mechanical slack. Refuse coordinate scales at
    // which a sub-nanometre chord residual cannot be resolved in f64.
    let tolerance = 8. * f64::EPSILON * (center.abs() + length + origin.length()).max(1.);
    if !tolerance.is_finite() || tolerance > 1e-9 {
        if TRACE {
            diagnostics.termination = ChordTermination::ResidualNotMet;
        }
        return Err("chord coordinate scale exceeds the 1e-9 m precision budget".into());
    }
    if TRACE {
        diagnostics.max_span_m = max;
        diagnostics.residual_tolerance_m = tolerance;
    }
    let mut lo = 0.;
    while lo < max {
        if work.intervals >= MAX_INTERVALS {
            work.exhausted = true;
            return Err("chord interval budget exhausted".into());
        }
        work.intervals += 1;
        let rb = rear.boundary(track, center);
        let fb = front.boundary(track, center);
        let hi = rb.min(fb).min(max);
        if !hi.is_finite() || hi < lo {
            return Err("unordered generated centerline intervals in chord search".into());
        }
        if hi > lo {
            let a = rear.polynomial(track, center - lo * 0.5, -(hi - lo) * 0.5)?;
            let b = front.polynomial(track, center + lo * 0.5, (hi - lo) * 0.5)?;
            let d: [Vec3; 4] = std::array::from_fn(|i| (b[i] - a[i]) / length);
            if d.iter().any(|v| !v.is_finite()) {
                return Err("chord difference polynomial overflow".into());
            }
            // Cubic Bezier convex hull gives a cheap upper distance bound.
            // Most ordinary intervals are provably short of the rigid length.
            let hull = [
                d[0],
                d[0] + d[1] / 3.,
                d[0] + d[1] * (2. / 3.) + d[2] / 3.,
                d.iter().copied().fold(Vec3::ZERO, |a, b| a + b),
            ];
            if hull.iter().any(|v| !v.is_finite()) {
                return Err("chord interval bound overflow".into());
            }
            let bound = hull.iter().map(|v| v.length()).fold(0_f64, f64::max);
            if !bound.is_finite() {
                return Err("chord distance polynomial overflow".into());
            }
            if bound >= 1. - tolerance / length {
                let mut p = [0.; 7];
                for i in 0..4 {
                    for j in 0..4 {
                        p[i + j] += d[i].dot(d[j]);
                    }
                }
                p[0] -= 1.;
                let critical = work.zeros(&derivative(&p, 6), 5)?;
                let mut x0 = 0.;
                let mut f0 = work.value(&p, 6, x0)?;
                let ftol = (2. + tolerance / length) * tolerance / length;
                for x1 in critical
                    .into_iter()
                    .filter(|x| *x > 0. && *x < 1.)
                    .chain([1.])
                {
                    work.extrema += u32::from(x1 < 1.);
                    let f1 = work.value(&p, 6, x1)?;
                    let candidate = if f0.abs() <= ftol {
                        Some(x0)
                    } else if f0.is_sign_negative() != f1.is_sign_negative() {
                        Some(work.bisect(&p, 6, x0, x1)?)
                    } else if f1.abs() <= ftol {
                        Some(x1)
                    } else {
                        None
                    };
                    if let Some(x) = candidate {
                        let span = lo + (hi - lo) * x;
                        let stations = (center - span * 0.5, center + span * 0.5);
                        // Only endpoint arithmetic within the floating-point
                        // allowance may snap; meaningful discrepancies fail.
                        let normalize = |s: f64| -> Result<f64> {
                            if circuit {
                                return Ok(s.rem_euclid(total));
                            }
                            if s < -tolerance || s > total + tolerance {
                                return Err("chord root outside open track".into());
                            }
                            Ok(s.clamp(0., total))
                        };
                        let pa = track
                            .sample_at_distance(normalize(stations.0)?)
                            .map_err(|e| e.to_string())?
                            .position_m;
                        let pb = track
                            .sample_at_distance(normalize(stations.1)?)
                            .map_err(|e| e.to_string())?
                            .position_m;
                        let residual = (pb - pa).length() - length;
                        if TRACE {
                            diagnostics.bracket_span_m =
                                Some([lo + (hi - lo) * x0, lo + (hi - lo) * x1]);
                            diagnostics.residual_m = Some(residual);
                        }
                        if !residual.is_finite() || residual.abs() > tolerance {
                            if TRACE {
                                diagnostics.termination = ChordTermination::ResidualNotMet;
                            }
                            return Err(format!(
                                "chord residual {residual:e} m exceeds roundoff allowance {tolerance:e} m"
                            ));
                        }
                        let stations = if circuit {
                            stations
                        } else {
                            (normalize(stations.0)?, normalize(stations.1)?)
                        };
                        if TRACE {
                            diagnostics.stations_m = Some(stations);
                            diagnostics.termination = ChordTermination::Converged;
                        }
                        return Ok(stations);
                    }
                    x0 = x1;
                    f0 = f1;
                }
            }
        }
        if hi >= max {
            break;
        }
        if rb <= hi {
            rear.advance(track, circuit)?;
        }
        if fb <= hi {
            front.advance(track, circuit)?;
        }
        lo = hi;
    }
    if TRACE {
        diagnostics.termination = ChordTermination::NoBracket;
    }
    Err(format!(
        "no local rigid chord of {length:.6} m at station {center:.6} within span {max:.6} m; curvature or endpoint is incompatible"
    ))
}
