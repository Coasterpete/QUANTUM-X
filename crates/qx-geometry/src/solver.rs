//! Spatial RK4 integration. See docs/core-foundation.md for equations/error limits.
use crate::{
    GeometryError, Result,
    authoring::{
        BoundaryPolicy, Domain, InitialConditions, Section, SectionId, SectionKind, validate_track,
    },
    ensure,
    math::{Frame, Quaternion, Vec3},
    transition::Side,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerationOptions {
    pub spline_arc_length: crate::arc_length::ArcLengthOptions,
    pub max_step_m: f64,
    pub max_samples: usize,
    pub max_rotation_per_step_rad: f64,
    pub curvature_warning_per_m: f64,
}
impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            spline_arc_length: Default::default(),
            max_step_m: 0.1,
            max_samples: 200_000,
            max_rotation_per_step_rad: 0.15,
            curvature_warning_per_m: 0.5,
        }
    }
}
impl GenerationOptions {
    pub(crate) fn validate(self) -> Result<()> {
        self.spline_arc_length.validate()?;
        ensure(
            self.max_step_m.is_finite() && (1e-6..=10.0).contains(&self.max_step_m),
            "max_step_m must be in [1e-6,10]",
        )?;
        ensure(
            (2..=1_000_000).contains(&self.max_samples),
            "sample budget must be in [2,1000000]",
        )?;
        ensure(
            self.max_rotation_per_step_rad.is_finite()
                && (1e-5..=0.5).contains(&self.max_rotation_per_step_rad),
            "invalid step rotation limit",
        )?;
        ensure(
            self.curvature_warning_per_m.is_finite() && self.curvature_warning_per_m > 0.0,
            "invalid curvature warning threshold",
        )
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub section_id: SectionId,
    pub input: Option<f64>,
    pub code: &'static str,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CenterlineSample {
    pub distance_m: f64,
    pub design_time_s: f64,
    pub position_m: Vec3,
    pub frame: Frame,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SectionRange {
    pub id: SectionId,
    pub start_sample: usize,
    pub end_sample: usize,
    pub start_distance_m: f64,
    pub end_distance_m: f64,
}
/// Measurements are at integration-interval midpoints. Derivatives come from
/// adjacent generated frames, independently of the authored force values.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphSample {
    pub section_id: SectionId,
    pub domain: Domain,
    pub input: f64,
    pub distance_m: f64,
    pub design_time_s: f64,
    pub elevation_m: f64,
    pub bank_rad: Option<f64>,
    pub pitch_rad: f64,
    pub yaw_rad: f64,
    pub curvature_per_m: f64,
    pub normal_g: f64,
    pub lateral_g: f64,
    pub roll_rate_rad_per_m: f64,
    pub pitch_rate_rad_per_m: Option<f64>,
    pub yaw_rate_rad_per_m: Option<f64>,
    pub reference_speed_mps: f64,
    pub authored_normal_g: Option<f64>,
    pub authored_lateral_g: Option<f64>,
    pub authored_pitch_rate: Option<f64>,
    pub authored_yaw_rate: Option<f64>,
    pub authored_roll_rate: f64,
    /// Frenet axes are undefined on a straight; the rider frame remains valid.
    pub frenet_normal: Option<Vec3>,
    pub frenet_binormal: Option<Vec3>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedTrack {
    pub samples: Vec<CenterlineSample>,
    pub graphs: Vec<GraphSample>,
    pub sections: Vec<SectionRange>,
    pub diagnostics: Vec<Diagnostic>,
    pub connections: Vec<SectionConnection>,
    pub numerical: Vec<SplineNumerics>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SplineNumerics {
    pub section_id: SectionId,
    pub length_m: f64,
    pub estimated_quadrature_error_m: f64,
    pub quadrature_evaluations: usize,
    pub table_stations: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SectionConnection {
    pub section_id: SectionId,
    pub position_gap_m: f64,
    pub tangent_angle_rad: f64,
    pub orientation_angle_rad: f64,
    pub curvature_vector_gap_per_m: f64,
    pub twist_rate_gap_rad_per_m: f64,
    pub reference_speed_gap_mps: f64,
}
impl GeneratedTrack {
    pub fn length_m(&self) -> f64 {
        self.samples.last().map_or(0.0, |p| p.distance_m)
    }
    /// Binary-search the integrated arc-distance coordinate. Hermite position
    /// interpolation uses endpoint tangents; orientation uses shortest slerp.
    /// This is a diagnostic/contact-frame query, not vehicle dynamics.
    pub fn sample_at_distance(&self, s: f64) -> Result<CenterlineSample> {
        ensure(
            s.is_finite() && s >= 0.0 && s <= self.length_m() && !self.samples.is_empty(),
            "distance query outside generated track",
        )?;
        let i = self.samples.partition_point(|p| p.distance_m < s);
        if i == 0 {
            return Ok(self.samples[0].clone());
        }
        if i == self.samples.len() {
            return Ok(self.samples[i - 1].clone());
        }
        let a = &self.samples[i - 1];
        let b = &self.samples[i];
        let h = b.distance_m - a.distance_m;
        let u = (s - a.distance_m) / h;
        let p = a.position_m * (2.0 * u * u * u - 3.0 * u * u + 1.0)
            + a.frame.tangent() * (h * (u * u * u - 2.0 * u * u + u))
            + b.position_m * (-2.0 * u * u * u + 3.0 * u * u)
            + b.frame.tangent() * (h * (u * u * u - u * u));
        Ok(CenterlineSample {
            distance_m: s,
            design_time_s: a.design_time_s + (b.design_time_s - a.design_time_s) * u,
            position_m: p,
            frame: Frame::new(a.frame.orientation().slerp(b.frame.orientation(), u)?)?,
        })
    }
}

#[derive(Clone, Copy)]
struct State {
    p: Vec3,
    q: Quaternion,
}
#[derive(Clone, Copy)]
struct Derivative {
    p: Vec3,
    q: Quaternion,
    omega: Vec3,
}
impl State {
    fn shifted(self, d: Derivative, h: f64) -> Self {
        Self {
            p: self.p + d.p * h,
            q: self.q.plus(d.q.scaled(h)),
        }
    }
}
#[derive(Clone, Copy)]
struct Context<'a> {
    section: &'a Section,
    speed: f64,
    gravity: f64,
}
impl Context<'_> {
    fn omega(self, x: f64, q: Quaternion, side: Side) -> Result<Vec3> {
        let frame = Frame::new(q)?;
        let t = frame.tangent();
        let n = frame.normal();
        let l = frame.lateral();
        let scale = self.section.domain.metres_per_input(self.speed);
        let value = |c: &crate::transition::Channel| c.evaluate_side(x, side).map(|e| e.value);
        let twist = value(&self.section.roll_rate)? / scale;
        let dt = match &self.section.kind {
            SectionKind::Spline { .. } => {
                return Err(GeometryError("spline uses its analytic evaluator".into()));
            }
            SectionKind::Straight {} => Vec3::ZERO,
            SectionKind::Arc {
                radius_m,
                plane_angle_rad,
            } => (n * plane_angle_rad.cos() + l * plane_angle_rad.sin()) / *radius_m,
            SectionKind::Force {
                normal_g,
                lateral_g,
            } => {
                let grav = Vec3::new(0.0, -self.gravity, 0.0);
                n * ((self.gravity * value(normal_g)? + grav.dot(n)) / (self.speed * self.speed))
                    + l * ((self.gravity * value(lateral_g)? + grav.dot(l))
                        / (self.speed * self.speed))
            }
            SectionKind::RiderLocal {
                pitch_rate,
                yaw_rate,
            } => n * (value(pitch_rate)? / scale) + l * (value(yaw_rate)? / scale),
            SectionKind::Geometric {
                pitch_rate,
                yaw_rate,
            } => {
                ensure(
                    (1.0 - t.y * t.y).max(0.0).sqrt() > 1e-6,
                    "world geometric pitch/yaw undefined near vertical; use rider-local controls",
                )?;
                let (up, _) = frame.horizontal_basis()?;
                up * (value(pitch_rate)? / scale) + t.cross(Vec3::Y) * (value(yaw_rate)? / scale)
            }
        };
        let omega = t.cross(dt) + t * twist;
        ensure(omega.is_finite(), "nonfinite curvature/twist")?;
        Ok(omega)
    }
    fn derivative(self, x: f64, state: State, side: Side) -> Result<Derivative> {
        let frame = Frame::new(state.q)?;
        let q = frame.orientation();
        let omega = self.omega(x, q, side)?;
        let angular = Quaternion {
            w: 0.0,
            x: omega.x,
            y: omega.y,
            z: omega.z,
        };
        Ok(Derivative {
            p: frame.tangent(),
            q: angular.compose(q).scaled(0.5),
            omega,
        })
    }
    fn step(self, state: State, x0: f64, x1: f64, options: GenerationOptions) -> Result<State> {
        let h = (x1 - x0) * self.section.domain.metres_per_input(self.speed);
        let mid = (x0 + x1) * 0.5;
        let a = self.derivative(x0, state, Side::Right)?;
        let b = self.derivative(mid, state.shifted(a, h * 0.5), Side::Right)?;
        let c = self.derivative(mid, state.shifted(b, h * 0.5), Side::Right)?;
        let d = self.derivative(x1, state.shifted(c, h), Side::Left)?;
        for v in [a, b, c, d] {
            ensure(
                v.omega.length() * h <= options.max_rotation_per_step_rad,
                "rotation per integration step too large; decrease max_step_m",
            )?;
        }
        let p = state.p + (a.p + b.p * 2.0 + c.p * 2.0 + d.p) * (h / 6.0);
        let q = state
            .q
            .plus(
                a.q.plus(b.q.scaled(2.0))
                    .plus(c.q.scaled(2.0))
                    .plus(d.q)
                    .scaled(h / 6.0),
            )
            .normalized()?;
        ensure(
            p.is_finite() && p.length() <= 1e9,
            "position overflow or world extent exceeds 1e9 m",
        )?;
        ensure(
            (p - state.p).length() > 0.0,
            "integration lost position precision; move origin or increase step",
        )?;
        Ok(State { p, q })
    }
}

pub fn generate(
    initial: &InitialConditions,
    sections: &[Section],
    options: GenerationOptions,
) -> Result<GeneratedTrack> {
    validate_track(initial, sections)?;
    options.validate()?;
    let mut state = State {
        p: initial.position_m,
        q: initial.orientation.normalized()?,
    };
    let mut result = GeneratedTrack {
        samples: vec![CenterlineSample {
            distance_m: 0.0,
            design_time_s: 0.0,
            position_m: state.p,
            frame: Frame::new(state.q)?,
        }],
        graphs: vec![],
        sections: vec![],
        diagnostics: vec![],
        connections: vec![],
        numerical: vec![],
    };
    let mut distance = 0.0;
    let mut time = 0.0;
    let mut previous: Option<(Vec3, f64)> = None;
    for section in sections {
        if matches!(section.kind, SectionKind::Spline { .. }) {
            let end =
                crate::spline_track::append(initial, section, options, &mut result, previous)?;
            let last = result.samples.last().expect("spline samples");
            state = State {
                p: last.position_m,
                q: last.frame.orientation(),
            };
            distance = last.distance_m;
            time = last.design_time_s;
            previous = Some(end);
            continue;
        }
        let context = Context {
            section,
            speed: section
                .reference_speed_mps
                .unwrap_or(initial.reference_speed_mps),
            gravity: initial.gravity_mps2,
        };
        let start_omega = context
            .omega(0.0, state.q, Side::Right)
            .map_err(|e| section_error(section, 0.0, e))?;
        record_connection(
            section,
            state.q,
            start_omega,
            context.speed,
            previous,
            &mut result,
        )?;
        if let Some((omega, speed)) = previous
            && ((omega - start_omega).length() > 1e-8 || (speed - context.speed).abs() > 1e-8)
        {
            ensure(
                section.incoming_boundary != BoundaryPolicy::RequireContinuousRates,
                format!(
                    "section {} requires continuous curvature/twist/reference speed at its start",
                    section.id.0
                ),
            )?;
            result.diagnostics.push(Diagnostic{section_id:section.id,input:Some(0.0),code:"section_rate_jump",message:"pose is continuous; curvature, twist, or reference speed jumps at the section boundary".into()});
        }
        let mut knots = vec![0.0, section.extent];
        for (name, channel) in section.channels() {
            knots.extend(
                channel
                    .knots()
                    .into_iter()
                    .filter(|x| *x > 0.0 && *x < section.extent),
            );
            for issue in channel.continuity()? {
                result.diagnostics.push(Diagnostic{section_id:section.id,input:Some(issue.input),code:"channel_discontinuity",message:format!("{name}: value jump {}, derivative jump {}; one-sided integration preserves pose",issue.value_jump,issue.derivative_jump)});
            }
        }
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| crate::same_input(*a, *b));
        // Retain the explicit endpoint when channel knots differ by roundoff.
        if let Some(last) = knots.last_mut() {
            *last = section.extent;
        }
        let start_sample = result.samples.len() - 1;
        let start_distance = distance;
        let mut warned_curvature = false;
        let mut warned_vertical = false;
        for pair in knots.windows(2) {
            let interval_m = (pair[1] - pair[0]) * section.domain.metres_per_input(context.speed);
            let count_float = (interval_m / options.max_step_m).ceil().max(4.0);
            ensure(
                count_float <= options.max_samples.saturating_sub(result.samples.len()) as f64,
                format!("section {} exceeds generation sample budget", section.id.0),
            )?;
            let count = count_float as usize;
            for i in 0..count {
                let x0 = pair[0] + (pair[1] - pair[0]) * (i as f64 / count as f64);
                let x1 = if i + 1 == count {
                    pair[1]
                } else {
                    pair[0] + (pair[1] - pair[0]) * ((i + 1) as f64 / count as f64)
                };
                let h = (x1 - x0) * section.domain.metres_per_input(context.speed);
                let next = context
                    .step(state, x0, x1, options)
                    .map_err(|e| section_error(section, x0, e))?;
                let a = result.samples.last().expect("initial sample exists");
                let b = CenterlineSample {
                    distance_m: distance + h,
                    design_time_s: time + h / context.speed,
                    position_m: next.p,
                    frame: Frame::new(next.q)?,
                };
                ensure(
                    b.distance_m > a.distance_m && b.design_time_s > a.design_time_s,
                    "distance/time accumulation lost precision",
                )?;
                let graph = measure(context, (x0 + x1) * 0.5, a, &b)?;
                if graph.curvature_per_m > options.curvature_warning_per_m && !warned_curvature {
                    warned_curvature = true;
                    result.diagnostics.push(Diagnostic {
                        section_id: section.id,
                        input: Some(graph.input),
                        code: "extreme_curvature",
                        message: format!(
                            "curvature exceeds {} /m (diagnostic threshold, not a safety limit)",
                            options.curvature_warning_per_m
                        ),
                    });
                }
                if graph.bank_rad.is_none() && !warned_vertical {
                    warned_vertical = true;
                    result.diagnostics.push(Diagnostic{section_id:section.id,input:Some(graph.input),code:"vertical_bank_undefined",message:"gravity-referenced banking is undefined at a vertical tangent; rider frame remains valid".into()});
                }
                distance = b.distance_m;
                time = b.design_time_s;
                state = next;
                result.graphs.push(graph);
                result.samples.push(b);
            }
        }
        previous = Some((
            context.omega(section.extent, state.q, Side::Left)?,
            context.speed,
        ));
        result.sections.push(SectionRange {
            id: section.id,
            start_sample,
            end_sample: result.samples.len() - 1,
            start_distance_m: start_distance,
            end_distance_m: distance,
        });
    }
    Ok(result)
}
fn section_error(section: &Section, x: f64, e: GeometryError) -> GeometryError {
    GeometryError(format!(
        "section {} ({}) at input {x}: {e}",
        section.id.0, section.name
    ))
}

pub(crate) fn measure_section(
    section: &Section,
    speed: f64,
    gravity: f64,
    x: f64,
    a: &CenterlineSample,
    b: &CenterlineSample,
) -> Result<GraphSample> {
    measure(
        Context {
            section,
            speed,
            gravity,
        },
        x,
        a,
        b,
    )
}
pub(crate) fn record_connection(
    section: &Section,
    q: Quaternion,
    omega: Vec3,
    speed: f64,
    previous: Option<(Vec3, f64)>,
    result: &mut GeneratedTrack,
) -> Result<()> {
    if let Some((old, old_speed)) = previous {
        let t = Frame::new(q)?.tangent();
        let curvature_vector_gap_per_m = (old.cross(t) - omega.cross(t)).length();
        let twist_rate_gap_rad_per_m = (omega - old).dot(t);
        ensure(
            section.incoming_boundary != BoundaryPolicy::RequireContinuousRates
                || ((old - omega).length() <= 1e-8 && (speed - old_speed).abs() <= 1e-8),
            format!(
                "section {} requires continuous curvature/twist/reference speed at its start",
                section.id.0
            ),
        )?;
        result.connections.push(SectionConnection {
            section_id: section.id,
            position_gap_m: 0.0,
            tangent_angle_rad: 0.0,
            orientation_angle_rad: 0.0,
            curvature_vector_gap_per_m,
            twist_rate_gap_rad_per_m,
            reference_speed_gap_mps: speed - old_speed,
        });
    }
    Ok(())
}

fn measure(
    context: Context<'_>,
    x: f64,
    a: &CenterlineSample,
    b: &CenterlineSample,
) -> Result<GraphSample> {
    let h = b.distance_m - a.distance_m;
    let frame = Frame::new(a.frame.orientation().slerp(b.frame.orientation(), 0.5)?)?;
    let t = frame.tangent();
    let dt = (b.frame.tangent() - a.frame.tangent()) / h;
    let curvature = dt.length();
    let fnormal = if curvature > 1e-10 {
        Some(dt / curvature)
    } else {
        None
    };
    let omega = b
        .frame
        .orientation()
        .compose(a.frame.orientation().conjugate())
        .rotation_vector()?
        / h;
    let force = dt * (context.speed * context.speed) - Vec3::new(0.0, -context.gravity, 0.0);
    let value = |c: &crate::transition::Channel| c.evaluate_side(x, Side::Right).map(|e| e.value);
    let (normal, lateral, pitch, yaw) = match &context.section.kind {
        SectionKind::Force {
            normal_g,
            lateral_g,
        } => (Some(value(normal_g)?), Some(value(lateral_g)?), None, None),
        SectionKind::Geometric {
            pitch_rate,
            yaw_rate,
        }
        | SectionKind::RiderLocal {
            pitch_rate,
            yaw_rate,
        } => (None, None, Some(value(pitch_rate)?), Some(value(yaw_rate)?)),
        _ => (None, None, None, None),
    };
    let (dp, dy) = if (1.0 - t.y * t.y).max(0.0).sqrt() > 1e-6 {
        let (up, _) = frame.horizontal_basis()?;
        let yaw_direction = t.cross(Vec3::Y);
        (
            Some(dt.dot(up)),
            Some(dt.dot(yaw_direction) / yaw_direction.dot(yaw_direction)),
        )
    } else {
        (None, None)
    };
    let graph = GraphSample {
        section_id: context.section.id,
        domain: context.section.domain,
        input: x,
        distance_m: (a.distance_m + b.distance_m) * 0.5,
        design_time_s: (a.design_time_s + b.design_time_s) * 0.5,
        elevation_m: (a.position_m.y + b.position_m.y) * 0.5,
        bank_rad: frame.bank_rad(),
        pitch_rad: t.y.clamp(-1.0, 1.0).asin(),
        yaw_rad: t.z.atan2(t.x),
        curvature_per_m: curvature,
        normal_g: force.dot(frame.normal()) / context.gravity,
        lateral_g: force.dot(frame.lateral()) / context.gravity,
        roll_rate_rad_per_m: omega.dot(t),
        pitch_rate_rad_per_m: dp,
        yaw_rate_rad_per_m: dy,
        reference_speed_mps: context.speed,
        authored_normal_g: normal,
        authored_lateral_g: lateral,
        authored_pitch_rate: pitch,
        authored_yaw_rate: yaw,
        authored_roll_rate: value(&context.section.roll_rate)?,
        frenet_normal: fnormal,
        frenet_binormal: fnormal.map(|n| t.cross(n)),
    };
    ensure(
        [
            graph.curvature_per_m,
            graph.normal_g,
            graph.lateral_g,
            graph.roll_rate_rad_per_m,
        ]
        .iter()
        .all(|v| v.is_finite()),
        "measurement overflow",
    )?;
    Ok(graph)
}
