//! Adaptive Simpson speed quadrature on each smooth native parameter span.
//! Richardson estimates are diagnostic estimates, not certified error bounds.
use crate::{
    Result,
    curve::{CurveEvaluation, PreparedCurve},
    ensure,
    transition::Side,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcLengthOptions {
    pub absolute_tolerance_m: f64,
    pub relative_tolerance: f64,
    pub max_leaves: usize,
    pub max_depth: u32,
}
impl Default for ArcLengthOptions {
    fn default() -> Self {
        Self {
            absolute_tolerance_m: 1e-8,
            relative_tolerance: 1e-10,
            max_leaves: 65536,
            max_depth: 24,
        }
    }
}
impl ArcLengthOptions {
    pub(crate) fn validate(self) -> Result<()> {
        ensure(
            self.absolute_tolerance_m.is_finite()
                && (1e-12..=0.01).contains(&self.absolute_tolerance_m),
            "invalid arc-length absolute tolerance",
        )?;
        ensure(
            self.relative_tolerance.is_finite()
                && (1e-14..=1e-3).contains(&self.relative_tolerance),
            "invalid arc-length relative tolerance",
        )?;
        ensure(
            (4..=262144).contains(&self.max_leaves) && (4..=32).contains(&self.max_depth),
            "invalid quadrature budget",
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcStation {
    pub parameter: f64,
    pub distance_m: f64,
}
#[derive(Clone, Debug)]
pub struct ArcLengthTable {
    pub stations: Vec<ArcStation>,
    pub estimated_error_m: f64,
    pub evaluations: usize,
    options: ArcLengthOptions,
}
struct Quadrature<'a> {
    curve: &'a PreparedCurve,
    options: ArcLengthOptions,
    leaves: Vec<(f64, f64, f64)>,
    evaluations: usize,
}
impl Quadrature<'_> {
    fn speed(&mut self, u: f64, side: Side) -> Result<f64> {
        self.evaluations += 1;
        let speed = self.curve.evaluate(u, side)?.first.length();
        ensure(speed.is_finite(), "nonfinite curve speed")?;
        Ok(speed)
    }
    fn interval(&mut self, a: f64, b: f64, tol: f64) -> Result<()> {
        let mid = (a + b) * 0.5;
        let values = [
            self.speed(a, Side::Right)?,
            self.speed(mid, Side::Right)?,
            self.speed(b, Side::Left)?,
        ];
        self.refine(a, b, values, tol, 0)
    }
    fn refine(&mut self, a: f64, b: f64, v: [f64; 3], tol: f64, depth: u32) -> Result<()> {
        let mid = (a + b) * 0.5;
        let l = self.speed((a + mid) * 0.5, Side::Right)?;
        let r = self.speed((mid + b) * 0.5, Side::Right)?;
        let whole = (b - a) * (v[0] + 4.0 * v[1] + v[2]) / 6.0;
        let left = (mid - a) * (v[0] + 4.0 * l + v[1]) / 6.0;
        let right = (b - mid) * (v[1] + 4.0 * r + v[2]) / 6.0;
        let error = (left + right - whole).abs() / 15.0;
        if error <= tol + self.options.relative_tolerance * (left + right).abs() {
            ensure(
                self.leaves.len() < self.options.max_leaves,
                "arc-length leaf budget exhausted",
            )?;
            let length = left + right + (left + right - whole) / 15.0;
            ensure(
                length > 0.0 && length.is_finite(),
                "zero-length or degenerate curve span",
            )?;
            self.leaves.push((b, length, error));
            Ok(())
        } else {
            ensure(
                depth < self.options.max_depth && mid > a && mid < b,
                "arc-length quadrature did not converge; relax tolerance or repair curve",
            )?;
            self.refine(a, mid, [v[0], l, v[1]], tol * 0.5, depth + 1)?;
            self.refine(mid, b, [v[1], r, v[2]], tol * 0.5, depth + 1)
        }
    }
}
impl ArcLengthTable {
    pub fn build(curve: &PreparedCurve, options: ArcLengthOptions) -> Result<Self> {
        options.validate()?;
        let mut q = Quadrature {
            curve,
            options,
            leaves: vec![],
            evaluations: 0,
        };
        let (start, end) = curve.domain();
        for pair in curve.breaks().windows(2) {
            q.interval(
                pair[0],
                pair[1],
                options.absolute_tolerance_m * (pair[1] - pair[0]) / (end - start),
            )?;
        }
        let mut stations = vec![ArcStation {
            parameter: start,
            distance_m: 0.0,
        }];
        let mut sum = crate::InputSum::default();
        let mut estimated_error_m = 0.0;
        for (parameter, length, error) in q.leaves {
            let distance_m = sum.add(length);
            ensure(
                distance_m > stations.last().expect("initial station").distance_m,
                "arc-length stations lost precision",
            )?;
            stations.push(ArcStation {
                parameter,
                distance_m,
            });
            estimated_error_m += error;
        }
        Ok(Self {
            stations,
            estimated_error_m,
            evaluations: q.evaluations,
            options,
        })
    }
    pub fn length_m(&self) -> f64 {
        self.stations.last().map_or(0.0, |s| s.distance_m)
    }
    pub fn distance_at_parameter(&self, curve: &PreparedCurve, u: f64) -> Result<f64> {
        let (a, b) = curve.domain();
        ensure(
            u.is_finite() && u >= a && u <= b,
            "arc-length parameter outside domain",
        )?;
        let i = self.stations.partition_point(|s| s.parameter < u);
        if i < self.stations.len() && self.stations[i].parameter == u {
            return Ok(self.stations[i].distance_m);
        }
        let left = &self.stations[i - 1];
        Ok(left.distance_m + self.partial(curve, left.parameter, u)?)
    }
    fn partial(&self, curve: &PreparedCurve, a: f64, b: f64) -> Result<f64> {
        if a == b {
            return Ok(0.0);
        }
        let mut q = Quadrature {
            curve,
            options: self.options,
            leaves: vec![],
            evaluations: 0,
        };
        q.interval(a, b, self.options.absolute_tolerance_m * 0.1)?;
        Ok(q.leaves.iter().map(|l| l.1).sum())
    }
    /// Bracketed Newton/bisection. No division by a near-zero derivative.
    pub fn parameter_at_distance(&self, curve: &PreparedCurve, s: f64) -> Result<f64> {
        ensure(
            s.is_finite() && s >= 0.0 && s <= self.length_m(),
            "distance outside curve",
        )?;
        let i = self.stations.partition_point(|p| p.distance_m < s);
        if i == 0 {
            return Ok(self.stations[0].parameter);
        }
        if self.stations[i].distance_m == s {
            return Ok(self.stations[i].parameter);
        }
        let a = self.stations[i - 1];
        let b = self.stations[i];
        let mut lo = a.parameter;
        let mut hi = b.parameter;
        let mut u = lo + (hi - lo) * (s - a.distance_m) / (b.distance_m - a.distance_m);
        let tolerance = self.options.absolute_tolerance_m * 0.2
            + self.options.relative_tolerance * self.length_m();
        for _ in 0..64 {
            let residual = a.distance_m + self.partial(curve, a.parameter, u)? - s;
            if residual.abs() <= tolerance {
                return Ok(u);
            }
            if residual < 0.0 {
                lo = u;
            } else {
                hi = u;
            }
            let speed = curve.evaluate(u, Side::Right)?.first.length();
            let next = if speed > 1e-12 {
                u - residual / speed
            } else {
                f64::NAN
            };
            u = if next > lo && next < hi {
                next
            } else {
                (lo + hi) * 0.5
            };
        }
        Err(crate::GeometryError(
            "arc-length inversion did not converge".into(),
        ))
    }
    pub fn sample_at_distance(&self, curve: &PreparedCurve, s: f64) -> Result<CurveEvaluation> {
        curve.evaluate(self.parameter_at_distance(curve, s)?, Side::Right)
    }
}
