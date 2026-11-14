//! Explicit development fixture captured through the actual Editor and PBR
//! renderer. No desktop input, image synthesis, or composited interface.
use super::{
    ride_camera::Mode,
    session::{Session, blank_project, scene},
    train_playback::Snapshot,
    workbench::Profile,
};
use qx_document::{
    operations::{DeviceKind, TrackDevice},
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{Section, SectionKind},
    supports::*,
    topology::{CompletionSettings, LayoutMode},
};
use qx_rendering::{DisplayOptions, EditorSurface};
use serde_json::json;
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

fn capture(
    s: &mut Session,
    sc: &qx_rendering::InspectionScene,
    output: &Path,
    name: &str,
) -> Result<String, String> {
    s.visible_revision = s.revision;
    s.visible_preview = false;
    s.frame_requested = s.ride.mode == Mode::Free;
    qx_rendering::render_offscreen(sc, s, &output.join(name), 2000, 1400)
}
pub(crate) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("Choose a new integrated evidence directory".into());
    }
    std::fs::create_dir_all(&output)?;
    let catalog = qx_style::Catalog::built_in()?;
    let mut p = blank_project();
    p.metadata.name = "Integrated core verification fixture (not a designed ride)".into();
    p.initial.position_m.y = 12.;
    p.initial.reference_speed_mps = 10.;
    let mut arc = Section::straight(
        1,
        "Analytic 270 degree horizontal arc",
        30. * std::f64::consts::PI,
    );
    arc.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    p.sections = vec![arc];
    let train = Train::development_consist(
        "verification-train".into(),
        3,
        DevelopmentArrangement::Conventional,
    )?;
    p.heartline.train_configuration_id = Some(train.id.clone());
    p.trains.push(train.clone());
    let mut s = Session::new(p.clone(), None, catalog.clone())?;
    s.track_tab = 4;
    s.expansion.engineering = true;
    s.expansion.engineering_channel = 8;
    let initial = scene(&p, &catalog, s.options)?;
    let mut reports = vec![];
    reports.push(capture(
        &mut s,
        &initial,
        &output,
        "01-engineering-plots.png",
    )?);
    reports.push(capture(
        &mut s,
        &initial,
        &output,
        "02-closure-diagnostics.png",
    )?);
    let connector = qx_geometry::topology::complete(
        &p.initial,
        &p.sections,
        LayoutMode::Circuit,
        2,
        CompletionSettings {
            preferred_length_m: 31.5,
            minimum_radius_m: 10.,
            ..Default::default()
        },
        s.options,
    )?;
    s.expansion.completion = Some((p.sections.clone(), p.initial.clone(), connector));
    s.expansion.operations.layout = LayoutMode::Circuit;
    if !s.apply_expansion() {
        return Err(format!("completion apply failed: {:?}", s.expansion.message).into());
    }
    let completed = s.editor.project().clone();
    let closed = scene(&completed, &catalog, s.options)?;
    reports.push(capture(
        &mut s,
        &closed,
        &output,
        "03-completed-circuit.png",
    )?);
    s.undo(false);
    if s.editor.project() != &p {
        return Err("completion undo mismatch".into());
    }
    s.undo(true);
    let attachment = Attachment::at_station(
        &closed.track,
        18.,
        qx_geometry::math::Vec3::new(0., -0.5, 0.),
    )?;
    let mut manual = Structure::empty(1);
    manual.name = "Authored circular / rectangular portal".into();
    for (id, lateral, foundation) in [
        (1, -2., true),
        (2, -2., false),
        (3, 2., true),
        (4, 2., false),
    ] {
        let mut a = attachment.clone();
        a.offset_m.z = lateral;
        manual.nodes.push(Node {
            id,
            name: format!("Portal node {id}"),
            position: if foundation {
                NodePosition::Foundation {
                    attachment: a,
                    elevation_m: 0.,
                }
            } else {
                NodePosition::Track { attachment: a }
            },
        });
    }
    for (id, start_node, end_node, profile) in [
        (1, 1, 2, MemberProfile::Circular { diameter_m: 0.35 }),
        (2, 3, 4, MemberProfile::Circular { diameter_m: 0.35 }),
        (
            3,
            2,
            4,
            MemberProfile::Rectangular {
                width_m: 0.4,
                height_m: 0.45,
            },
        ),
    ] {
        manual.members.push(Member {
            id,
            start_node,
            end_node,
            profile,
            roll_rad: 0.,
        });
    }
    s.expansion.supports.push(manual);
    s.expansion.structure = Some(1);
    s.expansion.member = Some(3);
    if !s.apply_expansion() {
        return Err(format!("support apply: {:?}", s.expansion.message).into());
    }
    s.expansion.engineering = true;
    s.expansion.engineering_channel = 1;
    let manual_scene = scene(s.editor.project(), &catalog, s.options)?;
    reports.push(capture(
        &mut s,
        &manual_scene,
        &output,
        "04-authored-support.png",
    )?);
    let recipe = TimberRecipe {
        family: TimberFamily::HybridLattice,
        start_m: 25.,
        end_m: 80.,
        spacing_m: 4.,
        foundation_elevation_m: 0.,
        ..Default::default()
    };
    s.expansion
        .supports
        .push(timber(2, recipe.clone(), &closed.track)?);
    s.expansion.timber = recipe;
    s.expansion.structure = Some(2);
    s.expansion.member = None;
    let mut launch = TrackDevice::new(1, DeviceKind::Launch, 0., 35.);
    launch.acceleration_mps2 = 2.;
    let mut brake = TrackDevice::new(2, DeviceKind::Brake, 42., 75.);
    brake.acceleration_mps2 = 3.;
    s.expansion.operations.devices = vec![launch, brake];
    s.expansion.operations.physical.initial_speed_mps = 3.;
    if !s.apply_expansion() {
        return Err(format!("timber/devices apply: {:?}", s.expansion.message).into());
    }
    let final_project = s.editor.project().clone();
    let sc = scene(&final_project, &catalog, s.options)?;
    reports.push(capture(&mut s, &sc, &output, "05-procedural-timber.png")?);
    s.save_to(&output.join("integrated-development-fixture.qx.json"));
    if let Some(error) = &s.save_failure {
        return Err(error.clone().into());
    }
    if persistence::load(
        &output.join("integrated-development-fixture.qx.json"),
        &catalog,
    )? != final_project
    {
        return Err("save/reopen mismatch".into());
    }
    let mut sim = qx_physics::TrackSimulation::with_operations(
        train,
        Arc::new(sc.track.clone()),
        15.,
        final_project.operations.clone(),
    )?;
    let mut trace = String::from(
        "time_s,signed_speed_mps,travel_m,acceleration_x_mps2,acceleration_y_mps2,acceleration_z_mps2,device_force_n\n",
    );
    let mut measurements = vec![];
    s.workbench.active = Profile::Train;
    s.train_station_m = Some(15.);
    s.ride = Default::default();
    s.expansion.structure = None;
    for tick in 1..=1440 {
        let diagnostics = sim.step()?;
        let t = sim.telemetry();
        trace.push_str(&format!(
            "{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.6}\n",
            t.elapsed_s,
            t.speed_mps,
            t.distance_traveled_m,
            t.acceleration_mps2.x,
            t.acceleration_mps2.y,
            t.acceleration_mps2.z,
            t.applied_device_force_n
        ));
        if tick == 240 || tick == 1200 {
            let now = Instant::now();
            s.playback.snapshot = Some(Snapshot {
                velocity_initialization: None,
                poses: sim.snapshot()?,
                previous: sim.previous_snapshot().clone(),
                committed_at: now,
                ticks: sim.ticks(),
                running: false,
                diagnostics: Some(diagnostics),
                speed_mps: sim
                    .world()
                    .bodies
                    .iter()
                    .map(|b| b.linear_velocity_mps.length())
                    .collect(),
                lag_events: 0,
                error: None,
                telemetry: t.clone(),
                timing: Default::default(),
                timeline: vec![],
            });
            for (name, mode) in if tick == 240 {
                vec![
                    ("06-launch-chase.png", Mode::Chase),
                    ("07-launch-rider.png", Mode::Rider),
                ]
            } else {
                vec![("08-brake-chase.png", Mode::Chase)]
            } {
                s.ride.mode = mode;
                s.development = true;
                reports.push(capture(&mut s, &sc, &output, name)?);
                let ctx = egui::Context::default();
                let mut display = DisplayOptions::default();
                let mut rendered = ctx.run_pass(Default::default(), |ctx| {
                    s.ui(ctx, &sc, &mut display);
                });
                rendered.textures_delta.clear();
                if display.camera_view.is_none() || s.ride.mode != mode {
                    return Err("capture did not retain the requested ride camera".into());
                }
                if let Some(e) = &s.ride.error {
                    return Err(e.clone().into());
                }
                measurements.push(json!({"capture":name,"ticks":tick,"speed_mps":t.speed_mps,"elapsed_s":t.elapsed_s,"device_force_n":t.applied_device_force_n,"camera_and_pose":super::ride_scenario::record(&s,&display)}));
            }
        }
    }
    std::fs::write(output.join("physics.csv"), trace)?;
    let closure = qx_geometry::topology::diagnose(&sc.track)?;
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"fixture_only":true,"offscreen_actual_editor":true,"desktop_control":false,
        "fixed_step_s":qx_physics::FIXED_STEP_SECONDS,"project_roundtrip":true,"completion_undo_redo":true,
        "closure":{"position_m":closure.position_gap_m,"tangent_rad":closure.tangent_gap_rad,"frame_rad":closure.frame_gap_rad,"valid":closure.valid},
        "supports":final_project.supports.iter().map(|s|json!({"id":s.id,"nodes":s.nodes.len(),"members":s.members.len()})).collect::<Vec<_>>(),
        "physics_samples":1440,"measurements":measurements,"gpu_reports":reports}),
        )?,
    )?;
    println!("Integrated Editor evidence: {}", output.display());
    Ok(())
}
