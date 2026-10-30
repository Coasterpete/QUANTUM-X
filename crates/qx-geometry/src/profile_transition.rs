//! Exact profile conversion/subdivision and explicit constrained editing.
use crate::{
    Result, ensure,
    profile::{Continuity, NurbsProfile},
    transition::{Evaluation, Function, Side, Transition},
};

impl Transition {
    pub fn nurbs(id: u64, length: f64, start: f64, end: f64) -> Self {
        let mut profile = NurbsProfile::cubic(start, end);
        profile.incoming = Continuity::C2;
        Self::from_profile(id, length, profile)
    }
    pub fn from_profile(id: u64, length: f64, profile: NurbsProfile) -> Self {
        Self {
            id,
            length,
            start: profile.values.first().copied().unwrap_or(0.),
            end: profile.values.last().copied().unwrap_or(0.),
            amplitude: 0.,
            function: Function::Nurbs { profile },
        }
    }
    /// Polynomial legacy functions change basis exactly. Higher degree is retained
    /// when a quartic bump or quintic cannot be represented by a cubic.
    pub fn to_nurbs(&self) -> Result<Self> {
        self.validate()?;
        if matches!(self.function, Function::Nurbs { .. }) {
            return Ok(self.clone());
        }
        let d = self.end - self.start;
        let mut a = vec![self.start, 0., 0., 0.];
        match self.function {
            Function::Constant {}=>{},
            Function::Linear {}=>a[1]=d,
            Function::Quadratic {}=>a[2]=d,
            Function::CubicSmoothstep {}=>{a[2]=3.*d;a[3]=-2.*d;},
            Function::QuinticSmoothstep {}=>{a.resize(6,0.);a[3]=10.*d;a[4]=-15.*d;a[5]=6.*d;},
            Function::Hermite {start_slope:s,end_slope:e}=>{
                a[1]=self.length*s;a[2]=3.*d-self.length*(2.*s+e);a[3]=-2.*d+self.length*(s+e);
            },
            _=>return Err(crate::GeometryError("Cosine is preserved analytically; exact conversion to a finite NURBS is unavailable. Select an explicit reshape to change its representation.".into())),
        }
        if self.amplitude != 0. {
            a.resize(a.len().max(5), 0.);
            a[2] += 16. * self.amplitude;
            a[3] -= 32. * self.amplitude;
            a[4] += 16. * self.amplitude;
        }
        let mut p = NurbsProfile::polynomial(&a, a.len() - 1);
        p.values[0] = self.start;
        *p.values.last_mut().unwrap() = self.end;
        let next = Self::from_profile(self.id, self.length, p);
        next.validate()?;
        Ok(next)
    }
    pub fn incoming_continuity(&self) -> Continuity {
        match &self.function {
            Function::Nurbs { profile } => profile.incoming,
            _ => Continuity::Free,
        }
    }
    /// Exact knot insertion for rational profiles; exact analytic restriction for cosine.
    pub fn split(&self, fraction: f64, new_id: u64) -> Result<(Self, Self)> {
        self.validate()?;
        let x = self.length * fraction;
        ensure(
            fraction.is_finite()
                && fraction > 0.
                && fraction < 1.
                && x >= 1e-8
                && self.length - x >= 1e-8,
            "split must leave both regions at least 1e-8 domain units",
        )?;
        ensure(
            new_id > 0 && new_id != self.id,
            "split needs a fresh region ID",
        )?;
        if let Ok(t) = self.to_nurbs() {
            let Function::Nurbs { profile } = t.function else {
                unreachable!()
            };
            let (a, b) = profile.split(fraction)?;
            let left = Self::from_profile(self.id, x, a);
            let mut right = Self::from_profile(new_id, self.length - x, b);
            let ea = left.evaluate(x)?;
            let eb = right.evaluate(0.)?;
            if let Function::Nurbs { profile } = &mut right.function {
                profile.incoming =
                    if crate::approximately_equal(ea.derivative, eb.derivative, 1e-9, 1e-9) {
                        if crate::approximately_equal(
                            ea.second_derivative,
                            eb.second_derivative,
                            1e-7,
                            1e-9,
                        ) {
                            Continuity::C2
                        } else {
                            Continuity::C1
                        }
                    } else {
                        Continuity::C0
                    };
            }
            return Ok((left, right));
        }
        let (offset, scale, lo, hi, p) = match self.function {
            Function::Cosine {} => (
                self.start + (self.end - self.start) * 0.5,
                -(self.end - self.start) * 0.5,
                0.,
                std::f64::consts::PI,
                [
                    0.,
                    0.,
                    16. * self.amplitude,
                    -32. * self.amplitude,
                    16. * self.amplitude,
                ],
            ),
            Function::CosineSpan {
                offset,
                scale,
                phase_start,
                phase_end,
                polynomial,
            } => (offset, scale, phase_start, phase_end, polynomial),
            _ => return self.to_nurbs().map(|_| unreachable!()),
        };
        let restrict = |id, length, from: f64, to: f64| -> Result<Self> {
            let mut polynomial = [0.; 5];
            // Horner composition P(from+(to-from)*u), exact polynomial algebra.
            for coefficient in p.iter().rev() {
                for j in (1..5).rev() {
                    polynomial[j] = polynomial[j] * from + polynomial[j - 1] * (to - from);
                }
                polynomial[0] = polynomial[0] * from + coefficient;
            }
            let mut t = Self {
                id,
                length,
                start: 0.,
                end: 0.,
                amplitude: 0.,
                function: Function::CosineSpan {
                    offset,
                    scale,
                    phase_start: lo + (hi - lo) * from,
                    phase_end: lo + (hi - lo) * to,
                    polynomial,
                },
            };
            t.start = t.evaluate_validated(0.)?.value;
            t.end = t.evaluate_validated(length)?.value;
            t.validate()?;
            Ok(t)
        };
        Ok((
            restrict(self.id, x, 0., fraction)?,
            restrict(new_id, self.length - x, fraction, 1.)?,
        ))
    }
    pub fn set_endpoint_jet(&mut self, end: bool, target: Evaluation, order: usize) -> Result<()> {
        let mut next = self.to_nurbs()?;
        let Function::Nurbs { profile } = &mut next.function else {
            unreachable!()
        };
        profile.set_jet(end, next.length, target, order)?;
        next.start = profile.values[0];
        next.end = *profile.values.last().unwrap();
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn scale_values(&mut self, scale: f64) {
        self.start *= scale;
        self.end *= scale;
        self.amplitude *= scale;
        match &mut self.function {
            Function::Nurbs { profile } => {
                for v in &mut profile.values {
                    *v *= scale;
                }
            }
            Function::Hermite {
                start_slope,
                end_slope,
            } => {
                *start_slope *= scale;
                *end_slope *= scale;
            }
            Function::CosineSpan {
                offset,
                scale: s,
                polynomial,
                ..
            } => {
                *offset *= scale;
                *s *= scale;
                for v in polynomial {
                    *v *= scale;
                }
            }
            _ => {}
        }
    }
    /// Internal spline breaks (including derivative-only jumps) in physical domain units.
    pub fn internal_knots(&self) -> Vec<f64> {
        if let Function::Nurbs { profile } = &self.function {
            let mut k: Vec<_> = profile
                .knots
                .iter()
                .copied()
                .filter(|u| *u > 0. && *u < 1.)
                .map(|u| u * self.length)
                .collect();
            k.dedup();
            k
        } else {
            vec![]
        }
    }
    pub(crate) fn derivatives(&self, x: f64, side: Side, order: usize) -> Result<Vec<f64>> {
        if let Function::Nurbs { profile } = &self.function {
            return Ok(profile.derivatives_validated(
                profile.parameter(x, self.length),
                self.length,
                side,
                order,
            ));
        }
        if let Ok(t) = self.to_nurbs() {
            return t.derivatives(x, side, order);
        }
        let (offset, scale, lo, hi, p) = match self.function {
            Function::Cosine {} => (
                self.start + (self.end - self.start) * 0.5,
                -(self.end - self.start) * 0.5,
                0.,
                std::f64::consts::PI,
                [
                    0.,
                    0.,
                    16. * self.amplitude,
                    -32. * self.amplitude,
                    16. * self.amplitude,
                ],
            ),
            Function::CosineSpan {
                offset,
                scale,
                phase_start,
                phase_end,
                polynomial,
            } => (offset, scale, phase_start, phase_end, polynomial),
            _ => {
                return Err(crate::GeometryError(
                    "unsupported profile derivative representation".into(),
                ));
            }
        };
        let u = x / self.length;
        let rate = hi - lo;
        let phase = lo + rate * u;
        Ok((0..=order)
            .map(|k| {
                let trig = match k % 4 {
                    0 => phase.cos(),
                    1 => -phase.sin(),
                    2 => -phase.cos(),
                    _ => phase.sin(),
                };
                let polynomial = (k..5)
                    .map(|i| {
                        p[i] * (0..k).fold(1., |v, j| v * (i - j) as f64) * u.powi((i - k) as i32)
                    })
                    .sum::<f64>();
                ((if k == 0 { offset } else { 0. })
                    + scale * rate.powi(k as i32) * trig
                    + polynomial)
                    / self.length.powi(k as i32)
            })
            .collect())
    }
    pub fn evaluate_limit(&self, x: f64, side: Side) -> Result<Evaluation> {
        self.validate()?;
        ensure(
            x.is_finite() && (0.0..=self.length).contains(&x),
            "profile coordinate out of range",
        )?;
        if let Function::Nurbs { profile } = &self.function {
            profile.evaluate_validated(profile.parameter(x, self.length), self.length, side)
        } else {
            self.evaluate_validated(x)
        }
    }
}
