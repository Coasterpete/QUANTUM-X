//! Reproducible numerical evidence and real offscreen editor captures.
//! Synthetic fixtures reconstruct the recorded action; no owner's project is changed.
use super::{
    graph_canvas, graph_view,
    session::{Session, scene},
};
use qx_document::{editing, persistence, region_edit, sample::foundation_project};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionKind},
    transition::{Channel, Function, Transition, Unit},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

fn project(t: Transition) -> qx_document::Project {
    let mut p = foundation_project();
    p.metadata.name = "Transition regression · synthetic fixture".into();
    let mut s = Section::straight(1, "Pitch profile · numerical fixture", 10.);
    s.kind = SectionKind::RiderLocal {
        pitch_rate: Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![t],
        },
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 10., 0.),
    };
    p.sections = vec![s];
    p
}
fn channel(p: &qx_document::Project) -> &Channel {
    p.sections[0]
        .channels()
        .into_iter()
        .find(|(n, _)| *n == "pitch")
        .unwrap()
        .1
}
fn capture(
    root: &Path,
    name: &str,
    p: &qx_document::Project,
    bounds: Option<[f64; 4]>,
) -> Result<(), Box<dyn Error>> {
    let catalog = qx_style::Catalog::built_in()?;
    persistence::save(&root.join(format!("{name}.qx.json")), p, &catalog)?;
    let mut session = Session::new(p.clone(), None, catalog)?;
    session.select_channel(ChannelKey::Pitch);
    graph_canvas::fit(&mut session);
    if let Some(bounds) = bounds {
        session
            .graph_view
            .set(graph_view::View::exact(bounds, 10.)?);
    }
    session.transition = usize::from(channel(p).transitions.len() > 1);
    session.status =
        "Synthetic regression fixture · offscreen capture; not an owner-authored coaster".into();
    let scene = scene(p, &session.catalog, session.options)?;
    println!(
        "{}",
        qx_rendering::render_offscreen(
            &scene,
            &mut session,
            &root.join(format!("{name}.png")),
            1600,
            1000
        )?
    );
    Ok(())
}
pub(crate) fn run(root: PathBuf) -> Result<(), Box<dyn Error>> {
    if root.exists() {
        return Err("choose a new transition-scenario output directory".into());
    }
    std::fs::create_dir_all(&root)?;
    let catalog = qx_style::Catalog::built_in()?;
    let original = project(Transition {
        id: 1,
        length: 10.,
        start: 0.012,
        end: 0.025,
        amplitude: 0.,
        function: Function::Hermite {
            start_slope: 0.002,
            end_slope: -0.001,
        },
    });
    let mut split = original.clone();
    split.sections[0] =
        editing::split_transition(&original.sections[0], ChannelKey::Pitch, 1, 0.3577)?;
    let before_geometry = original.generate(&catalog, Default::default())?;
    let after_geometry = split.generate(&catalog, Default::default())?;
    if before_geometry.samples.len() != after_geometry.samples.len() {
        return Err("split changed integration stations".into());
    }
    let mut errors = [0_f64; 5];
    let mut csv = String::from(
        "station_m,original_value,split_value,original_derivative,split_derivative,original_second,split_second\n",
    );
    for i in 0..=10000 {
        let x = i as f64 / 1000.;
        let a = channel(&original).evaluate(x)?;
        let b = channel(&split).evaluate(x)?;
        errors[0] = errors[0].max((a.value - b.value).abs());
        errors[1] = errors[1].max((a.derivative - b.derivative).abs());
        errors[2] = errors[2].max((a.second_derivative - b.second_derivative).abs());
        csv.push_str(&format!(
            "{x:.15e},{:.15e},{:.15e},{:.15e},{:.15e},{:.15e},{:.15e}\n",
            a.value, b.value, a.derivative, b.derivative, a.second_derivative, b.second_derivative
        ));
    }
    for (a, b) in before_geometry.samples.iter().zip(&after_geometry.samples) {
        errors[3] = errors[3].max((a.position_m - b.position_m).length());
    }
    for (a, b) in before_geometry.graphs.iter().zip(&after_geometry.graphs) {
        errors[4] = errors[4].max((a.curvature_per_m - b.curvature_per_m).abs());
    }
    if errors[0] > 1e-13
        || errors[1] > 1e-12
        || errors[2] > 1e-11
        || errors[3] > 1e-11
        || errors[4] > 1e-10
    {
        return Err(format!("split numeric regression: {errors:?}").into());
    }
    std::fs::write(root.join("split-samples.csv"), csv)?;
    let zero = project(Transition::constant(1, 10., 0.));
    let mut shared = zero.clone();
    shared.sections[0] =
        editing::split_transition(&zero.sections[0], ChannelKey::Pitch, 1, 0.3577)?;
    let value = 1.2488377508273916;
    let mut old_action = shared.clone();
    let t = &mut old_action.sections[0]
        .channel_mut(ChannelKey::Pitch)?
        .transitions[1];
    t.function = Function::Cosine {};
    t.start = value;
    t.end = 0.;
    let mut fixed = shared.clone();
    fixed.sections[0] =
        region_edit::endpoint(&shared.sections[0], ChannelKey::Pitch, 1, false, value)?;
    let boundary = |p: &qx_document::Project| -> Result<[f64; 3], Box<dyn Error>> {
        let c = channel(p);
        let a = c.transitions[0].evaluate(c.transitions[0].length)?;
        let b = c.transitions[1].evaluate(0.)?;
        Ok([
            b.value - a.value,
            b.derivative - a.derivative,
            b.second_derivative - a.second_derivative,
        ])
    };
    let old_jump = boundary(&old_action)?;
    let fixed_jump = boundary(&fixed)?;
    if fixed_jump.iter().any(|v| v.abs() > 1e-10) {
        return Err("shared C2 edit created a jump".into());
    }
    let mut shaped = split.clone();
    shaped.sections[0] =
        region_edit::shape_control(&split.sections[0], ChannelKey::Pitch, 1, 3, 0.030)?;
    let shaped_geometry = shaped.generate(&catalog, Default::default())?;
    // Independent planar quadrature: angle is integral(pitch rate ds), then
    // position is integral((cos angle, sin angle) ds). No solver/frame reuse.
    let mut angle = 0_f64;
    let mut position = qx_geometry::math::Vec3::ZERO;
    let step = 10. / 20000.;
    for i in 0..20000 {
        let rate = channel(&shaped).evaluate((i as f64 + 0.5) * step)?.value;
        let middle = angle + rate * step * 0.5;
        position.x += middle.cos() * step;
        position.y += middle.sin() * step;
        angle += rate * step;
    }
    let last = shaped_geometry
        .samples
        .last()
        .ok_or("missing generated end")?;
    let position_error = (last.position_m - position).length();
    let angle_error = (last.frame.tangent().y.asin() - angle).abs();
    let mut authored_error = 0_f64;
    let mut curvature_error = 0_f64;
    for g in &shaped_geometry.graphs {
        let rate = channel(&shaped).evaluate(g.input)?.value;
        authored_error = authored_error
            .max((g.authored_pitch_rate.ok_or("missing authored pitch")? - rate).abs());
        curvature_error = curvature_error.max((g.curvature_per_m - rate.abs()).abs());
    }
    if position_error > 1e-7
        || angle_error > 1e-8
        || authored_error > 1e-14
        || curvature_error > 2e-5
    {
        return Err(format!("shape graph/geometry mismatch: {position_error} m, {angle_error} rad, authored {authored_error}, curvature {curvature_error}").into());
    }
    let report = serde_json::json!({"fixture":"synthetic; reconstruction retained from the earlier recording diagnosis, not the owner's original document","split_station_m":3.577,"profile_samples":10001,"geometry_stations":before_geometry.samples.len(),"maximum_split_error":{"value_rad_per_m":errors[0],"derivative_rad_per_m2":errors[1],"second_derivative_rad_per_m3":errors[2],"position_m":errors[3],"curvature_per_m":errors[4]},"reconstructed_old_edit_jump":{"value_rad_per_m":old_jump[0],"derivative_rad_per_m2":old_jump[1],"second_derivative_rad_per_m3":old_jump[2]},"shared_C2_edit_jump":{"value_rad_per_m":fixed_jump[0],"derivative_rad_per_m2":fixed_jump[1],"second_derivative_rad_per_m3":fixed_jump[2]},"local_shape_check":{"independent_planar_position_error_m":position_error,"independent_integrated_pitch_error_rad":angle_error,"authored_graph_error_rad_per_m":authored_error,"midpoint_curvature_difference_per_m":curvature_error,"quadrature_steps":20000},"captures":"real wgpu/egui offscreen output; no active desktop input"});
    std::fs::write(
        root.join("numerical-evidence.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    capture(
        &root,
        "profile-before-split",
        &original,
        Some([0., 10., 0.005, 0.035]),
    )?;
    capture(
        &root,
        "profile-after-split",
        &split,
        Some([0., 10., 0.005, 0.035]),
    )?;
    capture(
        &root,
        "profile-after-local-shape-edit",
        &shaped,
        Some([0., 10., 0.005, 0.035]),
    )?;
    capture(
        &root,
        "recorded-action-before-fix",
        &old_action,
        Some([2.8, 4.5, -0.2, 1.5]),
    )?;
    capture(
        &root,
        "recorded-action-after-fix",
        &fixed,
        Some([2.8, 4.5, -0.2, 1.5]),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
