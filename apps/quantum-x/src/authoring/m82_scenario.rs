//! Noninteractive acceptance through the same playback worker and viewport as the editor.
use super::{
    session::{Session, blank_project, scene},
    train_assets,
    train_playback::{Action, Snapshot},
    train_preview,
    workbench::Profile,
};
use qx_document::{
    command::Command,
    persistence,
    train::{ComponentKind, DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::Section,
    math::{Quaternion, Vec3},
};
use serde_json::{Value, json};
use std::{
    error::Error,
    path::PathBuf,
    time::{Duration, Instant},
};
fn wait(s: &mut Session, ticks: u64) -> Result<(), Box<dyn Error>> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        s.playback.sync(s.editor.project(), s.options);
        if let Some(e) = &s.playback.error {
            return Err(e.clone().into());
        }
        if s.playback
            .snapshot
            .as_ref()
            .is_some_and(|p| p.ticks == ticks)
        {
            return Ok(());
        }
        if Instant::now() >= until {
            return Err("playback worker timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn record(s: &Session, p: &Snapshot, display: &qx_rendering::DisplayOptions) -> Value {
    let diagnostics=p.diagnostics.as_ref().map(|d|json!({"joint_error_m":d.max_joint_error_m,"guide_error_m":d.max_guide_error_m,"angular_error_rad":d.max_angular_error_rad,"joint_force_n":d.joint_force_on_b_n,"guide_force_n":d.guide_force_n,"angular_torque_nm":d.angular_torque_on_b_nm}));
    json!({"ticks":p.ticks,"time_s":p.ticks as f64*qx_physics::FIXED_STEP_SECONDS,"running":p.running,
        "components":p.poses.components.iter().map(|c|json!({"id":c.id,"station_m":c.station_m,"position_m":c.pose.position_m,"rotation":c.pose.orientation})).collect::<Vec<_>>(),
        "joints":p.poses.joints.iter().map(|j|json!({"id":j.id,"a":j.anchor_a_m,"b":j.anchor_b_m,"error_m":j.error_m})).collect::<Vec<_>>(),
        "speed_mps":p.speed_mps,"diagnostics":diagnostics,"train":s.editor.project().trains[0],
        "instances":display.asset_instances.iter().map(|i|json!({"id":i.component_id,"matrix":i.transform,"bounds":i.bounds()})).collect::<Vec<_>>()})
}
pub(super) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&output)?;
    let fixtures = super::library_ui::assets_root().join("development/train");
    let mut cases = vec![];
    for (name, arrangement) in [
        ("conventional", DevelopmentArrangement::Conventional),
        ("below-track", DevelopmentArrangement::BelowTrack),
        ("rotating-seats", DevelopmentArrangement::RotatingSeats),
    ] {
        let dir = output.join(name);
        std::fs::create_dir_all(&dir)?;
        let mut project = blank_project();
        project.metadata.name = format!("M8.2 {name} development regression fixture");
        project.sections[0] = Section::straight(1, "Synthetic 0.2 rad incline", 100.);
        project.initial.orientation = Quaternion::axis_angle(Vec3::Z, -0.2)?;
        project.trains.push(Train::development_consist(
            "vehicle".into(),
            3,
            arrangement,
        )?);
        project.heartline.train_configuration_id = Some("vehicle".into());
        let mut s = Session::new(project, None, qx_style::Catalog::built_in()?)?;
        let body = s
            .assets
            .import_now(&fixtures.join("development-blender-body.glb"), &s.path)?;
        let gear = s
            .assets
            .import_now(&fixtures.join("development-bogie.glb"), &s.path)?;
        let mut trains = s.editor.project().trains.clone();
        for c in &mut trains[0].components {
            let mut a = train_assets::identity(if c.kind.is_running_gear() {
                gear.clone()
            } else {
                body.clone()
            });
            if c.kind == ComponentKind::PassengerAssembly {
                a.scale = Vec3::new(0.3, 0.7, 0.7);
                a.translation_m.y = -0.4;
            }
            c.asset = Some(a);
        }
        if !s.execute(vec![Command::SetTrains(trains)]) {
            return Err(s.status.into());
        }
        let path = dir.join("playback.qx.json");
        s.save_to(&path);
        if let Some(e) = s.save_failure {
            return Err(e.into());
        }
        let saved = s.editor.project().clone();
        if persistence::load(&path, &s.catalog)? != saved {
            return Err("M8.2 save/reopen mismatch".into());
        }
        let captured = scene(s.editor.project(), &s.catalog, s.options)?;
        s.visible_revision = s.revision;
        s.workbench.active = Profile::Train;
        s.train_station_m = Some(50.);
        s.playback.initial_speed_mps = 2.;
        s.playback
            .initialize(s.editor.project(), s.options, &captured.track, 50.);
        wait(&mut s, 0)?;
        let mut records = vec![];
        let mut maximum_joint: f64 = 0.;
        let mut maximum_guide: f64 = 0.;
        let mut maximum_angular: f64 = 0.;
        for tick in [0, 120] {
            if tick > 0 {
                for step in 1..=tick {
                    s.playback.action(Action::Step);
                    wait(&mut s, step)?;
                    let d = s
                        .playback
                        .snapshot
                        .as_ref()
                        .unwrap()
                        .diagnostics
                        .as_ref()
                        .unwrap();
                    maximum_joint = maximum_joint.max(d.max_joint_error_m);
                    maximum_guide = maximum_guide.max(d.max_guide_error_m);
                    maximum_angular = maximum_angular.max(d.max_angular_error_rad);
                }
            }
            let snapshot = s.playback.snapshot.as_ref().unwrap().clone();
            let mut display = Default::default();
            train_preview::populate(
                &saved.trains[0],
                &captured.track,
                &snapshot.poses,
                &mut s.assets,
                &mut display,
            );
            let mut r = record(&s, &snapshot, &display);
            let file = format!("tick-{tick}.png");
            let report =
                qx_rendering::render_offscreen(&captured, &mut s, &dir.join(&file), 1440, 960)?;
            r["capture"] = json!(file);
            r["render_report"] = json!(report);
            records.push(r);
        }
        if maximum_joint > 0.02 || maximum_guide > 0.02 || maximum_angular > 0.02 {
            return Err("M8.2 fixture constraint residual exceeded 0.02".into());
        }
        s.playback.action(Action::Reset);
        wait(&mut s, 0)?;
        if s.editor.project() != &saved {
            return Err("playback changed persisted authoring state".into());
        }
        let compact = "reset-960x640.png";
        qx_rendering::render_offscreen(&captured, &mut s, &dir.join(compact), 960, 640)?;
        cases.push(json!({"name":name,"project":format!("{name}/playback.qx.json"),"records":records,"compact_capture":compact,"maximum_joint_m":maximum_joint,"maximum_guide_m":maximum_guide,"maximum_angular_rad":maximum_angular}));
        println!(
            "M8.2 {name}: 3 cars, worker playback/reset, saved assets, joint {maximum_joint:.8} m / guide {maximum_guide:.8} m / angular {maximum_angular:.8} rad"
        );
    }
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"automated_offscreen":true,"manual_desktop_tested":false,"fixed_dt_s":qx_physics::FIXED_STEP_SECONDS,"cases":cases}),
        )?,
    )?;
    Ok(())
}
