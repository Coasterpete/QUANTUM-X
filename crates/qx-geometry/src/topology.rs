//! Authored layout intent is separate from measured geometric closure.
use crate::{
    Result,
    authoring::{InitialConditions, Section},
    ensure,
    math::Vec3,
    solver::{GeneratedTrack, GenerationOptions, generate},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    Open,
    Shuttle,
    Circuit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closure {
    pub position_gap_m: f64,
    pub tangent_gap_rad: f64,
    pub frame_gap_rad: f64,
    pub valid: bool,
}
pub const POSITION_TOLERANCE_M: f64 = 0.001;
pub const ANGLE_TOLERANCE_RAD: f64 = 0.001;
pub fn diagnose(track: &GeneratedTrack) -> Result<Closure> {
    ensure(track.samples.len() >= 2, "closure requires generated track")?;
    let a = &track.samples[0];
    let b = track.samples.last().unwrap();
    let position_gap_m = (a.position_m - b.position_m).length();
    let tangent_gap_rad = a
        .frame
        .tangent()
        .cross(b.frame.tangent())
        .length()
        .atan2(a.frame.tangent().dot(b.frame.tangent()));
    let frame_gap_rad = a
        .frame
        .orientation()
        .conjugate()
        .compose(b.frame.orientation())
        .rotation_vector()?
        .length();
    let joins_valid = track.connections.iter().all(|c| {
        c.position_gap_m <= POSITION_TOLERANCE_M
            && c.tangent_angle_rad <= ANGLE_TOLERANCE_RAD
            && c.orientation_angle_rad <= ANGLE_TOLERANCE_RAD
    });
    Ok(Closure {
        position_gap_m,
        tangent_gap_rad,
        frame_gap_rad,
        valid: joins_valid
            && position_gap_m <= POSITION_TOLERANCE_M
            && tangent_gap_rad <= ANGLE_TOLERANCE_RAD
            && frame_gap_rad <= ANGLE_TOLERANCE_RAD,
    })
}
pub fn status(mode: LayoutMode, closure: Closure) -> &'static str {
    match mode {
        LayoutMode::Open => "Open layout",
        LayoutMode::Shuttle => "Shuttle (open ends)",
        LayoutMode::Circuit if closure.valid => "Valid geometric circuit",
        LayoutMode::Circuit => "Circuit incomplete",
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CompletionSettings {
    pub preferred_length_m: f64,
    pub length_tolerance_fraction: f64,
    pub minimum_radius_m: f64,
}
impl Default for CompletionSettings {
    fn default() -> Self {
        Self {
            preferred_length_m: 40.,
            length_tolerance_fraction: 0.35,
            minimum_radius_m: 2.,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Completion {
    pub section: Section,
    pub closure: Closure,
    pub length_m: f64,
    pub attempts: usize,
}
/// Bounded Hermite connector search. Existing sections are immutable. Endpoint
/// tangents are exact; transported frame holonomy is corrected by explicit roll.
/// This solves G1 pose closure, not G2/force continuity or obstacle avoidance.
pub fn complete(
    initial: &InitialConditions,
    sections: &[Section],
    mode: LayoutMode,
    id: u64,
    settings: CompletionSettings,
    options: GenerationOptions,
) -> Result<Completion> {
    ensure(
        mode != LayoutMode::Shuttle,
        "Shuttle intent has open ends; choose Circuit to request completion",
    )?;
    ensure(
        settings.preferred_length_m.is_finite()
            && (0.1..=5000.).contains(&settings.preferred_length_m)
            && settings.minimum_radius_m.is_finite()
            && (0.1..=10000.).contains(&settings.minimum_radius_m)
            && settings.length_tolerance_fraction.is_finite()
            && (0.01..=0.9).contains(&settings.length_tolerance_fraction),
        "invalid completion length, tolerance or minimum radius",
    )?;
    ensure(
        !sections.iter().any(|s| s.id.0 == id),
        "connector region ID already exists",
    )?;
    let source = generate(initial, sections, options)?;
    ensure(
        !diagnose(&source)?.valid,
        "track already meets geometric closure tolerances",
    )?;
    let start = source.samples.first().unwrap();
    let end = source.samples.last().unwrap();
    ensure(
        (start.position_m - end.position_m).length()
            <= settings.preferred_length_m * (1. + settings.length_tolerance_fraction),
        "endpoint gap exceeds requested connector length; increase the length or edit the layout",
    )?;
    let local = |p: Vec3| end.frame.world_to_local(p - end.position_m);
    let target = local(start.position_m);
    let target_t = end.frame.world_to_local(start.frame.tangent());
    let mut best: Option<(f64, Completion)> = None;
    let mut last_error = String::from("no candidate meets the length and radius bounds");
    // Fixed budget: 41 scales, at most two generation calls per scale.
    for i in 0..41 {
        let handle = settings.preferred_length_m * (0.05 + i as f64 * 0.025);
        let points = [
            Vec3::ZERO,
            Vec3::X * handle,
            target - target_t * handle,
            target,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, position_m)| crate::curve::ControlPoint {
            id: i as u64 + 1,
            position_m,
        })
        .collect();
        let mut section = Section::spline(
            id,
            "Circuit connector",
            crate::curve::Curve::Bezier { points },
        );
        let speed = source.sections.last().unwrap().reference_speed_mps;
        section.reference_speed_mps = Some(speed);
        let connector_initial = InitialConditions {
            position_m: end.position_m,
            orientation: end.frame.orientation(),
            reference_speed_mps: speed,
            gravity_mps2: initial.gravity_mps2,
        };
        let attempt = (|| -> Result<(GeneratedTrack, f64)> {
            let preview = generate(&connector_initial, &[section.clone()], options)?;
            let length = preview.length_m();
            ensure(
                (length - settings.preferred_length_m).abs()
                    <= settings.preferred_length_m * settings.length_tolerance_fraction,
                "connector length outside requested tolerance",
            )?;
            ensure(
                preview
                    .graphs
                    .iter()
                    .all(|g| g.curvature_per_m <= 1. / settings.minimum_radius_m),
                "connector violates minimum radius; edit endpoints or relax the radius",
            )?;
            let frame = preview.samples.last().unwrap().frame;
            let twist = start
                .frame
                .normal()
                .dot(frame.lateral())
                .atan2(start.frame.normal().dot(frame.normal()));
            section.roll_rate = crate::transition::Channel::constant(
                crate::transition::Unit::RadiansPerNormalized,
                1.,
                twist,
            );
            let mut candidate = sections.to_vec();
            candidate.push(section.clone());
            Ok((generate(initial, &candidate, options)?, length))
        })();
        match attempt {
            Ok((track, length_m)) => {
                let closure = diagnose(&track)?;
                if closure.valid {
                    let score = (length_m - settings.preferred_length_m).abs();
                    if best.as_ref().is_none_or(|(old, _)| score < *old) {
                        best = Some((
                            score,
                            Completion {
                                section,
                                closure,
                                length_m,
                                attempts: 41,
                            },
                        ));
                    }
                } else {
                    last_error = format!(
                        "frame residual {:.6} rad exceeds tolerance",
                        closure.frame_gap_rad
                    );
                }
            }
            Err(e) => last_error = e.to_string(),
        }
    }
    best.map(|(_, r)| r).ok_or_else(|| crate::GeometryError(format!("No feasible connector after 41 bounded candidates: {last_error}. Existing layout retained.")))
}
