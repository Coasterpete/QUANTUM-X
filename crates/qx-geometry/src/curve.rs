//! Authoritative f64 curve representations. Derivatives use the native parameter.
//! Interior knots are right-sided by default; the final endpoint is left-sided.
use crate::{Result, ensure, math::Vec3, transition::Side};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlPoint {
    pub id: u64,
    pub position_m: Vec3,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Parameterization {
    Uniform,
    Chordal,
    #[default]
    Centripetal,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Curve {
    /// Shared endpoints: 3*n+1 controls, each segment has a unit parameter span.
    Bezier { points: Vec<ControlPoint> },
    BSpline {
        degree: usize,
        points: Vec<ControlPoint>,
        knots: Vec<f64>,
    },
    Nurbs {
        degree: usize,
        points: Vec<ControlPoint>,
        knots: Vec<f64>,
        weights: Vec<f64>,
    },
    /// Reflected endpoint ghosts give explicit, one-sided endpoint tangents.
    CatmullRom {
        points: Vec<ControlPoint>,
        parameterization: Parameterization,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveEvaluation {
    pub position_m: Vec3,
    pub first: Vec3,
    pub second: Vec3,
}
impl CurveEvaluation {
    pub fn tangent(self) -> Result<Vec3> {
        self.first.normalized()
    }
    pub fn curvature_vector(self) -> Result<Vec3> {
        let t = self.tangent()?;
        let speed = self.first.length();
        Ok((self.second - t * t.dot(self.second)) / (speed * speed))
    }
    pub fn curvature(self) -> Result<f64> {
        Ok(self.curvature_vector()?.length())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CurveJoin {
    pub parameter: f64,
    pub position_gap_m: f64,
    pub tangent_angle_rad: f64,
    pub first_derivative_gap: f64,
    pub second_derivative_gap: f64,
    pub curvature_vector_gap_per_m: f64,
}
impl CurveJoin {
    pub fn g0(&self, tolerance: f64) -> bool {
        self.position_gap_m <= tolerance
    }
    pub fn g1(&self, tolerance: f64) -> bool {
        self.g0(tolerance) && self.tangent_angle_rad <= tolerance
    }
    pub fn g2(&self, tolerance: f64) -> bool {
        self.g1(tolerance) && self.curvature_vector_gap_per_m <= tolerance
    }
    pub fn c1(&self, tolerance: f64) -> bool {
        self.g0(tolerance) && self.first_derivative_gap <= tolerance
    }
    pub fn c2(&self, tolerance: f64) -> bool {
        self.c1(tolerance) && self.second_derivative_gap <= tolerance
    }
}

impl Curve {
    pub fn method(&self) -> &'static str {
        match self {
            Self::Bezier { .. } => "bezier",
            Self::BSpline { .. } => "b_spline",
            Self::Nurbs { .. } => "nurbs",
            Self::CatmullRom { .. } => "catmull_rom",
        }
    }
    pub fn points(&self) -> &[ControlPoint] {
        match self {
            Self::Bezier { points }
            | Self::BSpline { points, .. }
            | Self::Nurbs { points, .. }
            | Self::CatmullRom { points, .. } => points,
        }
    }
    pub fn points_mut(&mut self) -> &mut Vec<ControlPoint> {
        match self {
            Self::Bezier { points }
            | Self::BSpline { points, .. }
            | Self::Nurbs { points, .. }
            | Self::CatmullRom { points, .. } => points,
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.prepare().map(|_| ())
    }
    pub fn evaluate(&self, parameter: f64) -> Result<CurveEvaluation> {
        self.prepare()?.evaluate(parameter, Side::Right)
    }
    pub fn prepare(&self) -> Result<PreparedCurve> {
        let points = self.points();
        ensure(
            (2..=4096).contains(&points.len()),
            "curve requires 2..4096 controls",
        )?;
        let mut ids = std::collections::HashSet::new();
        for p in points {
            ensure(
                p.id > 0 && ids.insert(p.id),
                "curve control IDs must be unique and nonzero",
            )?;
            ensure(
                p.position_m.is_finite() && p.position_m.length() <= 1e8,
                "invalid curve control position (limit 1e8 m)",
            )?;
        }
        let mut spans = Vec::new();
        let spline = match self {
            Self::Bezier { points } => {
                ensure(
                    points.len() >= 4 && (points.len() - 1).is_multiple_of(3),
                    "cubic Bezier requires 3*n+1 controls",
                )?;
                for p in points.windows(4).step_by(3) {
                    spans.push(Cubic::bezier(
                        [
                            p[0].position_m,
                            p[1].position_m,
                            p[2].position_m,
                            p[3].position_m,
                        ],
                        spans.len() as f64,
                        1.0,
                    ));
                }
                None
            }
            Self::BSpline {
                degree,
                points,
                knots,
            } => Some(Spline::new(
                *degree,
                points,
                knots,
                &vec![1.0; points.len()],
            )?),
            Self::Nurbs {
                degree,
                points,
                knots,
                weights,
            } => Some(Spline::new(*degree, points, knots, weights)?),
            Self::CatmullRom {
                points,
                parameterization,
            } => {
                let p: Vec<_> = points.iter().map(|p| p.position_m).collect();
                let alpha = match parameterization {
                    Parameterization::Uniform => 0.0,
                    Parameterization::Chordal => 1.0,
                    Parameterization::Centripetal => 0.5,
                };
                let mut time = 0.0;
                for i in 0..p.len() - 1 {
                    let p0 = if i == 0 { p[0] * 2.0 - p[1] } else { p[i - 1] };
                    let p3 = if i + 2 == p.len() {
                        p[i + 1] * 2.0 - p[i]
                    } else {
                        p[i + 2]
                    };
                    let a = (p[i] - p0).length();
                    let b = (p[i + 1] - p[i]).length();
                    let c = (p3 - p[i + 1]).length();
                    ensure(
                        a > 1e-9 && b > 1e-9 && c > 1e-9,
                        "Catmull-Rom has duplicate/degenerate adjacent controls",
                    )?;
                    let (a, b, c) = (a.powf(alpha), b.powf(alpha), c.powf(alpha));
                    let m0 =
                        ((p[i] - p0) / a - (p[i + 1] - p0) / (a + b) + (p[i + 1] - p[i]) / b) * b;
                    let m1 =
                        ((p[i + 1] - p[i]) / b - (p3 - p[i]) / (b + c) + (p3 - p[i + 1]) / c) * b;
                    spans.push(Cubic::bezier(
                        [p[i], p[i] + m0 / 3.0, p[i + 1] - m1 / 3.0, p[i + 1]],
                        time,
                        b,
                    ));
                    time += b;
                }
                None
            }
        };
        let breaks = if let Some(s) = &spline {
            let mut k = s.knots[0][s.degree..=s.controls[0].len()].to_vec();
            k.dedup();
            k
        } else {
            let mut k: Vec<_> = spans.iter().map(|s| s.start).collect();
            let last = spans.last().expect("nonempty cubic curve");
            k.push(last.start + last.length);
            k
        };
        Ok(PreparedCurve {
            spans,
            spline,
            breaks,
        })
    }
    /// Explicit rigid alignment: translate the start to zero and rotate its tangent to +X.
    /// The shortest rotation preserves distances; exact antiparallel tangents use
    /// a deterministic perpendicular axis selected from world +Y (or +Z).
    pub fn align_start(&mut self) -> Result<()> {
        let prepared = self.prepare()?;
        let e = prepared.evaluate(prepared.domain().0, Side::Right)?;
        let q = crate::math::rotation_between(e.tangent()?, Vec3::X)?;
        for p in self.points_mut() {
            p.position_m = q.rotate_unit(p.position_m - e.position_m);
        }
        Ok(())
    }
    /// Exact conversion for a single cubic Bezier, or nonrational B-spline to NURBS.
    pub fn to_nurbs(&self) -> Result<Self> {
        self.validate()?;
        match self {
            Self::Bezier { points } if points.len() == 4 => Ok(Self::Nurbs {
                degree: 3,
                points: points.clone(),
                knots: clamped_knots(4, 3)?,
                weights: vec![1.0; 4],
            }),
            Self::BSpline {
                degree,
                points,
                knots,
            } => Ok(Self::Nurbs {
                degree: *degree,
                points: points.clone(),
                knots: knots.clone(),
                weights: vec![1.0; points.len()],
            }),
            _ => Err(crate::GeometryError(
                "exact conversion to NURBS is supported for a single Bezier or B-spline".into(),
            )),
        }
    }
    /// Shape-preserving single Boehm knot insertion; caller supplies the new stable ID.
    pub fn insert_knot(&mut self, u: f64, new_id: u64) -> Result<()> {
        self.validate()?;
        ensure(
            new_id > 0 && self.points().iter().all(|p| p.id != new_id),
            "new control ID is invalid or already used",
        )?;
        let mut candidate = self.clone();
        let (degree, points, knots, mut weights) = match &mut candidate {
            Self::BSpline {
                degree,
                points,
                knots,
            } => (*degree, points, knots, None),
            Self::Nurbs {
                degree,
                points,
                knots,
                weights,
            } => (*degree, points, knots, Some(weights)),
            _ => {
                return Err(crate::GeometryError(
                    "knot insertion requires B-spline or NURBS".into(),
                ));
            }
        };
        let n = points.len() - 1;
        ensure(
            u.is_finite() && u > knots[degree] && u < knots[n + 1],
            "inserted knot must be strictly inside the curve domain",
        )?;
        let k = knots.partition_point(|v| *v <= u) - 1;
        let multiplicity = knots.iter().filter(|v| **v == u).count();
        ensure(
            multiplicity < degree,
            "knot insertion would create an unsupported discontinuity",
        )?;
        let h: Vec<_> = points
            .iter()
            .enumerate()
            .map(|(i, p)| Hom::new(p.position_m, weights.as_ref().map_or(1.0, |w| w[i])))
            .collect();
        let mut q = vec![Hom::default(); n + 2];
        q[..=k - degree].copy_from_slice(&h[..=k - degree]);
        q[k - multiplicity + 1..].copy_from_slice(&h[k - multiplicity..]);
        for i in k - degree + 1..=k - multiplicity {
            let a = (u - knots[i]) / (knots[i + degree] - knots[i]);
            q[i] = h[i - 1].mix(h[i], a);
        }
        // Retain old identifiers in order, assigning a fresh ID to the inserted slot.
        let slot = k - multiplicity;
        let mut ids: Vec<_> = points.iter().map(|p| p.id).collect();
        ids.insert(slot, new_id);
        *points = q
            .iter()
            .zip(ids)
            .map(|(h, id)| ControlPoint {
                id,
                position_m: h.xyz / h.w,
            })
            .collect();
        if let Some(w) = &mut weights {
            **w = q.iter().map(|h| h.w).collect();
        }
        knots.insert(k + 1, u);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}

pub fn clamped_knots(count: usize, degree: usize) -> Result<Vec<f64>> {
    ensure(
        (1..=8).contains(&degree) && count > degree && count <= 4096,
        "invalid control count/degree",
    )?;
    Ok((0..count + degree + 1)
        .map(|i| {
            if i <= degree {
                0.0
            } else if i >= count {
                1.0
            } else {
                (i - degree) as f64 / (count - degree) as f64
            }
        })
        .collect())
}

#[derive(Clone)]
struct Cubic {
    p: [Vec3; 4],
    start: f64,
    length: f64,
}
impl Cubic {
    fn bezier(p: [Vec3; 4], start: f64, length: f64) -> Self {
        Self { p, start, length }
    }
    fn evaluate(&self, u: f64) -> CurveEvaluation {
        let t = ((u - self.start) / self.length).clamp(0.0, 1.0);
        let mix = |a: Vec3, b: Vec3| a * (1.0 - t) + b * t;
        let [a, b, c, d] = self.p;
        let first = (mix(mix(b - a, c - b), mix(c - b, d - c))) * (3.0 / self.length);
        let second = mix(c - b * 2.0 + a, d - c * 2.0 + b) * (6.0 / (self.length * self.length));
        CurveEvaluation {
            position_m: mix(mix(mix(a, b), mix(b, c)), mix(mix(b, c), mix(c, d))),
            first,
            second,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Hom {
    xyz: Vec3,
    w: f64,
}
impl Hom {
    fn new(p: Vec3, w: f64) -> Self {
        Self { xyz: p * w, w }
    }
    fn mix(self, b: Self, t: f64) -> Self {
        Self {
            xyz: self.xyz * (1.0 - t) + b.xyz * t,
            w: self.w * (1.0 - t) + b.w * t,
        }
    }
    fn difference(self, b: Self, scale: f64) -> Self {
        Self {
            xyz: (self.xyz - b.xyz) * scale,
            w: (self.w - b.w) * scale,
        }
    }
}
struct Spline {
    degree: usize,
    controls: [Vec<Hom>; 3],
    knots: [Vec<f64>; 3],
}
impl Spline {
    fn new(degree: usize, points: &[ControlPoint], knots: &[f64], weights: &[f64]) -> Result<Self> {
        ensure(
            (1..=8).contains(&degree) && points.len() > degree,
            "B-spline degree must be 1..8 and less than control count",
        )?;
        ensure(
            knots.len() == points.len() + degree + 1,
            "wrong knot vector length",
        )?;
        ensure(
            knots.iter().all(|k| k.is_finite() && k.abs() <= 1e6)
                && knots.windows(2).all(|w| w[0] <= w[1]),
            "knots must be finite, bounded, and nondecreasing",
        )?;
        let lo = knots[degree];
        let hi = knots[points.len()];
        ensure(hi - lo > 1e-9, "empty or tiny B-spline parameter domain")?;
        for pair in knots.windows(2) {
            ensure(
                pair[1] == pair[0] || pair[1] - pair[0] >= 1e-10,
                "knot spans are too small",
            )?;
        }
        let mut run = 0;
        let mut prev = f64::NAN;
        for &u in knots {
            run = if u == prev { run + 1 } else { 1 };
            prev = u;
            ensure(
                run <= degree + 1 && (!(u > lo && u < hi) || run <= degree),
                "unsupported knot multiplicity (interior position discontinuity)",
            )?;
        }
        ensure(
            weights.len() == points.len()
                && weights
                    .iter()
                    .all(|w| w.is_finite() && (1e-8..=1e8).contains(w)),
            "NURBS weights must match controls and be positive in [1e-8,1e8]",
        )?;
        let h: Vec<_> = points
            .iter()
            .zip(weights)
            .map(|(p, &w)| Hom::new(p.position_m, w))
            .collect();
        let mut result = Self {
            degree,
            controls: [h, vec![], vec![]],
            knots: [knots.to_vec(), vec![], vec![]],
        };
        for order in 1..=2.min(degree) {
            let p = degree + 1 - order;
            result.controls[order] = (0..result.controls[order - 1].len() - 1)
                .map(|i| {
                    let den = result.knots[order - 1][i + p + 1] - result.knots[order - 1][i + 1];
                    if den == 0.0 {
                        Hom::default()
                    } else {
                        result.controls[order - 1][i + 1]
                            .difference(result.controls[order - 1][i], p as f64 / den)
                    }
                })
                .collect();
            result.knots[order] =
                result.knots[order - 1][1..result.knots[order - 1].len() - 1].to_vec();
        }
        Ok(result)
    }
    fn evaluate(&self, u: f64, side: Side) -> Result<CurveEvaluation> {
        let mut h = [Hom::default(); 3];
        for (order, item) in h.iter_mut().enumerate().take(2.min(self.degree) + 1) {
            *item = de_boor(
                self.degree - order,
                &self.controls[order],
                &self.knots[order],
                u,
                side,
            );
        }
        ensure(
            h[0].w > 0.0 && h[0].w.is_finite(),
            "rational denominator is zero or invalid",
        )?;
        let position_m = h[0].xyz / h[0].w;
        let first = (h[1].xyz - position_m * h[1].w) / h[0].w;
        let second = (h[2].xyz - first * (2.0 * h[1].w) - position_m * h[2].w) / h[0].w;
        Ok(CurveEvaluation {
            position_m,
            first,
            second,
        })
    }
}
fn de_boor(p: usize, controls: &[Hom], knots: &[f64], u: f64, side: Side) -> Hom {
    let n = controls.len() - 1;
    let k = if u >= knots[n + 1] {
        n
    } else if u <= knots[p] {
        p
    } else {
        let k = match side {
            Side::Left => knots.partition_point(|v| *v < u),
            Side::Right => knots.partition_point(|v| *v <= u),
        };
        (k - 1).clamp(p, n)
    };
    let mut d = [Hom::default(); 9];
    d[..=p].copy_from_slice(&controls[k - p..=k]);
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = knots[i + p - r + 1] - knots[i];
            let a = if den == 0.0 {
                0.0
            } else {
                (u - knots[i]) / den
            };
            d[j] = d[j - 1].mix(d[j], a);
        }
    }
    d[p]
}

/// Validated, prepared evaluator owns derivative nets; repeated evaluation avoids
/// rebuilding or rescanning the authored controls. No rendering/document types.
pub struct PreparedCurve {
    spans: Vec<Cubic>,
    spline: Option<Spline>,
    breaks: Vec<f64>,
}
impl PreparedCurve {
    pub fn domain(&self) -> (f64, f64) {
        (self.breaks[0], *self.breaks.last().expect("curve domain"))
    }
    pub fn breaks(&self) -> &[f64] {
        &self.breaks
    }
    pub fn evaluate(&self, u: f64, side: Side) -> Result<CurveEvaluation> {
        let (a, b) = self.domain();
        ensure(
            u.is_finite() && u >= a && u <= b,
            "curve parameter outside valid domain",
        )?;
        let e = if let Some(s) = &self.spline {
            s.evaluate(u, side)?
        } else {
            let i = match side {
                Side::Left => self.breaks.partition_point(|v| *v < u),
                Side::Right => self.breaks.partition_point(|v| *v <= u),
            }
            .saturating_sub(1)
            .min(self.spans.len() - 1);
            self.spans[i].evaluate(u)
        };
        ensure(
            e.position_m.is_finite() && e.first.is_finite() && e.second.is_finite(),
            "curve evaluation overflow",
        )?;
        Ok(e)
    }
    pub fn continuity(&self) -> Result<Vec<CurveJoin>> {
        self.breaks[1..self.breaks.len() - 1]
            .iter()
            .map(|&u| {
                let a = self.evaluate(u, Side::Left)?;
                let b = self.evaluate(u, Side::Right)?;
                Ok(CurveJoin {
                    parameter: u,
                    position_gap_m: (b.position_m - a.position_m).length(),
                    tangent_angle_rad: a
                        .tangent()?
                        .cross(b.tangent()?)
                        .length()
                        .atan2(a.tangent()?.dot(b.tangent()?)),
                    first_derivative_gap: (b.first - a.first).length(),
                    second_derivative_gap: (b.second - a.second).length(),
                    curvature_vector_gap_per_m: (b.curvature_vector()? - a.curvature_vector()?)
                        .length(),
                })
            })
            .collect()
    }
}
