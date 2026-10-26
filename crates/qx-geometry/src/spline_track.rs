//! Spline adapter into the shared track output. Minimal tangent rotations
//! approximate Bishop transport; explicit roll is integrated separately in xi=s/L.
use crate::{
    Result,
    arc_length::ArcLengthTable,
    authoring::{InitialConditions, Section, SectionKind},
    ensure,
    math::{Frame, Quaternion, Vec3, rotation_between},
    solver::{
        CenterlineSample, Diagnostic, GeneratedTrack, GenerationOptions, SectionRange,
        SplineNumerics, measure_section, record_connection,
    },
    transition::Side,
};

pub(crate) fn append(
    initial: &InitialConditions,
    section: &Section,
    options: GenerationOptions,
    result: &mut GeneratedTrack,
    previous: Option<(Vec3, f64)>,
) -> Result<(Vec3, f64)> {
    let SectionKind::Spline { curve } = &section.kind else {
        unreachable!()
    };
    let prepared = curve.prepare()?;
    let table = ArcLengthTable::build(&prepared, options.spline_arc_length)?;
    let length = table.length_m();
    let speed = section
        .reference_speed_mps
        .unwrap_or(initial.reference_speed_mps);
    let start = result.samples.last().expect("initial sample").clone();
    let anchor = start.frame;
    let (u0, u1) = prepared.domain();
    let omega_at = |u: f64, xi: f64, frame: Frame, side: Side| -> Result<Vec3> {
        let e = prepared.evaluate(u, side)?;
        let curvature = anchor.local_to_world(e.curvature_vector()?);
        let twist = section.roll_rate.evaluate_side(xi, side)?.value / length;
        Ok(frame.tangent().cross(curvature) + frame.tangent() * twist)
    };
    let start_omega = omega_at(u0, 0.0, anchor, Side::Right)?;
    record_connection(
        section,
        start.frame.orientation(),
        start_omega,
        speed,
        previous,
        result,
    )?;
    if let Some((old, v)) = previous
        && ((old - start_omega).length() > 1e-8 || (v - speed).abs() > 1e-8)
    {
        result.diagnostics.push(Diagnostic {
            section_id: section.id,
            input: Some(0.0),
            code: "section_rate_jump",
            message:
                "pose is inherited; spline curvature/twist/reference speed jumps at the boundary"
                    .into(),
        });
    }
    result.numerical.push(SplineNumerics {
        section_id: section.id,
        length_m: length,
        estimated_quadrature_error_m: table.estimated_error_m,
        quadrature_evaluations: table.evaluations,
        table_stations: table.stations.len(),
    });
    for join in prepared.continuity()? {
        if !join.g2(1e-8) {
            result.diagnostics.push(Diagnostic {
                section_id: section.id,
                input: Some(table.distance_at_parameter(&prepared, join.parameter)? / length),
                code: "spline_curvature_jump",
                message: format!(
                    "native u={}: G1={}, G2={}, C1={}, C2={}; curvature-vector gap {} /m",
                    join.parameter,
                    join.g1(1e-8),
                    join.g2(1e-8),
                    join.c1(1e-8),
                    join.c2(1e-8),
                    join.curvature_vector_gap_per_m
                ),
            });
        }
    }
    let mut boundaries = section.roll_rate.knots();
    for &u in prepared.breaks() {
        boundaries.push(table.distance_at_parameter(&prepared, u)? / length);
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| crate::same_input(*a, *b));
    *boundaries.last_mut().expect("spline boundaries") = 1.0;
    for issue in section.roll_rate.continuity()? {
        result.diagnostics.push(Diagnostic {
            section_id: section.id,
            input: Some(issue.input),
            code: "channel_discontinuity",
            message: format!(
                "roll per normalized distance: value jump {}, derivative jump {}",
                issue.value_jump, issue.derivative_jump
            ),
        });
    }
    let start_index = result.samples.len() - 1;
    let mut warned = false;
    for pair in boundaries.windows(2) {
        let smooth_start = result.samples.len() - 1;
        let count = (((pair[1] - pair[0]) * length / options.max_step_m).ceil() as usize).max(4);
        ensure(
            count <= options.max_samples.saturating_sub(result.samples.len()),
            "spline exceeds generation sample budget",
        )?;
        for i in 0..count {
            let a_x = pair[0] + (pair[1] - pair[0]) * (i as f64 / count as f64);
            let b_x = if i + 1 == count {
                pair[1]
            } else {
                pair[0] + (pair[1] - pair[0]) * ((i + 1) as f64 / count as f64)
            };
            let mid = (a_x + b_x) * 0.5;
            let u = table.parameter_at_distance(&prepared, (b_x * length).min(length))?;
            let um = table.parameter_at_distance(&prepared, mid * length)?;
            let e = prepared.evaluate(u, Side::Left)?;
            let em = prepared.evaluate(um, Side::Right)?;
            let t = anchor.local_to_world(e.tangent()?);
            let tm = anchor.local_to_world(em.tangent()?);
            let last = result.samples.last().expect("previous spline sample");
            // Two minimal rotations through the midpoint reduce transport error
            // while keeping exact analytic tangents and avoiding Frenet singularities.
            let r0 = rotation_between(last.frame.tangent(), tm)?;
            let r1 = rotation_between(tm, t)?;
            let roll = |x, side| section.roll_rate.evaluate_side(x, side).map(|e| e.value);
            let rates = [
                roll(a_x, Side::Right)?,
                roll(mid, Side::Right)?,
                roll(b_x, Side::Left)?,
            ];
            let twist = (b_x - a_x) * (rates[0] + 4.0 * rates[1] + rates[2]) / 6.0;
            let twist_guard = rates.iter().map(|r| r.abs()).fold(0.0, f64::max) * (b_x - a_x);
            let angle =
                r0.rotation_vector()?.length() + r1.rotation_vector()?.length() + twist_guard;
            ensure(
                angle <= options.max_rotation_per_step_rad,
                "spline tangent/twist turns too far per step or has a cusp; decrease max_step_m",
            )?;
            let q = Quaternion::axis_angle(t, twist)?
                .compose(r1)
                .compose(r0)
                .compose(last.frame.orientation())
                .normalized()?;
            let next = CenterlineSample {
                distance_m: start.distance_m + b_x * length,
                design_time_s: start.design_time_s + b_x * length / speed,
                position_m: start.position_m + anchor.local_to_world(e.position_m),
                frame: Frame::new(q)?,
            };
            ensure(
                next.position_m.is_finite()
                    && next.position_m.length() <= 1e9
                    && next.distance_m > last.distance_m
                    && next.design_time_s > last.design_time_s,
                "spline world bounds or accumulation precision exceeded",
            )?;
            ensure(
                (next.position_m - last.position_m).length() > 0.0,
                "spline sampling lost world-position precision; move origin or increase step",
            )?;
            let graph = measure_section(section, speed, initial.gravity_mps2, mid, last, &next)?;
            if graph.curvature_per_m > options.curvature_warning_per_m && !warned {
                result.diagnostics.push(Diagnostic{section_id:section.id,input:Some(mid),code:"extreme_curvature",message:"spline exceeds configured curvature diagnostic threshold (not a safety limit)".into()});
                warned = true;
            }
            result.graphs.push(graph);
            result.samples.push(next);
        }
        result.smooth_ranges.push(crate::solver::SmoothRange {
            section_id: section.id,
            start_sample: smooth_start,
            end_sample: result.samples.len() - 1,
        });
    }
    let frame = result.samples.last().expect("spline endpoint").frame;
    let end_omega = omega_at(u1, 1.0, frame, Side::Left)?;
    result.sections.push(SectionRange {
        id: section.id,
        start_sample: start_index,
        end_sample: result.samples.len() - 1,
        start_distance_m: start.distance_m,
        end_distance_m: start.distance_m + length,
        end_angular_rate_rad_per_m: end_omega,
        reference_speed_mps: speed,
    });
    Ok((end_omega, speed))
}
