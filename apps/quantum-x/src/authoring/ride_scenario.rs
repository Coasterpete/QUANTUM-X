//! Authentic offscreen Editor captures. Input track/geometry/assets are never
//! rewritten. A missing train is supplied only in the explicit output fixture.
use super::{
    ride_camera::Mode,
    session::{Session, scene},
    workbench::Profile,
};
use qx_document::{
    persistence,
    train::{DevelopmentArrangement, Train},
};
use serde_json::json;
use std::{error::Error, path::PathBuf};

pub(super) fn record(s: &Session, display: &qx_rendering::DisplayOptions) -> serde_json::Value {
    let poses = s.ride.presentation.as_ref().unwrap();
    json!({"camera":display.camera_view.map(|v|json!({"eye_m":v.eye_m,"forward":v.forward,"up":v.up})),
        "components":poses.components.iter().map(|p|json!({"id":p.id,"position_m":p.pose.position_m,"orientation":p.pose.orientation,"station_m":p.station_m})).collect::<Vec<_>>(),
        "joints":poses.joints.iter().map(|j|json!({"id":j.id,"error_m":j.error_m})).collect::<Vec<_>>(),
        "instances":display.asset_instances.iter().map(|i|json!({"id":i.component_id,"matrix":i.transform})).collect::<Vec<_>>()})
}
pub fn run(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("choose a new ride evidence directory".into());
    }
    let catalog = qx_style::Catalog::built_in()?;
    let input_bytes = std::fs::read(&input)?;
    let mut project = persistence::load(&input, &catalog)?;
    let original_track = (
        project.initial.clone(),
        project.sections.clone(),
        project.construction.clone(),
    );
    let needs_association = project
        .heartline
        .train_configuration_id
        .as_ref()
        .is_none_or(|id| !project.trains.iter().any(|t| &t.id == id));
    let added_train = needs_association && project.trains.is_empty();
    if needs_association && !project.trains.is_empty() {
        project.heartline.train_configuration_id = Some(project.trains[0].id.clone());
    } else if added_train {
        let id = "ride-capture-development-fixture".to_string();
        if project.trains.iter().any(|t| t.id == id) {
            return Err("capture fixture ID already exists; associate a train first".into());
        }
        project.trains.push(Train::development_consist(
            id.clone(),
            3,
            DevelopmentArrangement::Conventional,
        )?);
        project.heartline.train_configuration_id = Some(id);
    }
    std::fs::create_dir_all(&output)?;
    let mut s = Session::new(project, Some(&input), catalog)?;
    s.assets.load_blocking(s.editor.project(), &s.path);
    s.save_to(&output.join("inspection.qx.json"));
    if let Some(e) = &s.save_failure {
        return Err(e.clone().into());
    }
    let saved = s.editor.project().clone();
    if persistence::load(&output.join("inspection.qx.json"), &s.catalog)? != saved {
        return Err("ride fixture save/reopen mismatch".into());
    }
    if original_track
        != (
            saved.initial.clone(),
            saved.sections.clone(),
            saved.construction.clone(),
        )
    {
        return Err("ride scenario changed authored geometry".into());
    }
    let captured = scene(&saved, &s.catalog, s.options)?;
    s.visible_revision = s.revision;
    s.frame_requested = false;
    s.workbench.active = Profile::Train;
    let station = captured.track.length_m() * 0.5;
    s.train_station_m = Some(station);
    let ctx = egui::Context::default();
    let mut display = qx_rendering::DisplayOptions::default();
    let mut records = vec![];
    for (name, mode) in [
        ("free", Mode::Free),
        ("rider", Mode::Rider),
        ("front", Mode::Front),
        ("rear", Mode::Rear),
        ("chase", Mode::Chase),
        ("track", Mode::Track),
    ] {
        s.ride.mode = mode;
        let mut pass = ctx.run_pass(Default::default(), |ctx| {
            super::ride_camera::present(&mut s, ctx, &captured, &mut display)
        });
        pass.textures_delta.clear();
        if let Some(e) = &s.ride.error {
            return Err(e.clone().into());
        }
        let mut evidence = record(&s, &display);
        let report = qx_rendering::render_offscreen(
            &captured,
            &mut s,
            &output.join(format!("{name}.png")),
            1440,
            960,
        )?;
        evidence["mode"] = json!(name);
        evidence["report"] = json!(report);
        records.push(evidence);
    }
    super::ride_quality::record(output.join("inspection.qx.json"), output.join("pov60"))?;
    let recording: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("pov60/evidence.json"))?)?;
    let mut frames = recording["frames"]
        .as_array()
        .ok_or("missing recording frames")?
        .clone();
    for frame in &mut frames {
        frame["file"] = json!(format!(
            "pov60/{}",
            frame["file"].as_str().ok_or("missing frame path")?
        ));
    }
    if s.editor.project() != &saved || std::fs::read(&input)? != input_bytes {
        return Err("capture altered the input or authored state".into());
    }
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"source":input,"offscreen":true,"native_desktop_tested":false,"development_train_added":added_train,"project_roundtrip":true,"input_unchanged":true,"dt_s":qx_physics::FIXED_STEP_SECONDS,"fps":60,"records":records,"frames":frames,"recording":recording}),
        )?,
    )?;
    println!(
        "Ride camera evidence: {} (six camera modes and authentic 60 FPS independent-physics POV frames; input unchanged)",
        output.display()
    );
    Ok(())
}
