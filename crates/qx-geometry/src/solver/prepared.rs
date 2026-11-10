//! Experimental graph-source queries. Enabled only by `experimental-graph-source`.
//!
//! Retains production RK4 checkpoints at two resolutions. Each query restarts
//! the same RK4 step from the preceding checkpoint, at most once per resolution.
//! No pose interpolation, station-zero reintegration, or physics integration.
//! Jets are source differential jets at an approximate pose, **not** exact
//! derivatives of the numerical RK4 extension. See the M4E-B report for defects.
use super::*;
use crate::heartline::AuthoredReference;

/// Caller-owned document-instance/generation identity. Never persisted or inferred
/// from equal positions/lengths. A source edit must retire the generation token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceIdentity {
    pub document: u128,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivativeProvenance {
    /// Differentiated source RHS, evaluated on the approximate integrated frame.
    SourceEquationAtIntegratedPose,
}

#[derive(Clone, Debug)]
pub struct Evaluation {
    pub source: SourceIdentity,
    pub section: SectionId,
    /// Index into this snapshot's source charts, not render/sample pieces.
    pub chart: usize,
    pub station_m: f64,
    pub section_input: f64,
    pub position_m: Vec3,
    pub frame: Frame,
    /// C', C'', C''' with respect to physical centerline station.
    pub position_derivatives: [Vec3; 3],
    /// World k, k', k'' per physical centerline station.
    pub angular_derivatives: [Vec3; 3],
    pub provenance: DerivativeProvenance,
    /// Fine/coarse differences, NOT certified bounds. In the asymptotic RK4
    /// regime with a true 2:1 grid ratio the fine error is approximately
    /// difference/15. Short spans/ceil rounding need not have that ratio. Includes accumulated
    /// checkpoint error, not just the query's local truncation error.
    pub refinement_position_difference_m: f64,
    pub refinement_orientation_difference_rad: f64,
    pub refinement_angular_differences: [f64; 3],
    pub local_rk4_steps: usize,
}
impl Evaluation {
    /// Constant local datum offset only; derivatives remain in centerline station,
    /// never offset-path arc length. This defines no wheel/contact surface.
    pub fn constant_offset(&self, offset_m: Vec3) -> Result<[Vec3; 4]> {
        ensure(offset_m.is_finite(), "nonfinite constant datum offset")?;
        let r = self.frame.local_to_world(offset_m);
        let [k, dk, ddk] = self.angular_derivatives;
        let [d, dd, ddd] = self.position_derivatives;
        let result = [
            self.position_m + r,
            d + k.cross(r),
            dd + dk.cross(r) + k.cross(k.cross(r)),
            ddd + ddk.cross(r)
                + dk.cross(k.cross(r)) * 2.
                + k.cross(dk.cross(r))
                + k.cross(k.cross(k.cross(r))),
        ];
        ensure(
            result[1].length() > 1e-12,
            "singular constant-offset datum tangent",
        )?;
        ensure(
            result.iter().all(|v| v.is_finite()),
            "constant offset jet overflow",
        )?;
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct Chart {
    pub section: SectionId,
    pub start_station_m: f64,
    pub end_station_m: f64,
    pub start_input: f64,
    pub end_input: f64,
    section_index: usize,
}

#[derive(Clone, Debug)]
pub struct Boundary {
    pub station_m: f64,
    pub left_chart: usize,
    pub right_chart: usize,
    /// Actual one-sided source-equation angular gaps at one shared approximate
    /// pose. Unlike pose convergence, these measure source compatibility.
    pub angular_gaps: [f64; 3],
    /// Absolute 1e-9 thresholds in rad/m, rad/m², rad/m³ respectively. Numerical
    /// classification at the prepared boundary pose, not an exact C3 proof.
    pub compatible: bool,
}

#[derive(Clone)]
struct Cell {
    x0: f64,
    x1: f64,
    a: State,
    b: State,
}

/// Immutable, nonpersisted source snapshot. All public queries require its token.
/// Charts end at every production integration/source knot and section boundary;
/// no periodic wrapping or rider-reference inverse is offered.
pub struct PreparedGraph {
    identity: SourceIdentity,
    initial: InitialConditions,
    sections: Vec<Section>,
    options: GenerationOptions,
    charts: Vec<Chart>,
    fine: Vec<Vec<Cell>>,
    coarse: Vec<Vec<Cell>>,
    boundaries: Vec<Boundary>,
}

impl PreparedGraph {
    pub fn prepare(
        identity: SourceIdentity,
        reference: AuthoredReference,
        initial: &InitialConditions,
        sections: &[Section],
        options: GenerationOptions,
    ) -> Result<Self> {
        ensure(
            identity.document != 0 && identity.generation != 0,
            "source identity must be nonzero",
        )?;
        ensure(
            reference == AuthoredReference::TrackCenterline,
            "prepared graph requires an authored physical centerline; rider inverse unsupported",
        )?;
        validate_track(initial, sections)?;
        options.validate()?;
        ensure(
            options.max_step_m >= 2e-6,
            "prepared refinement requires max_step_m >= 2e-6",
        )?;
        for section in sections {
            ensure(
                !matches!(
                    section.kind,
                    SectionKind::Spline { .. } | SectionKind::Geometric { .. }
                ),
                format!(
                    "region {}: native/world-geometric source jets unsupported",
                    section.id.0
                ),
            )?;
        }
        let coarse_track = generate(initial, sections, options)?;
        let fine_options = GenerationOptions {
            max_step_m: options.max_step_m * 0.5,
            ..options
        };
        let fine_track = generate(initial, sections, fine_options)?;
        let mut charts = Vec::new();
        let mut fine = Vec::new();
        let mut coarse = Vec::new();
        let mut station = crate::InputSum::default();
        let mut start = 0.;
        let mut range_index = 0;
        for (section_index, section) in sections.iter().enumerate() {
            let context = Context {
                section,
                speed: section
                    .reference_speed_mps
                    .unwrap_or(initial.reference_speed_mps),
                gravity: initial.gravity_mps2,
            };
            let scale = section.domain.metres_per_input(context.speed);
            for pair in context.knots()?.windows(2) {
                let chart = Chart {
                    section: section.id,
                    section_index,
                    start_station_m: start + pair[0] * scale,
                    end_station_m: start + pair[1] * scale,
                    start_input: pair[0],
                    end_input: pair[1],
                };
                ensure(
                    chart.end_station_m > chart.start_station_m,
                    "source station mapping lost precision",
                )?;
                coarse.push(cells(&coarse_track, range_index, &chart)?);
                fine.push(cells(&fine_track, range_index, &chart)?);
                charts.push(chart);
                range_index += 1;
            }
            start = station.add(section.extent * scale);
            // Use exactly the same station at adjoining section endpoints.
            charts
                .last_mut()
                .expect("validated section knots")
                .end_station_m = start;
        }
        let mut result = Self {
            identity,
            initial: initial.clone(),
            sections: sections.to_vec(),
            options,
            charts,
            fine,
            coarse,
            boundaries: Vec::new(),
        };
        for i in 1..result.charts.len() {
            let left = &result.charts[i - 1];
            let right = &result.charts[i];
            let cell = result.fine[i - 1].last().expect("nonempty chart");
            // Identical inherited pose isolates genuine source jet differences
            // from integration error. No epsilon queries across the boundary.
            let a = result
                .context(i - 1)
                .omega_jet(left.end_input, cell.b.q, Side::Left)?;
            let b = result
                .context(i)
                .omega_jet(right.start_input, cell.b.q, Side::Right)?;
            let gaps = std::array::from_fn(|j| (a[j] - b[j]).length());
            result.boundaries.push(Boundary {
                station_m: right.start_station_m,
                left_chart: i - 1,
                right_chart: i,
                angular_gaps: gaps,
                compatible: gaps.iter().all(|gap| *gap <= 1e-9),
            });
        }
        Ok(result)
    }
    pub fn charts(&self) -> &[Chart] {
        &self.charts
    }
    pub fn boundaries(&self) -> &[Boundary] {
        &self.boundaries
    }
    pub fn length_m(&self) -> f64 {
        self.charts.last().expect("validated graph").end_station_m
    }
    pub fn checkpoint_count(&self) -> usize {
        self.fine
            .iter()
            .chain(&self.coarse)
            .map(|c| c.len() + 1)
            .sum()
    }
    /// Exact source comparison includes IDs, controls, domain/speed and options.
    /// The caller still owns token retirement on edits/undo/replacement.
    pub fn matches_source(
        &self,
        initial: &InitialConditions,
        sections: &[Section],
        options: GenerationOptions,
    ) -> bool {
        self.initial == *initial && self.sections == sections && self.options == options
    }
    /// Reject any interval that touches/crosses an incompatible source boundary.
    /// Querying a single endpoint requires evaluate's explicit valid side instead.
    pub fn validate_interval(&self, expected: SourceIdentity, a: f64, b: f64) -> Result<()> {
        self.check(expected, a)?;
        self.check(expected, b)?;
        let lo = a.min(b);
        let hi = a.max(b);
        ensure(
            !self
                .boundaries
                .iter()
                .any(|j| !j.compatible && j.station_m >= lo && j.station_m <= hi),
            "interval touches a discontinuous source jet; use separate one-sided charts",
        )
    }
    pub fn evaluate(&self, expected: SourceIdentity, s: f64, side: Side) -> Result<Evaluation> {
        self.check(expected, s)?;
        ensure(
            !(s == 0. && side == Side::Left || s == self.length_m() && side == Side::Right),
            "requested side outside open source domain",
        )?;
        let i = self.charts.partition_point(|c| match side {
            Side::Left => c.end_station_m < s,
            Side::Right => c.end_station_m <= s,
        });
        let chart = &self.charts[i];
        let x = if s == chart.start_station_m {
            chart.start_input
        } else if s == chart.end_station_m {
            chart.end_input
        } else {
            chart.start_input
                + (s - chart.start_station_m) / (chart.end_station_m - chart.start_station_m)
                    * (chart.end_input - chart.start_input)
        };
        let (fine, steps_a) = self.pose(i, x, &self.fine[i])?;
        let (coarse, steps_b) = self.pose(i, x, &self.coarse[i])?;
        let context = self.context(i);
        let angular = context.omega_jet(x, fine.q, side)?;
        let old_angular = context.omega_jet(x, coarse.q, side)?;
        let frame = Frame::new(fine.q)?;
        let t = frame.tangent();
        let d = angular[0].cross(t);
        Ok(Evaluation {
            source: self.identity,
            section: chart.section,
            chart: i,
            station_m: s,
            section_input: x,
            position_m: fine.p,
            frame,
            position_derivatives: [t, d, angular[1].cross(t) + angular[0].cross(d)],
            angular_derivatives: angular,
            provenance: DerivativeProvenance::SourceEquationAtIntegratedPose,
            refinement_position_difference_m: (fine.p - coarse.p).length(),
            refinement_orientation_difference_rad: fine
                .q
                .compose(coarse.q.conjugate())
                .rotation_vector()?
                .length(),
            refinement_angular_differences: std::array::from_fn(|j| {
                (angular[j] - old_angular[j]).length()
            }),
            local_rk4_steps: steps_a + steps_b,
        })
    }
    fn check(&self, expected: SourceIdentity, s: f64) -> Result<()> {
        ensure(expected == self.identity, "stale source identity")?;
        ensure(
            s.is_finite() && s >= 0. && s <= self.length_m(),
            "station outside prepared physical source",
        )
    }
    fn context(&self, chart: usize) -> Context<'_> {
        let section = &self.sections[self.charts[chart].section_index];
        Context {
            section,
            speed: section
                .reference_speed_mps
                .unwrap_or(self.initial.reference_speed_mps),
            gravity: self.initial.gravity_mps2,
        }
    }
    fn pose(&self, chart: usize, x: f64, cells: &[Cell]) -> Result<(State, usize)> {
        let j = cells.partition_point(|c| c.x1 < x).min(cells.len() - 1);
        let c = &cells[j];
        if x == c.x0 {
            return Ok((c.a, 0));
        }
        if x == c.x1 {
            return Ok((c.b, 0));
        }
        ensure(x > c.x0 && x < c.x1, "query escaped retained source cell")?;
        Ok((self.context(chart).step(c.a, c.x0, x, self.options)?, 1))
    }
}

fn cells(track: &GeneratedTrack, range_index: usize, chart: &Chart) -> Result<Vec<Cell>> {
    let range = track
        .smooth_ranges
        .get(range_index)
        .ok_or_else(|| GeometryError("source/range mapping mismatch".into()))?;
    ensure(
        range.section_id == chart.section,
        "source section ownership mismatch",
    )?;
    let count = range.end_sample - range.start_sample;
    let state = |j: usize| {
        let p = &track.samples[j];
        State {
            p: p.position_m,
            q: p.frame.orientation(),
        }
    };
    Ok((0..count)
        .map(|j| {
            let input = |k| {
                if k == count {
                    chart.end_input
                } else {
                    chart.start_input
                        + (chart.end_input - chart.start_input) * (k as f64 / count as f64)
                }
            };
            Cell {
                x0: input(j),
                x1: input(j + 1),
                a: state(range.start_sample + j),
                b: state(range.start_sample + j + 1),
            }
        })
        .collect())
}
