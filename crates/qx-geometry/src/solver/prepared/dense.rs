//! Experimental C3 quaternion-hodograph extension of the retained source RK4.
//!
//! Q(t) is septic endpoint Hermite data from the shared source equations. Define
//! C_t = h vec(Q X Q*) and s_t = h |Q|². Polynomial antiderivatives and a
//! monotone scalar inverse give C_s = R X, R = rotation(Q/|Q|), throughout a cell.
//! Position is integrated from this hodograph, NOT interpolated independently.
//! The resulting physical stations differ slightly from nominal source stations;
//! neither stations nor endpoint positions are silently pinned to RK4 output.
//! No production generation, document or physics behavior uses this module.
use super::*;

const ZERO: Quaternion = Quaternion {
    w: 0.,
    x: 0.,
    y: 0.,
    z: 0.,
};
fn dot(a: Quaternion, b: Quaternion) -> f64 {
    a.w * b.w + a.x * b.x + a.y * b.y + a.z * b.z
}
fn vector(q: Quaternion) -> Vec3 {
    Vec3::new(q.x, q.y, q.z)
}
fn pure(v: Vec3) -> Quaternion {
    Quaternion {
        w: 0.,
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

/// Inspectable numerical representation, not a persisted curve or rail face.
#[derive(Clone, Debug)]
pub struct Segment {
    pub start_station_m: f64,
    pub end_station_m: f64,
    pub start_position_m: Vec3,
    pub nominal_step_m: f64,
    pub start_input: f64,
    pub end_input: f64,
    /// Power coefficients Q(t), t in [0,1]. Polynomial primitives use these
    /// very coefficients; independent tools can reconstruct every pose.
    pub quaternion_coefficients: [Quaternion; 8],
    pub chart: usize,
    /// Conservative power-basis bound on |Q|, over the entire cell.
    pub quaternion_norm_lower_bound: f64,
    speed: [f64; 15],
    hodograph: [Vec3; 15],
}

fn primitive(coeff: &[f64; 15], t: f64) -> f64 {
    let mut sum = 0.;
    for i in (0..15).rev() {
        sum = sum * t + coeff[i] / (i + 1) as f64;
    }
    sum * t
}
impl Segment {
    fn displacement(&self, t: f64) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for i in (0..15).rev() {
            sum = sum * t + self.hodograph[i] / (i + 1) as f64;
        }
        sum * (t * self.nominal_step_m)
    }
    fn distance(&self, t: f64) -> f64 {
        primitive(&self.speed, t) * self.nominal_step_m
    }
    fn q_jet(&self, t: f64) -> [Quaternion; 4] {
        std::array::from_fn(|order| {
            let mut sum = ZERO;
            for i in (order..8).rev() {
                let factor = (0..order).fold(1., |v, j| v * (i - j) as f64);
                sum = sum
                    .scaled(t)
                    .plus(self.quaternion_coefficients[i].scaled(factor));
            }
            sum
        })
    }
    fn inverse(&self, s: f64) -> Result<f64> {
        if s == self.start_station_m {
            return Ok(0.);
        }
        if s == self.end_station_m {
            return Ok(1.);
        }
        let target = s - self.start_station_m;
        let length = self.end_station_m - self.start_station_m;
        let (mut lo, mut hi) = (0., 1.);
        let mut t = (target / length).clamp(0., 1.);
        for _ in 0..48 {
            let f = self.distance(t) - target;
            // Solve locally, without a world-station-sized residual tolerance.
            if f.abs() <= 4. * f64::EPSILON * self.nominal_step_m {
                return Ok(t);
            }
            if f > 0. {
                hi = t;
            } else {
                lo = t;
            }
            let q = self.q_jet(t)[0];
            let next = t - f / (self.nominal_step_m * dot(q, q));
            t = if next > lo && next < hi {
                next
            } else {
                (lo + hi) * 0.5
            };
        }
        Err(GeometryError(
            "dense physical-station inverse did not converge".into(),
        ))
    }
    fn pose(&self, s: f64) -> Result<(State, [Vec3; 3], f64)> {
        let t = self.inverse(s)?;
        let [q, dq, ddq, dddq] = self.q_jet(t);
        let n = dot(q, q);
        let dn = 2. * dot(q, dq);
        let ddn = 2. * (dot(dq, dq) + dot(q, ddq));
        let dddn = 2. * (3. * dot(dq, ddq) + dot(q, dddq));
        let f = n.sqrt().recip();
        let df = -0.5 * f * dn / n;
        let ddf = f * (0.75 * dn * dn / (n * n) - 0.5 * ddn / n);
        let dddf = f * (-1.875 * (dn / n).powi(3) + 2.25 * dn * ddn / (n * n) - 0.5 * dddn / n);
        let a = q.scaled(f);
        let du = dq.scaled(f).plus(q.scaled(df));
        let ddu = ddq.scaled(f).plus(dq.scaled(2. * df)).plus(q.scaled(ddf));
        let dddu = dddq
            .scaled(f)
            .plus(ddq.scaled(3. * df))
            .plus(dq.scaled(3. * ddf))
            .plus(q.scaled(dddf));
        let v = self.nominal_step_m * n;
        let dv = self.nominal_step_m * dn;
        let ddv = self.nominal_step_m * ddn;
        let da = du.scaled(1. / v);
        let dda = ddu.scaled(1. / v.powi(2)).plus(du.scaled(-dv / v.powi(3)));
        let ddda = dddu
            .scaled(1. / v.powi(3))
            .plus(ddu.scaled(-3. * dv / v.powi(4)))
            .plus(du.scaled(3. * dv * dv / v.powi(5) - ddv / v.powi(4)));
        let angular = [
            vector(da.compose(a.conjugate())) * 2.,
            vector(dda.compose(a.conjugate()).plus(da.compose(da.conjugate()))) * 2.,
            vector(
                ddda.compose(a.conjugate())
                    .plus(dda.compose(da.conjugate()).scaled(2.))
                    .plus(da.compose(dda.conjugate())),
            ) * 2.,
        ];
        ensure(
            angular.iter().all(|v| v.is_finite()),
            "dense angular jet overflow",
        )?;
        Ok((
            State {
                p: self.start_position_m + self.displacement(t),
                q: a,
            },
            angular,
            t,
        ))
    }
}

fn endpoint(context: Context<'_>, x: f64, q: Quaternion, side: Side) -> Result<[Quaternion; 4]> {
    let [k, dk, ddk] = context.omega_jet(x, q, side)?;
    let d = pure(k).compose(q).scaled(0.5);
    let dd = pure(dk).compose(q).plus(pure(k).compose(d)).scaled(0.5);
    let ddd = pure(ddk)
        .compose(q)
        .plus(pure(dk).compose(d).scaled(2.))
        .plus(pure(k).compose(dd))
        .scaled(0.5);
    Ok([q, d, dd, ddd])
}
fn coefficients(a: [Quaternion; 4], b: [Quaternion; 4], h: f64) -> [Quaternion; 8] {
    let mut c = [ZERO; 8];
    c[0] = a[0];
    c[1] = a[1].scaled(h);
    c[2] = a[2].scaled(h * h / 2.);
    c[3] = a[3].scaled(h.powi(3) / 6.);
    let r0 = b[0].plus(c[0].plus(c[1]).plus(c[2]).plus(c[3]).scaled(-1.));
    let r1 = b[1]
        .scaled(h)
        .plus(c[1].plus(c[2].scaled(2.)).plus(c[3].scaled(3.)).scaled(-1.));
    let r2 = b[2]
        .scaled(h * h)
        .plus(c[2].scaled(2.).plus(c[3].scaled(6.)).scaled(-1.));
    let r3 = b[3].scaled(h.powi(3)).plus(c[3].scaled(-6.));
    for (i, weights) in [
        [35., -15., 2.5, -1. / 6.],
        [-84., 39., -7., 0.5],
        [70., -34., 6.5, -0.5],
        [-20., 10., -2., 1. / 6.],
    ]
    .iter()
    .enumerate()
    {
        c[i + 4] = [r0, r1, r2, r3]
            .into_iter()
            .zip(weights)
            .fold(ZERO, |s, (v, w)| s.plus(v.scaled(*w)));
    }
    c
}

fn build(source: &PreparedGraph, grid: &[Vec<Cell>]) -> Result<Vec<Segment>> {
    let mut result = Vec::new();
    let mut sum = crate::InputSum::default();
    let mut station = 0.;
    let mut p = source.initial.position_m;
    for (chart, cells) in grid.iter().enumerate() {
        let context = source.context(chart);
        let scale = context.section.domain.metres_per_input(context.speed);
        for cell in cells {
            let h = (cell.x1 - cell.x0) * scale;
            let a = endpoint(context, cell.x0, cell.a.q, Side::Right)?;
            let b = endpoint(context, cell.x1, cell.b.q, Side::Left)?;
            ensure(
                dot(a[0], b[0]) > 0.,
                "dense quaternion sign/rotation branch",
            )?;
            let q = coefficients(a, b, h);
            let radius = q[1..].iter().map(|v| dot(*v, *v).sqrt()).sum::<f64>();
            let lower = dot(q[0], q[0]).sqrt() - radius;
            ensure(
                lower > 0.5,
                "dense quaternion interval cannot be certified nonsingular",
            )?;
            let mut speed = [0.; 15];
            let mut hodograph = [Vec3::ZERO; 15];
            for i in 0..8 {
                for j in 0..8 {
                    speed[i + j] += dot(q[i], q[j]);
                    hodograph[i + j] = hodograph[i + j]
                        + vector(q[i].compose(pure(Vec3::X)).compose(q[j].conjugate()));
                }
            }
            let length = h * primitive(&speed, 1.);
            let end = sum.add(length);
            ensure(
                length.is_finite() && end > station,
                "dense station accumulation lost precision",
            )?;
            let segment = Segment {
                start_station_m: station,
                end_station_m: end,
                start_position_m: p,
                nominal_step_m: h,
                start_input: cell.x0,
                end_input: cell.x1,
                quaternion_coefficients: q,
                chart,
                quaternion_norm_lower_bound: lower,
                speed,
                hodograph,
            };
            p = p + segment.displacement(1.);
            result.push(segment);
            station = end;
        }
    }
    Ok(result)
}

/// Immutable research reference. Physical-station and position checkpoints are
/// regenerated from the hodograph. The old restart evaluator remains executable.
pub struct ContinuousGraph {
    source: PreparedGraph,
    fine: Vec<Segment>,
    coarse: Vec<Segment>,
}
impl ContinuousGraph {
    pub fn prepare(
        identity: SourceIdentity,
        reference: AuthoredReference,
        initial: &InitialConditions,
        sections: &[Section],
        options: GenerationOptions,
    ) -> Result<Self> {
        let source = PreparedGraph::prepare(identity, reference, initial, sections, options)?;
        let fine = build(&source, &source.fine)?;
        let coarse = build(&source, &source.coarse)?;
        Ok(Self {
            source,
            fine,
            coarse,
        })
    }
    pub fn segments(&self) -> &[Segment] {
        &self.fine
    }
    pub fn length_m(&self) -> f64 {
        self.fine.last().expect("validated source").end_station_m
    }
    pub fn source(&self) -> &PreparedGraph {
        &self.source
    }
    pub fn validate_interval(&self, expected: SourceIdentity, a: f64, b: f64) -> Result<()> {
        self.check(expected, a)?;
        self.check(expected, b)?;
        let (lo, hi) = (a.min(b), a.max(b));
        for boundary in self.source.boundaries() {
            let i = self
                .fine
                .partition_point(|c| c.chart < boundary.right_chart);
            let s = self.fine[i].start_station_m;
            // The older 1e-9 source classification is not a continuity proof.
            // Never average even a small authored jump into a shared jet.
            ensure(
                boundary.angular_gaps.iter().all(|v| *v == 0.) || s < lo || s > hi,
                "dense interval touches unequal sided source jets",
            )?;
        }
        Ok(())
    }
    fn check(&self, expected: SourceIdentity, s: f64) -> Result<()> {
        ensure(expected == self.source.identity, "stale source identity")?;
        ensure(
            s.is_finite() && s >= 0. && s <= self.length_m(),
            "station outside dense physical source",
        )
    }
    pub fn evaluate(&self, expected: SourceIdentity, s: f64, side: Side) -> Result<Evaluation> {
        self.check(expected, s)?;
        ensure(
            !(s == 0. && side == Side::Left || s == self.length_m() && side == Side::Right),
            "requested side outside open dense domain",
        )?;
        let select = |grid: &[Segment], station| {
            grid.partition_point(|c| match side {
                Side::Left => c.end_station_m < station,
                Side::Right => c.end_station_m <= station,
            })
            .min(grid.len() - 1)
        };
        let cell = &self.fine[select(&self.fine, s)];
        let (pose, angular, t) = cell.pose(s)?;
        let old_s = s.min(self.coarse.last().expect("source").end_station_m);
        let (old, old_angular, _) = self.coarse[select(&self.coarse, old_s)].pose(old_s)?;
        let frame = Frame::new(pose.q)?;
        let tangent = frame.tangent();
        let dt = angular[0].cross(tangent);
        Ok(Evaluation {
            source: expected,
            section: self.source.charts[cell.chart].section,
            chart: cell.chart,
            station_m: s,
            section_input: cell.start_input + (cell.end_input - cell.start_input) * t,
            position_m: pose.p,
            frame,
            position_derivatives: [
                tangent,
                dt,
                angular[1].cross(tangent) + angular[0].cross(dt),
            ],
            angular_derivatives: angular,
            provenance: DerivativeProvenance::NumericalDenseExtension,
            refinement_position_difference_m: (pose.p - old.p).length(),
            refinement_orientation_difference_rad: pose
                .q
                .compose(old.q.conjugate())
                .rotation_vector()?
                .length(),
            refinement_angular_differences: std::array::from_fn(|i| {
                (angular[i] - old_angular[i]).length()
            }),
            local_rk4_steps: 0,
        })
    }
}
