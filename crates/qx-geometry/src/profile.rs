//! Scalar rational B-splines over normalized station u, never positional track curves.
//! Physical derivatives are obtained by dividing by the region's domain length.
use crate::{
    Result, ensure,
    transition::{Evaluation, Side},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Continuity {
    /// Explicit advanced choice; also preserves unconstrained legacy boundaries.
    Free,
    C0,
    C1,
    #[default]
    C2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NurbsProfile {
    /// New regions are cubic. Degrees four/five preserve legacy polynomial bumps exactly.
    pub degree: usize,
    pub values: Vec<f64>,
    pub knots: Vec<f64>,
    pub weights: Vec<f64>,
    pub incoming: Continuity,
    /// Repeated knots may reduce basis continuity; editing retains this contract.
    #[serde(default)]
    pub interior: Continuity,
}

impl NurbsProfile {
    pub(crate) fn parameter(&self, x: f64, length: f64) -> f64 {
        let u = (x / length).clamp(0., 1.);
        let i = self.knots.partition_point(|k| *k < u);
        for j in [i.saturating_sub(1), i.min(self.knots.len() - 1)] {
            let k = self.knots[j];
            if crate::approximately_equal(u, k, 0., 8. * f64::EPSILON) {
                return k;
            }
        }
        u
    }
    pub fn cubic(start: f64, end: f64) -> Self {
        Self::polynomial(&[start, 0., 3. * (end - start), -2. * (end - start)], 3)
    }
    /// Exact change of basis from power coefficients to a clamped polynomial NURBS.
    pub fn polynomial(coefficients: &[f64], degree: usize) -> Self {
        fn choose(n: usize, k: usize) -> f64 {
            (0..k).fold(1., |v, i| v * (n - i) as f64 / (i + 1) as f64)
        }
        let values = (0..=degree)
            .map(|i| {
                coefficients
                    .iter()
                    .enumerate()
                    .take(i + 1)
                    .map(|(k, a)| a * choose(i, k) / choose(degree, k))
                    .sum()
            })
            .collect();
        Self {
            degree,
            values,
            knots: [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
            weights: vec![1.; degree + 1],
            incoming: Continuity::Free,
            interior: Continuity::C2,
        }
    }
    pub fn validate(&self) -> Result<()> {
        let p = self.degree;
        let n = self.values.len();
        ensure(
            (3..=5).contains(&p) && n > p && n <= 256,
            "profile needs degree 3..5 and at most 256 controls",
        )?;
        ensure(
            self.weights.len() == n && self.knots.len() == n + p + 1,
            "profile control/weight/knot counts disagree",
        )?;
        ensure(
            // Legacy slopes up to 1e12 over lengths up to 1e6 legitimately
            // produce interior controls larger than the endpoint-value bound.
            self.values.iter().all(|v| v.is_finite() && v.abs() <= 1e20),
            "invalid profile control value",
        )?;
        ensure(
            self.weights
                .iter()
                .all(|w| w.is_finite() && (1e-8..=1e8).contains(w)),
            "profile weights must be positive in [1e-8,1e8]",
        )?;
        ensure(
            self.knots
                .iter()
                .all(|k| k.is_finite() && (0.0..=1.0).contains(k))
                && self.knots.windows(2).all(|w| w[0] <= w[1]),
            "profile knots must be finite and ordered in [0,1]",
        )?;
        ensure(
            self.knots[..=p].iter().all(|k| *k == 0.) && self.knots[n..].iter().all(|k| *k == 1.),
            "profile endpoints must be clamped at 0 and 1",
        )?;
        let mut run = 0;
        let mut last = f64::NAN;
        for &k in &self.knots {
            run = if k == last { run + 1 } else { 1 };
            last = k;
            ensure(
                run <= if k == 0. || k == 1. { p + 1 } else { p },
                "invalid profile knot multiplicity",
            )?;
        }
        if self.interior != Continuity::Free {
            let mut previous = 0.;
            for &u in self.knots.iter().filter(|u| **u > 0. && **u < 1.) {
                if u == previous {
                    continue;
                }
                previous = u;
                let a = self.evaluate_validated(u, 1., Side::Left)?;
                let b = self.evaluate_validated(u, 1., Side::Right)?;
                let eq = |x, y| crate::approximately_equal(x, y, 1e-8, 1e-9);
                ensure(
                    eq(a.value, b.value)
                        && (self.interior == Continuity::C0 || eq(a.derivative, b.derivative))
                        && (self.interior != Continuity::C2
                            || eq(a.second_derivative, b.second_derivative)),
                    format!(
                        "internal profile knot {u} violates {:?}: value jump {:.6e}, slope jump {:.6e}, second derivative jump {:.6e}; preserve constraints or explicitly relax internal continuity",
                        self.interior,
                        b.value - a.value,
                        b.derivative - a.derivative,
                        b.second_derivative - a.second_derivative
                    ),
                )?;
            }
        }
        Ok(())
    }
    pub fn evaluate(&self, u: f64, length: f64, side: Side) -> Result<Evaluation> {
        self.validate()?;
        ensure(
            u.is_finite() && (0.0..=1.0).contains(&u) && length.is_finite() && length > 0.,
            "invalid profile coordinate/length",
        )?;
        self.evaluate_validated(u, length, side)
    }
    pub(crate) fn evaluate_validated(&self, u: f64, length: f64, side: Side) -> Result<Evaluation> {
        let [a, b, c] = std::array::from_fn(|order| self.homogeneous(u, order, side));
        let value = a[0] / a[1];
        let first = (b[0] - value * b[1]) / a[1];
        let second = (c[0] - 2. * first * b[1] - value * c[1]) / a[1];
        let e = Evaluation {
            value,
            derivative: first / length,
            second_derivative: second / (length * length),
        };
        ensure(
            [e.value, e.derivative, e.second_derivative]
                .iter()
                .all(|v| v.is_finite()),
            "profile evaluation overflow",
        )?;
        Ok(e)
    }
    /// Higher jets distinguish genuine span boundaries from redundant inserted knots.
    pub(crate) fn derivatives_validated(
        &self,
        u: f64,
        length: f64,
        side: Side,
        order: usize,
    ) -> Vec<f64> {
        let h: Vec<_> = (0..=order)
            .map(|k| {
                if k <= self.degree {
                    self.homogeneous(u, k, side)
                } else {
                    [0.; 2]
                }
            })
            .collect();
        let mut derivatives = vec![0.; order + 1];
        for k in 0..=order {
            let mut numerator = h[k][0];
            let mut choose = 1.;
            for j in 1..=k {
                choose *= (k + 1 - j) as f64 / j as f64;
                numerator -= choose * h[j][1] * derivatives[k - j];
            }
            derivatives[k] = numerator / h[0][1];
        }
        for (k, v) in derivatives.iter_mut().enumerate() {
            *v /= length.powi(k as i32);
        }
        derivatives
    }
    fn control(&self, i: usize, order: usize) -> [f64; 2] {
        if order == 0 {
            let w = self.weights[i];
            return [self.values[i] * w, w];
        }
        let p = self.degree + 1 - order;
        let den = self.knots[i + self.degree + 1] - self.knots[i + order];
        if den == 0. {
            return [0.; 2];
        }
        let a = self.control(i, order - 1);
        let b = self.control(i + 1, order - 1);
        [
            (b[0] - a[0]) * p as f64 / den,
            (b[1] - a[1]) * p as f64 / den,
        ]
    }
    fn homogeneous(&self, u: f64, order: usize, side: Side) -> [f64; 2] {
        let p = self.degree - order;
        let knots = &self.knots[order..self.knots.len() - order];
        let n = self.values.len() - 1 - order;
        let k = if u >= 1. {
            n
        } else if u <= 0. {
            p
        } else {
            let j = match side {
                Side::Left => knots.partition_point(|v| *v < u),
                Side::Right => knots.partition_point(|v| *v <= u),
            };
            (j - 1).clamp(p, n)
        };
        let mut d = [[0.; 2]; 6];
        for (j, item) in d.iter_mut().enumerate().take(p + 1) {
            *item = self.control(k - p + j, order);
        }
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = k - p + j;
                let den = knots[i + p - r + 1] - knots[i];
                let t = if den == 0. { 0. } else { (u - knots[i]) / den };
                d[j] = std::array::from_fn(|c| d[j - 1][c] * (1. - t) + d[j][c] * t);
            }
        }
        d[p]
    }
    /// Boehm insertion in homogeneous coordinates. No resampling or fitting.
    pub fn insert_knot(&mut self, u: f64) -> Result<()> {
        self.validate()?;
        ensure(
            u.is_finite() && u > 0. && u < 1.,
            "insert knot inside (0,1)",
        )?;
        let p = self.degree;
        let s = self.knots.iter().filter(|k| **k == u).count();
        ensure(
            s < p && self.values.len() < 256,
            "profile knot multiplicity/control limit exceeded",
        )?;
        let k = self.knots.partition_point(|v| *v <= u) - 1;
        let h: Vec<_> = (0..self.values.len()).map(|i| self.control(i, 0)).collect();
        let mut q = vec![[0.; 2]; h.len() + 1];
        q[..=k - p].copy_from_slice(&h[..=k - p]);
        q[k - s + 1..].copy_from_slice(&h[k - s..]);
        for i in k - p + 1..=k - s {
            let t = (u - self.knots[i]) / (self.knots[i + p] - self.knots[i]);
            for c in 0..2 {
                q[i][c] = (1. - t) * h[i - 1][c] + t * h[i][c];
            }
        }
        let mut next = self.clone();
        next.values = q.iter().map(|h| h[0] / h[1]).collect();
        next.weights = q.iter().map(|h| h[1]).collect();
        next.knots.insert(k + 1, u);
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn split(&self, u: f64) -> Result<(Self, Self)> {
        self.validate()?;
        ensure(u.is_finite() && u > 0. && u < 1., "split must be interior")?;
        let mut net = self.clone();
        while net.knots.iter().filter(|k| **k == u).count() < net.degree {
            net.insert_knot(u)?;
        }
        let k = net.knots.partition_point(|v| *v <= u) - 1;
        let seam = k - net.degree;
        let mut left = net.clone();
        let mut right = net.clone();
        left.values = net.values[..=seam].to_vec();
        left.weights = net.weights[..=seam].to_vec();
        left.knots = net.knots[..=k].iter().map(|k| k / u).collect();
        left.knots.push(1.);
        right.values = net.values[seam..].to_vec();
        right.weights = net.weights[seam..].to_vec();
        right.knots = vec![0.];
        right.knots.extend(
            net.knots[k - net.degree + 1..]
                .iter()
                .map(|k| (k - u) / (1. - u)),
        );
        // An existing repeated knot may intentionally be only C0/C1. Exact
        // subdivision must preserve that behavior, not invent a C2 contract.
        let a = self.evaluate(u, 1., Side::Left)?;
        let b = self.evaluate(u, 1., Side::Right)?;
        let eq = |x, y| crate::approximately_equal(x, y, 1e-8, 1e-9);
        right.incoming = if !eq(a.value, b.value) {
            Continuity::Free
        } else if !eq(a.derivative, b.derivative) {
            Continuity::C0
        } else if !eq(a.second_derivative, b.second_derivative) {
            Continuity::C1
        } else {
            Continuity::C2
        };
        left.validate()?;
        right.validate()?;
        Ok((left, right))
    }
    /// Add degrees of freedom without changing shape, for local endpoint constraints.
    pub fn refine_for_edit(&mut self) -> Result<()> {
        for u in [0.2, 0.4, 0.6, 0.8] {
            if !self.knots.contains(&u) {
                self.insert_knot(u)?;
            }
        }
        Ok(())
    }
    /// Fix an endpoint jet while preserving the opposite endpoint's first two derivatives.
    /// With separated endpoint control supports this is a triangular linear solve.
    pub fn set_jet(
        &mut self,
        end: bool,
        length: f64,
        target: Evaluation,
        order: usize,
    ) -> Result<()> {
        self.refine_for_edit()?;
        let u = if end { 1. } else { 0. };
        let count = self.values.len();
        for j in 0..=order.min(2) {
            let i = if end { count - 1 - j } else { j };
            let current = self.evaluate(u, length, Side::Right)?;
            let mut basis = self.clone();
            basis.values.fill(0.);
            basis.values[i] = 1.;
            let b = basis.evaluate(u, length, Side::Right)?;
            let select = |e: Evaluation| [e.value, e.derivative, e.second_derivative][j];
            let coefficient = select(b);
            ensure(
                coefficient.is_finite() && coefficient != 0.,
                "singular endpoint constraint",
            )?;
            self.values[i] += (select(target) - select(current)) / coefficient;
        }
        self.validate()
    }
}
