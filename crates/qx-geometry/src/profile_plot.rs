//! Read-only scalar graph approximation, separate from evaluation and integration.
//! Each nonzero knot span is bounded before subdivision; no feature search relies
//! on a uniform grid or on midpoint agreement. See docs/m1.1-graph-precision.md.
use crate::{
    Result, ensure,
    profile::NurbsProfile,
    transition::{Function, Transition},
};

/// One chord and the enclosing value range of the underlying curve on its interval.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub bounds: [f64; 2],
    /// False means the caller must show the enclosure, rather than trust the chord.
    pub resolved: bool,
}

/// Bounded-cost output in transition-local input/output units, never screen pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Plot {
    pub segments: Vec<Segment>,
    pub limited: bool,
}

#[derive(Clone)]
struct Span {
    x: [f64; 2],
    degree: usize,
    // Homogeneous Bernstein coefficients (value * weight, weight), degree <= 5.
    h: [[f64; 2]; 6],
    cosine: Option<Cosine>,
    depth: u32,
}
#[derive(Clone, Copy)]
struct Cosine {
    offset: f64,
    scale: f64,
    phase: [f64; 2],
}

impl Span {
    fn endpoints(&self) -> [f64; 2] {
        [0, self.degree].map(|i| {
            self.h[i][0] / self.h[i][1]
                + self.cosine.map_or(0., |c| {
                    c.offset + c.scale * c.phase[usize::from(i != 0)].cos()
                })
        })
    }
    fn bounds(&self) -> [f64; 2] {
        let mut bounds = [f64::INFINITY, f64::NEG_INFINITY];
        for h in &self.h[..=self.degree] {
            include(&mut bounds, [h[0] / h[1]; 2]);
        }
        if let Some(c) = self.cosine {
            let cos = cosine_bounds(c.phase);
            let values = cos.map(|v| c.offset + c.scale * v);
            bounds[0] += values[0].min(values[1]);
            bounds[1] += values[0].max(values[1]);
        }
        // Roundoff guard, not an interval-arithmetic certification. Exact constants
        // retain their useful unit-specific Fit Curve margins.
        if bounds[0] != bounds[1] {
            let pad = bounds[0].abs().max(bounds[1].abs()) * 512. * f64::EPSILON;
            bounds[0] -= pad;
            bounds[1] += pad;
        }
        bounds
    }
    fn chord_error(&self) -> f64 {
        let p = self.degree;
        let a = self.h[0][0] / self.h[0][1];
        let b = self.h[p][0] / self.h[p][1];
        let mut error = 0_f64;
        // N(u) - L(u)W(u) has degree p+1. Its Bernstein coefficients,
        // divided by elevated positive W coefficients, bound y(u)-L(u).
        // Using control abscissae i/p directly would be wrong for rational y(u).
        for j in 0..=p + 1 {
            let t = j as f64 / (p + 1) as f64;
            let left = self.h[j.min(p)];
            let right = self.h[j.saturating_sub(1)];
            let residual = (1. - t) * (left[0] - a * left[1]) + t * (right[0] - b * right[1]);
            let weight = (1. - t) * left[1] + t * right[1];
            error = error.max((residual / weight).abs());
        }
        if let Some(c) = self.cosine {
            // The second-derivative interpolation bound, also capped by the
            // full cosine range. This covers arbitrary legacy cosine phases.
            let width = c.phase[1] - c.phase[0];
            error += c.scale.abs() * (width * width / 8.).min(2.);
        }
        error
    }
    fn split(&self) -> Option<(Self, Self)> {
        let middle = self.x[0] + (self.x[1] - self.x[0]) * 0.5;
        if self.depth >= 48 || middle <= self.x[0] || middle >= self.x[1] {
            return None;
        }
        let mut left = self.clone();
        let mut right = self.clone();
        left.x[1] = middle;
        right.x[0] = middle;
        left.depth += 1;
        right.depth += 1;
        let mut work = self.h;
        for r in 1..=self.degree {
            for j in 0..=self.degree - r {
                work[j] = std::array::from_fn(|c| (work[j][c] + work[j + 1][c]) * 0.5);
            }
            left.h[r] = work[0];
            right.h[self.degree - r] = work[self.degree - r];
        }
        if let Some(c) = self.cosine {
            let mid = c.phase[0] + (c.phase[1] - c.phase[0]) * 0.5;
            left.cosine.as_mut().unwrap().phase[1] = mid;
            right.cosine.as_mut().unwrap().phase[0] = mid;
        }
        Some((left, right))
    }
}

fn include(bounds: &mut [f64; 2], values: [f64; 2]) {
    bounds[0] = bounds[0].min(values[0]);
    bounds[1] = bounds[1].max(values[1]);
}

fn cosine_bounds(phase: [f64; 2]) -> [f64; 2] {
    use std::f64::consts::{PI, TAU};
    // Avoid integer/phase reduction assumptions for extreme imported phases.
    if phase[1] - phase[0] >= TAU || phase.iter().any(|v| v.abs() > 1e12) {
        return [-1., 1.];
    }
    let values = phase.map(f64::cos);
    let mut result = [values[0].min(values[1]), values[0].max(values[1])];
    let first = (phase[0] / PI).ceil();
    let last = (phase[1] / PI).floor();
    for i in 0..3 {
        let k = first + i as f64;
        if k <= last {
            include(&mut result, [if k % 2. == 0. { 1. } else { -1. }; 2]);
        }
    }
    result
}

fn profile_spans(profile: &NurbsProfile, range: [f64; 2]) -> Vec<Span> {
    let p = profile.degree;
    let constant = profile.values.iter().all(|v| *v == profile.values[0]);
    (p..profile.values.len())
        .filter_map(|k| {
            let a = profile.knots[k].max(range[0]);
            let b = profile.knots[k + 1].min(range[1]);
            if b <= a {
                return None;
            }
            let mut h = [[0.; 2]; 6];
            for (i, coefficient) in h.iter_mut().enumerate().take(p + 1) {
                // Blossom on this span: p-i copies of a and i copies of b.
                // All blends stay convex, including at repeated span boundaries.
                let mut d = [[0.; 2]; 6];
                for (j, item) in d.iter_mut().enumerate().take(p + 1) {
                    let index = k - p + j;
                    let w = profile.weights[index];
                    *item = [profile.values[index] * w, w];
                }
                for r in 1..=p {
                    let u = if r <= p - i { a } else { b };
                    for j in (r..=p).rev() {
                        let index = k - p + j;
                        let den = profile.knots[index + p - r + 1] - profile.knots[index];
                        let t = (u - profile.knots[index]) / den;
                        d[j] = std::array::from_fn(|c| (1. - t) * d[j - 1][c] + t * d[j][c]);
                    }
                }
                *coefficient = if constant {
                    [profile.values[0], 1.]
                } else {
                    d[p]
                };
            }
            Some(Span {
                x: [a, b],
                degree: p,
                h,
                cosine: None,
                depth: 0,
            })
        })
        .collect()
}

fn prepare(t: &Transition, range: [f64; 2]) -> Result<Vec<Span>> {
    t.validate()?;
    ensure(
        range.iter().all(|x| x.is_finite())
            && range[0] >= 0.
            && range[1] <= t.length
            && range[0] <= range[1],
        "plot interval must be inside the transition",
    )?;
    if range[0] == range[1] {
        return Ok(vec![]);
    }
    let range = range.map(|x| x / t.length);
    match &t.function {
        Function::Nurbs { profile } => Ok(profile_spans(profile, range)),
        Function::Cosine {} | Function::CosineSpan { .. } => {
            let (offset, scale, phase, polynomial) = match t.function {
                Function::Cosine {} => (
                    t.start + (t.end - t.start) * 0.5,
                    -(t.end - t.start) * 0.5,
                    [0., std::f64::consts::PI],
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
                } => (offset, scale, [phase_start, phase_end], polynomial),
                _ => unreachable!(),
            };
            let mut spans = profile_spans(&NurbsProfile::polynomial(&polynomial, 4), range);
            for span in &mut spans {
                span.cosine = Some(Cosine {
                    offset,
                    scale,
                    phase: span.x.map(|u| phase[0] + (phase[1] - phase[0]) * u),
                });
            }
            Ok(spans)
        }
        _ => {
            // Temporary exact polynomial basis for plotting only. Authored data
            // and the evaluator retain their existing representation/semantics.
            let Function::Nurbs { profile } = t.to_nurbs()?.function else {
                unreachable!()
            };
            Ok(profile_spans(&profile, range))
        }
    }
}

fn refine(spans: Vec<Span>, tolerance: f64, budget: usize, fit: bool) -> Vec<(Span, bool)> {
    let mut pending: Vec<_> = spans.into_iter().rev().collect();
    let mut output = Vec::new();
    while let Some(span) = pending.pop() {
        let error = if fit {
            let bounds = span.bounds();
            let [a, b] = span.endpoints();
            (a.min(b) - bounds[0]).max(bounds[1] - a.max(b))
        } else {
            span.chord_error()
        };
        let resolved = error <= tolerance;
        if !resolved
            && output.len() + pending.len() + 2 <= budget
            && let Some((left, right)) = span.split()
        {
            pending.push(right);
            pending.push(left);
        } else {
            output.push((span, resolved));
        }
    }
    output
}

/// Approximate a visible portion with an absolute vertical error in output units.
/// Positive rational weights make the hull/error bounds valid across whole spans.
/// A small budget returns explicit unresolved enclosures, never silently drops spans.
pub fn sample(
    t: &Transition,
    range: [f64; 2],
    tolerance: f64,
    max_segments: usize,
) -> Result<Plot> {
    ensure(
        tolerance.is_finite() && tolerance > 0.,
        "invalid plot tolerance",
    )?;
    ensure(
        (1..=16384).contains(&max_segments),
        "invalid plot segment budget",
    )?;
    let spans = prepare(t, range)?;
    if spans.len() > max_segments {
        let mut bounds = [f64::INFINITY, f64::NEG_INFINITY];
        for span in spans {
            include(&mut bounds, span.bounds());
        }
        return Ok(Plot {
            segments: vec![Segment {
                from: [range[0], t.evaluate_validated(range[0])?.value],
                to: [range[1], t.evaluate_validated(range[1])?.value],
                bounds,
                resolved: false,
            }],
            limited: true,
        });
    }
    let mut limited = false;
    let segments = refine(spans, tolerance, max_segments, false)
        .into_iter()
        .map(|(s, resolved)| {
            let x = s.x.map(|u| u * t.length);
            let from = [x[0], t.evaluate_validated(x[0])?.value];
            let to = [x[1], t.evaluate_validated(x[1])?.value];
            let endpoints = s.endpoints();
            let endpoint_error = (from[1] - endpoints[0])
                .abs()
                .max((to[1] - endpoints[1]).abs());
            let resolved = resolved && s.chord_error() + endpoint_error <= tolerance;
            limited |= !resolved;
            let mut bounds = s.bounds();
            include(&mut bounds, [from[1].min(to[1]), from[1].max(to[1])]);
            Ok(Segment {
                from,
                to,
                bounds,
                resolved,
            })
        })
        .collect::<Result<_>>()?;
    Ok(Plot { segments, limited })
}

/// Conservative extrema enclosure, tightened independently of the current view.
/// At the budget/precision limit the remaining hull is kept, so fitting can be
/// loose but never intentionally substitutes sampled maxima for an enclosure.
pub fn bounds(t: &Transition) -> Result<[f64; 2]> {
    let spans = prepare(t, [0., t.length])?;
    let mut initial = [f64::INFINITY, f64::NEG_INFINITY];
    for s in &spans {
        include(&mut initial, s.bounds());
    }
    let tolerance = ((initial[1] - initial[0]) * 1e-7)
        .max(initial[0].abs().max(initial[1].abs()) * 1024. * f64::EPSILON)
        .max(1e-12);
    let mut bounds = [f64::INFINITY, f64::NEG_INFINITY];
    for (s, _) in refine(spans, tolerance, 2048, true) {
        include(&mut bounds, s.bounds());
    }
    include(&mut bounds, [t.start.min(t.end), t.start.max(t.end)]);
    Ok(bounds)
}
