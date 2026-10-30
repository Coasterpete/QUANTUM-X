//! Channels use seconds/metres as input, declared by their section.
//! Slopes use output units per input unit; amplitude adds an endpoint-flat bump.
use crate::{Result, approximately_equal, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    G,
    RadiansPerSecond,
    RadiansPerMetre,
    RadiansPerNormalized,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Function {
    Constant {},
    Linear {},
    Quadratic {},
    CubicSmoothstep {},
    QuinticSmoothstep {},
    Cosine {},
    Hermite {
        start_slope: f64,
        end_slope: f64,
    },
    Nurbs {
        profile: crate::profile::NurbsProfile,
    },
    /// Exact legacy cosine restriction; cosine is not a rational polynomial.
    CosineSpan {
        offset: f64,
        scale: f64,
        phase_start: f64,
        phase_end: f64,
        polynomial: [f64; 5],
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub id: u64,
    pub length: f64,
    pub start: f64,
    pub end: f64,
    /// Output-unit amplitude of 16u²(1-u)², whose peak is one.
    pub amplitude: f64,
    pub function: Function,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evaluation {
    pub value: f64,
    pub derivative: f64,
    pub second_derivative: f64,
}
impl Transition {
    pub fn constant(id: u64, length: f64, value: f64) -> Self {
        Self {
            id,
            length,
            start: value,
            end: value,
            amplitude: 0.0,
            function: Function::Constant {},
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure(self.id > 0, "transition ID must be nonzero")?;
        ensure(
            self.length.is_finite() && (1e-8..=1e6).contains(&self.length),
            "transition length must be in [1e-8,1e6]",
        )?;
        for v in [self.start, self.end, self.amplitude] {
            ensure(
                v.is_finite() && v.abs() <= 1e12,
                "transition values must be finite and bounded by 1e12",
            )?;
        }
        if let Function::Hermite {
            start_slope,
            end_slope,
        } = self.function
        {
            ensure(
                start_slope.is_finite()
                    && end_slope.is_finite()
                    && start_slope.abs() <= 1e12
                    && end_slope.abs() <= 1e12,
                "invalid Hermite slopes",
            )?;
        }
        if matches!(self.function, Function::Constant {}) {
            ensure(
                self.start == self.end && self.amplitude == 0.0,
                "constant transition requires equal endpoints and zero amplitude",
            )?;
        }
        match &self.function {
            Function::Nurbs { profile } => {
                profile.validate()?;
                ensure(
                    self.amplitude == 0.
                        && self.start == profile.values[0]
                        && self.end == *profile.values.last().unwrap(),
                    "NURBS endpoint mirrors/amplitude are inconsistent; use profile editing commands",
                )?;
            }
            Function::CosineSpan {
                offset,
                scale,
                phase_start,
                phase_end,
                polynomial,
            } => {
                ensure(
                    [*offset, *scale, *phase_start, *phase_end]
                        .iter()
                        .chain(polynomial)
                        .all(|v| v.is_finite() && v.abs() <= 1e20)
                        && phase_end > phase_start
                        && self.amplitude == 0.,
                    "invalid legacy cosine span",
                )?;
                let a = self.evaluate_validated(0.)?;
                let b = self.evaluate_validated(self.length)?;
                ensure(
                    approximately_equal(self.start, a.value, 1e-10, 1e-12)
                        && approximately_equal(self.end, b.value, 1e-10, 1e-12),
                    "legacy cosine endpoint mirrors are inconsistent",
                )?;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn evaluate(&self, x: f64) -> Result<Evaluation> {
        self.validate()?;
        ensure(
            x.is_finite() && (0.0..=self.length).contains(&x),
            "transition coordinate out of range",
        )?;
        self.evaluate_validated(x)
    }
    pub(crate) fn evaluate_validated(&self, x: f64) -> Result<Evaluation> {
        let u = (x / self.length).clamp(0.0, 1.0);
        if let Function::Nurbs { profile } = &self.function {
            return profile.evaluate_validated(
                profile.parameter(x, self.length),
                self.length,
                Side::Right,
            );
        }
        let d = self.end - self.start;
        let (value, derivative) = match &self.function {
            Function::Constant {} => (self.start, 0.0),
            Function::Linear {} => (self.start + d * u, d / self.length),
            Function::Quadratic {} => (self.start + d * u * u, 2.0 * d * u / self.length),
            Function::CubicSmoothstep {} => (
                self.start + d * u * u * (3.0 - 2.0 * u),
                d * 6.0 * u * (1.0 - u) / self.length,
            ),
            Function::QuinticSmoothstep {} => (
                self.start + d * u * u * u * (10.0 + u * (-15.0 + 6.0 * u)),
                d * 30.0 * u * u * (1.0 - u) * (1.0 - u) / self.length,
            ),
            Function::Cosine {} => {
                let a = std::f64::consts::PI * u;
                (
                    self.start + d * (1.0 - a.cos()) * 0.5,
                    d * std::f64::consts::PI * a.sin() * 0.5 / self.length,
                )
            }
            Function::Hermite {
                start_slope: a,
                end_slope: b,
            } => {
                let h = self.length;
                let v = (2.0 * u * u * u - 3.0 * u * u + 1.0) * self.start
                    + (u * u * u - 2.0 * u * u + u) * h * a
                    + (-2.0 * u * u * u + 3.0 * u * u) * self.end
                    + (u * u * u - u * u) * h * b;
                let dv = ((6.0 * u * u - 6.0 * u) * self.start
                    + (3.0 * u * u - 4.0 * u + 1.0) * h * a
                    + (-6.0 * u * u + 6.0 * u) * self.end
                    + (3.0 * u * u - 2.0 * u) * h * b)
                    / h;
                (v, dv)
            }
            Function::CosineSpan {
                offset,
                scale,
                phase_start,
                phase_end,
                polynomial: p,
            } => {
                let rate = phase_end - phase_start;
                let phase = phase_start + rate * u;
                let v = p.iter().rev().fold(0., |s, c| s * u + c);
                let dv = (1..5).rev().fold(0., |s, i| s * u + i as f64 * p[i]);
                (
                    offset + scale * phase.cos() + v,
                    (-scale * rate * phase.sin() + dv) / self.length,
                )
            }
            Function::Nurbs { .. } => unreachable!("handled above"),
        };
        let value = value + self.amplitude * 16.0 * u * u * (1.0 - u) * (1.0 - u);
        let derivative =
            derivative + self.amplitude * 32.0 * u * (1.0 - u) * (1.0 - 2.0 * u) / self.length;
        let second = match &self.function {
            Function::Constant {} | Function::Linear {} => 0.,
            Function::Quadratic {} => 2. * d,
            Function::CubicSmoothstep {} => 6. * d * (1. - 2. * u),
            Function::QuinticSmoothstep {} => 60. * d * u * (1. - u) * (1. - 2. * u),
            Function::Cosine {} => {
                0.5 * d * std::f64::consts::PI.powi(2) * (std::f64::consts::PI * u).cos()
            }
            Function::Hermite {
                start_slope: a,
                end_slope: b,
            } => {
                (12. * u - 6.) * self.start
                    + (6. * u - 4.) * self.length * a
                    + (-12. * u + 6.) * self.end
                    + (6. * u - 2.) * self.length * b
            }
            Function::CosineSpan {
                scale,
                phase_start,
                phase_end,
                polynomial: p,
                ..
            } => {
                let rate = phase_end - phase_start;
                -scale * rate * rate * (phase_start + rate * u).cos()
                    + (2..5)
                        .rev()
                        .fold(0., |s, i| s * u + (i * (i - 1)) as f64 * p[i])
            }
            Function::Nurbs { .. } => unreachable!(),
        };
        let second_derivative = (second + self.amplitude * 32. * (1. - 6. * u + 6. * u * u))
            / (self.length * self.length);
        ensure(
            value.is_finite() && derivative.is_finite() && second_derivative.is_finite(),
            "transition evaluation overflow",
        )?;
        Ok(Evaluation {
            value,
            derivative,
            second_derivative,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub unit: Unit,
    pub transitions: Vec<Transition>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuityIssue {
    pub input: f64,
    pub value_jump: f64,
    pub derivative_jump: f64,
    pub second_derivative_jump: f64,
}
impl Channel {
    pub fn constant(unit: Unit, length: f64, value: f64) -> Self {
        Self {
            unit,
            transitions: vec![Transition::constant(1, length, value)],
        }
    }
    pub fn length(&self) -> f64 {
        let mut sum = crate::InputSum::default();
        self.transitions.iter().fold(0.0, |_, t| sum.add(t.length))
    }
    pub fn validate(&self) -> Result<()> {
        ensure(
            !self.transitions.is_empty() && self.transitions.len() <= 4096,
            "channel must contain 1..4096 transitions",
        )?;
        let mut ids = std::collections::HashSet::new();
        let mut cursor = 0.0;
        let mut sum = crate::InputSum::default();
        for t in &self.transitions {
            t.validate()?;
            ensure(ids.insert(t.id), "duplicate transition ID in channel")?;
            let next = sum.add(t.length);
            ensure(
                next > cursor && next <= 1e6,
                "channel length exceeds limit or loses precision",
            )?;
            cursor = next;
        }
        for pair in self.transitions.windows(2) {
            use crate::profile::Continuity;
            let rule = pair[1].incoming_continuity();
            if rule == Continuity::Free {
                continue;
            }
            let a = pair[0].evaluate_validated(pair[0].length)?;
            let b = pair[1].evaluate_validated(0.)?;
            let equal = |a, b| approximately_equal(a, b, 1e-8, 1e-9);
            ensure(
                equal(a.value, b.value)
                    && (rule == Continuity::C0 || equal(a.derivative, b.derivative))
                    && (rule != Continuity::C2 || equal(a.second_derivative, b.second_derivative)),
                format!(
                    "regions {} / {} violate {rule:?}: value jump {:.12e}, derivative jump {:.12e}, second derivative jump {:.12e}; edit the shared boundary or explicitly relax continuity in advanced controls",
                    pair[0].id,
                    pair[1].id,
                    b.value - a.value,
                    b.derivative - a.derivative,
                    b.second_derivative - a.second_derivative
                ),
            )?;
        }
        Ok(())
    }
    /// Right-continuous internally; the final endpoint uses the final transition.
    pub fn evaluate(&self, x: f64) -> Result<Evaluation> {
        self.validate()?;
        ensure(
            x.is_finite() && (0.0..=self.length()).contains(&x),
            "channel coordinate out of range",
        )?;
        self.evaluate_side(x, Side::Right)
    }
    pub(crate) fn evaluate_side(&self, x: f64, side: Side) -> Result<Evaluation> {
        let mut start = 0.0;
        let mut sum = crate::InputSum::default();
        for (i, t) in self.transitions.iter().enumerate() {
            let end = sum.add(t.length);
            let at_end = crate::same_input(x, end);
            if (x < end && !at_end)
                || (at_end && side == Side::Left)
                || i + 1 == self.transitions.len()
            {
                let local = if at_end {
                    t.length
                } else {
                    (x - start).clamp(0., t.length)
                };
                if let Function::Nurbs { profile } = &t.function {
                    return profile.evaluate_validated(
                        profile.parameter(local, t.length),
                        t.length,
                        side,
                    );
                }
                return t.evaluate_validated(local);
            }
            start = end;
        }
        unreachable!("validated channel is nonempty")
    }
    pub fn knots(&self) -> Vec<f64> {
        let mut sum = crate::InputSum::default();
        let mut knots = vec![0.0];
        for t in &self.transitions {
            let start = *knots.last().unwrap();
            knots.extend(t.internal_knots().into_iter().map(|x| start + x));
            knots.push(sum.add(t.length));
        }
        knots
    }
    pub fn continuity(&self) -> Result<Vec<ContinuityIssue>> {
        self.validate()?;
        let mut result = Vec::new();
        let mut sum = crate::InputSum::default();
        for pair in self.transitions.windows(2) {
            let x = sum.add(pair[0].length);
            let a = pair[0].evaluate(pair[0].length)?;
            let b = pair[1].evaluate(0.0)?;
            if !approximately_equal(a.value, b.value, 1e-10, 1e-10)
                || !approximately_equal(a.derivative, b.derivative, 1e-10, 1e-10)
                || !approximately_equal(a.second_derivative, b.second_derivative, 1e-8, 1e-10)
            {
                result.push(ContinuityIssue {
                    input: x,
                    value_jump: b.value - a.value,
                    derivative_jump: b.derivative - a.derivative,
                    second_derivative_jump: b.second_derivative - a.second_derivative,
                });
            }
        }
        let mut cursor = 0.;
        for t in &self.transitions {
            for x in t.internal_knots() {
                let a = t.evaluate_limit(x, Side::Left)?;
                let b = t.evaluate_limit(x, Side::Right)?;
                if !approximately_equal(a.value, b.value, 1e-10, 1e-10)
                    || !approximately_equal(a.derivative, b.derivative, 1e-10, 1e-10)
                    || !approximately_equal(a.second_derivative, b.second_derivative, 1e-8, 1e-10)
                {
                    result.push(ContinuityIssue {
                        input: cursor + x,
                        value_jump: b.value - a.value,
                        derivative_jump: b.derivative - a.derivative,
                        second_derivative_jump: b.second_derivative - a.second_derivative,
                    });
                }
            }
            cursor += t.length;
        }
        result.sort_by(|a, b| a.input.total_cmp(&b.input));
        Ok(result)
    }
    /// Keep real short features while redundant inserted split knots do not
    /// repartition integration. Jets through 2p detect rational span changes.
    pub fn integration_knots(&self) -> Result<Vec<f64>> {
        self.validate()?;
        let changes = |a: &Transition, x: f64, b: &Transition, y: f64| -> Result<bool> {
            let a_jet = a.derivatives(x, Side::Left, 10)?;
            let b_jet = b.derivatives(y, Side::Right, 10)?;
            let scale = a
                .start
                .abs()
                .max(a.end.abs())
                .max(b.start.abs())
                .max(b.end.abs())
                .max(a.amplitude.abs())
                .max(b.amplitude.abs())
                .max(1e-12);
            let scale = [a, b].iter().fold(scale, |scale, t| {
                if let Function::Nurbs { profile } = &t.function {
                    profile
                        .values
                        .iter()
                        .fold(scale, |scale, v| scale.max(v.abs()))
                } else {
                    scale
                }
            });
            Ok(a_jet.iter().zip(b_jet).enumerate().any(|(k, (u, v))| {
                let conditioning = 512.
                    * f64::EPSILON
                    * scale
                    * (a.length.powi(-(k as i32)) + b.length.powi(-(k as i32)));
                !approximately_equal(*u, v, 1e-9 + conditioning, 1e-8)
            }))
        };
        let mut knots = vec![0.];
        let mut sum = crate::InputSum::default();
        let mut cursor = 0.;
        for (i, t) in self.transitions.iter().enumerate() {
            for x in t.internal_knots() {
                if changes(t, x, t, x)? {
                    knots.push(cursor + x);
                }
            }
            cursor = sum.add(t.length);
            if let Some(next) = self.transitions.get(i + 1)
                && changes(t, t.length, next, 0.)?
            {
                knots.push(cursor);
            }
        }
        knots.push(self.length());
        Ok(knots)
    }
    pub fn insert(&mut self, index: usize, transition: Transition) -> Result<()> {
        ensure(
            index <= self.transitions.len(),
            "transition insertion index out of bounds",
        )?;
        let mut candidate = self.clone();
        candidate.transitions.insert(index, transition);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn replace(&mut self, id: u64, transition: Transition) -> Result<()> {
        ensure(
            id == transition.id,
            "replacement must preserve transition ID",
        )?;
        let i = self
            .transitions
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| crate::GeometryError("transition ID not found".into()))?;
        let mut candidate = self.clone();
        candidate.transitions[i] = transition;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
    pub fn remove(&mut self, id: u64) -> Result<()> {
        let i = self
            .transitions
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| crate::GeometryError("transition ID not found".into()))?;
        let mut candidate = self.clone();
        candidate.transitions.remove(i);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoints_and_analytic_derivatives() {
        let functions = [
            Function::Linear {},
            Function::Quadratic {},
            Function::CubicSmoothstep {},
            Function::QuinticSmoothstep {},
            Function::Cosine {},
            Function::Hermite {
                start_slope: 0.3,
                end_slope: -0.7,
            },
        ];
        for function in functions {
            let t = Transition {
                id: 1,
                length: 3.0,
                start: -2.0,
                end: 4.0,
                amplitude: 0.8,
                function,
            };
            assert!((t.evaluate(0.0).unwrap().value + 2.0).abs() < 1e-12);
            assert!((t.evaluate(3.0).unwrap().value - 4.0).abs() < 1e-12);
            let x = 1.1;
            let eps = 1e-5;
            let finite = (t.evaluate(x + eps).unwrap().value - t.evaluate(x - eps).unwrap().value)
                / (2.0 * eps);
            assert!((finite - t.evaluate(x).unwrap().derivative).abs() < 1e-8);
        }
        let t = Transition {
            id: 2,
            length: 2.0,
            start: 1.0,
            end: 4.0,
            amplitude: 0.0,
            function: Function::Hermite {
                start_slope: 0.3,
                end_slope: -0.7,
            },
        };
        assert!((t.evaluate(0.0).unwrap().derivative - 0.3).abs() < 1e-12);
        assert!((t.evaluate(2.0).unwrap().derivative + 0.7).abs() < 1e-12);
    }
    #[test]
    fn ordered_editing_and_one_sided_limits() {
        let mut c = Channel::constant(Unit::G, 1.0, 1.0);
        c.insert(1, Transition::constant(2, 2.0, 3.0)).unwrap();
        assert_eq!(c.evaluate(1.0).unwrap().value, 3.0);
        assert_eq!(c.evaluate_side(1.0, Side::Left).unwrap().value, 1.0);
        assert_eq!(c.continuity().unwrap()[0].value_jump, 2.0);
        let before = c.clone();
        assert!(c.insert(0, Transition::constant(2, 1.0, 0.0)).is_err());
        assert_eq!(before, c);
        c.remove(2).unwrap();
        assert!(c.remove(1).is_err());
        assert!(c.evaluate(f64::NAN).is_err());
        assert!(c.evaluate(-1.0).is_err());
        let mut t = Transition::constant(1, 0.0, 1.0);
        assert!(t.validate().is_err());
        t.length = 1.0;
        t.end = 2.0;
        assert!(t.validate().is_err());
    }
    #[test]
    fn flat_endpoint_slopes_amplitude_and_compensated_boundaries() {
        for function in [
            Function::CubicSmoothstep {},
            Function::QuinticSmoothstep {},
            Function::Cosine {},
        ] {
            let t = Transition {
                id: 1,
                length: 2.0,
                start: 0.0,
                end: 2.0,
                amplitude: 0.7,
                function,
            };
            assert!(t.evaluate(0.0).unwrap().derivative.abs() < 1e-12);
            assert!(t.evaluate(2.0).unwrap().derivative.abs() < 1e-12);
            assert!((t.evaluate(1.0).unwrap().value - 1.7).abs() < 1e-12);
        }
        let channel = Channel {
            unit: Unit::G,
            transitions: (1..=1000)
                .map(|id| Transition::constant(id, 0.1, 1.0))
                .collect(),
        };
        channel.validate().unwrap();
        assert_eq!(channel.length(), 100.0);
        assert!(channel.continuity().unwrap().is_empty());
        let bad = Transition {
            id: 1,
            length: 1.0,
            start: 0.0,
            end: 0.0,
            amplitude: 0.0,
            function: Function::Hermite {
                start_slope: f64::INFINITY,
                end_slope: 0.0,
            },
        };
        assert!(bad.evaluate(0.5).is_err());
    }
}
