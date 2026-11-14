//! Bounded native application acceptance. Real worker, editor, cameras and wgpu;
//! no window input, alternate physics engine or standalone viewer.
use super::{
    ride_camera::Mode,
    session::{Session, blank_project, scene},
    train_playback::{Action, Snapshot},
    workbench::Profile,
};
use qx_document::{
    command::Command,
    persistence,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::authoring::{Section, SectionKind};
use serde_json::json;
use std::{
    error::Error,
    path::PathBuf,
    time::{Duration, Instant},
};

fn wait(s: &mut Session, predicate: impl Fn(&Snapshot) -> bool) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        s.playback.sync(s.editor.project(), s.options);
        if let Some(e) = &s.playback.error {
            return Err(e.clone().into());
        }
        if s.playback.snapshot.as_ref().is_some_and(&predicate) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("M4J playback worker timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// An explicit synthetic saved fixture is used when no project is supplied.
/// Both policies reopen exactly the same file through normal project persistence.
pub fn run(input: Option<PathBuf>, output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("choose a fresh M4J native evidence directory".into());
    }
    std::fs::create_dir_all(&output)?;
    let catalog = qx_style::Catalog::built_in()?;
    let synthetic = input.is_none();
    let input = if let Some(p) = input {
        p
    } else {
        let mut p = blank_project();
        p.metadata.name = "M4J synthetic R20 development fixture".into();
        p.sections[0] = Section::straight(1, "Synthetic horizontal R20", 120.);
        p.sections[0].kind = SectionKind::Arc {
            radius_m: 20.,
            plane_angle_rad: std::f64::consts::FRAC_PI_2,
        };
        p.trains.push(Train::development_consist(
            "m4j".into(),
            3,
            DevelopmentArrangement::Conventional,
        )?);
        p.heartline.train_configuration_id = Some("m4j".into());
        p.operations.physical.initial_speed_mps = 15.;
        let path = output.join("fixture.qx.json");
        persistence::save(&path, &p, &catalog)?;
        path
    };
    let bytes = std::fs::read(&input)?;
    let mut records = vec![];
    for experimental in [false, true] {
        let name = if experimental {
            "experimental"
        } else {
            "legacy"
        };
        let project = persistence::load(&input, &catalog)?;
        let mut s = Session::new(project, Some(&input), catalog.clone())?;
        s.assets.load_blocking(s.editor.project(), &s.path);
        // Keep an unsaved transaction while exercising initialization and playback.
        let mut metadata = s.editor.project().metadata.clone();
        metadata
            .description
            .push_str("\nM4J unsaved regression transaction");
        if !s.execute(vec![Command::SetMetadata(metadata)]) {
            return Err(s.status.into());
        }
        let project = s.editor.project().clone();
        let selected = s.selected;
        let dirty = s.dirty();
        let captured = scene(&project, &s.catalog, s.options)?;
        let track = captured.train_track()?;
        s.visible_revision = s.revision;
        s.workbench.active = Profile::Train;
        s.development = true;
        s.playback.experimental_velocity = experimental;
        s.playback.initial_speed_mps = project.operations.physical.initial_speed_mps;
        let station = if synthetic {
            40.
        } else {
            track.length_m() * 0.5
        };
        s.train_station_m = Some(station);
        let clock = Instant::now();
        s.playback.initialize(&project, s.options, track, station);
        wait(&mut s, |p| p.ticks == 0)?;
        let initialize_ms = clock.elapsed().as_secs_f64() * 1000.;
        let initial = s.playback.snapshot.as_ref().unwrap().poses.clone();
        let initialization = s
            .playback
            .snapshot
            .as_ref()
            .unwrap()
            .velocity_initialization
            .clone();
        // Settle workspace navigation before choosing its attached camera. The
        // existing per-workspace camera bank restores modes on profile changes.
        let ctx = egui::Context::default();
        let mut display = qx_rendering::DisplayOptions::default();
        for _ in 0..3 {
            let mut pass = ctx.run_pass(Default::default(), |ctx| {
                use qx_rendering::EditorSurface;
                s.ui(ctx, &captured, &mut display);
            });
            pass.textures_delta.clear();
        }
        if initialization.is_some() != experimental {
            return Err("M4J wrong worker initializer".into());
        }
        for tick in [0, 1, 120] {
            s.ride.mode = Mode::Chase;
            let current = s.playback.snapshot.as_ref().unwrap().ticks;
            for next in current + 1..=tick {
                s.playback.action(Action::Step);
                wait(&mut s, |p| p.ticks == next)?;
            }
            let file = format!("{name}-tick-{tick}.png");
            let render_report =
                qx_rendering::render_offscreen(&captured, &mut s, &output.join(&file), 1440, 960)?;
            let p = s.playback.snapshot.as_ref().unwrap();
            if s.ride.mode != Mode::Chase || s.ride.error.is_some() {
                return Err("M4J chase camera was not active".into());
            }
            if s.ride.presentation.as_ref() != Some(&p.poses) {
                return Err("M4J viewport does not match physics snapshot".into());
            }
            records.push(json!({"policy":name,"tick":tick,"capture":file,"render_report":render_report,
                "initialize_worker_roundtrip_ms":initialize_ms,
                "initializer":initialization.as_ref().map(|r|json!({"rows":r.rows,"sweeps":r.sweeps,"reference_guide":r.reference_guide,"speed_mps":r.achieved_speed_mps,"residual":r.max_residual})),
                "telemetry_speed_mps":p.telemetry.speed_mps,"poses":p.poses.components.iter().map(|c|json!({"id":c.id,"position_m":c.pose.position_m,"orientation":c.pose.orientation})).collect::<Vec<_>>(),
                "step_max_ms":p.timing.step_max_ms,"joint_reactions_n":p.diagnostics.as_ref().map(|d|&d.joint_force_on_b_n)}));
        }
        s.playback.action(Action::Play);
        wait(&mut s, |p| p.running && p.ticks >= 123)?;
        s.playback.action(Action::Pause);
        wait(&mut s, |p| !p.running)?;
        let paused = s.playback.snapshot.as_ref().unwrap().ticks;
        std::thread::sleep(Duration::from_millis(25));
        s.playback.sync(&project, s.options);
        if s.playback.snapshot.as_ref().unwrap().ticks != paused {
            return Err("pause advanced ticks".into());
        }
        s.playback.action(Action::Reset);
        wait(&mut s, |p| p.ticks == 0 && !p.running)?;
        if s.playback.snapshot.as_ref().unwrap().poses != initial {
            return Err("native reset mismatch".into());
        }
        // Attached cameras already follow the same physical poses. Capture rider
        // presentation after reset using the real native viewport.
        s.ride.mode = Mode::Rider;
        qx_rendering::render_offscreen(
            &captured,
            &mut s,
            &output.join(format!("{name}-rider-reset.png")),
            1440,
            960,
        )?;
        if s.ride.error.is_some() {
            return Err(format!("camera error: {:?}", s.ride.error).into());
        }
        s.playback.discard();
        if s.editor.project() != &project
            || s.selected != selected
            || s.dirty() != dirty
            || !s.editor.can_undo()
        {
            return Err("playback changed authored state/history/selection".into());
        }
        if !s.editor.undo() || !s.editor.redo() || s.editor.project() != &project {
            return Err("history lost".into());
        }
        let saved = output.join(format!("{name}-reopened.qx.json"));
        persistence::save(&saved, s.editor.project(), &s.catalog)?;
        if persistence::load(&saved, &s.catalog)? != project {
            return Err("M4J save/reopen mismatch".into());
        }
    }
    if std::fs::read(&input)? != bytes {
        return Err("input project modified".into());
    }
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(&json!({
        "offscreen_native_application":true,"desktop_manually_verified":false,
        "synthetic_development_fixture":synthetic,"input":input,"input_unchanged":true,
        "play_pause_reset":true,"history_dirty_selection_preserved":true,"save_reopen":true,
        "records":records}))?,
    )?;
    println!(
        "M4J native worker/wgpu/egui checks passed: {}",
        output.display()
    );
    Ok(())
}
