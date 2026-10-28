//! Reproducible actual-app import/assignment/save/reopen/offscreen acceptance.
use super::{
    session::{Session, scene},
    train_assets, train_preview,
    workbench::Profile,
};
use qx_document::{
    command::Command,
    persistence,
    train::{ComponentKind, Train},
};
use qx_geometry::math::{Quaternion, Vec3};
use qx_rendering::{DisplayOptions, EditorSurface};
use serde_json::json;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
};

pub(super) fn run(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&output)?;
    let catalog = qx_style::Catalog::built_in()?;
    let p = persistence::load(&input, &catalog)?;
    let mut s = Session::new(p, Some(&input), catalog.clone())?;
    let train = Train::development_fixture("m81-development".into());
    let mechanical = train.clone();
    let id = train.id.clone();
    let mut trains = s.editor.project().trains.clone();
    trains.push(train);
    s.execute(vec![Command::SetTrains(trains)]);
    let mut reference = s.editor.project().heartline.clone();
    reference.train_configuration_id = Some(id.clone());
    if !s.execute(vec![Command::SetHeartline(reference)]) {
        return Err(s.status.into());
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/development/train");
    let start = Instant::now();
    let body = s
        .assets
        .import_now(&fixtures.join("development-blender-body.glb"), &s.path)?;
    let bogie = s
        .assets
        .import_now(&fixtures.join("development-bogie.glb"), &s.path)?;
    let import_ms = start.elapsed().as_secs_f64() * 1000.;
    for c in &mechanical.components {
        let mut asset = train_assets::identity(if c.kind == ComponentKind::CarBody {
            body.clone()
        } else {
            bogie.clone()
        });
        // Nontrivial correction verifies pivot and TRS composition independently.
        if c.kind == ComponentKind::CarBody {
            asset.pivot_m = Vec3::new(0.05, 0., 0.);
            asset.translation_m = Vec3::new(0.05, 0.08, 0.);
            asset.scale = Vec3::new(1., 1.02, 1.);
            asset.rotation = Quaternion::axis_angle(Vec3::X, 0.01)?;
        }
        if !s.assign_train_asset(&id, &c.id, Some(asset)) {
            return Err(s.status.into());
        }
    }
    let assigned = s.editor.project().clone();
    s.undo(false);
    s.undo(true);
    if s.editor.project() != &assigned {
        return Err("Assignment undo/redo did not round trip".into());
    }
    if !s.assign_train_asset(&id, "car-1", None) {
        return Err(s.status.into());
    }
    s.undo(false);
    if s.editor.project() != &assigned {
        return Err("Remove/undo changed the document".into());
    }
    let path = output.join("train-inspection.qx.json");
    s.save_to(&path);
    if s.save_failure.is_some() {
        return Err(s.status.into());
    }
    let reopened = persistence::load(&path, &catalog)?;
    if reopened != assigned {
        return Err("Save/reopen changed assignments".into());
    }
    let mut s = Session::new(reopened, Some(&path), catalog.clone())?;
    s.assets.load_blocking(s.editor.project(), &s.path);
    if !s.assets.errors.is_empty() {
        return Err(format!("Reopen asset failures: {:?}", s.assets.errors).into());
    }
    let relocated_dir = output.join("save-as");
    std::fs::create_dir_all(&relocated_dir)?;
    let relocated = relocated_dir.join("train-inspection.qx.json");
    s.save_to(&relocated);
    if s.save_failure.is_some() {
        return Err(s.status.into());
    }
    let saved = persistence::load(&relocated, &catalog)?;
    if saved != assigned {
        return Err("Save As changed assignments".into());
    }
    s.open(Some(relocated.clone()), true);
    s.assets.load_blocking(s.editor.project(), &s.path);
    if !s.assets.errors.is_empty() {
        return Err("Save As did not package assets".into());
    }
    let captured = scene(s.editor.project(), &catalog, Default::default())?;
    s.visible_revision = s.revision;
    s.workbench.active = Profile::Train;
    s.frame_requested = false;
    let mut records = vec![];
    for (case, fraction) in [("station-a", 0.4), ("station-b", 0.6)] {
        let station = captured.track.length_m() * fraction;
        s.train_station_m = Some(station);
        let train = s
            .editor
            .project()
            .trains
            .iter()
            .find(|t| t.id == id)
            .unwrap();
        let poses = qx_physics::consist::inspect(train, &captured.track, station)?;
        let mut display = DisplayOptions::default();
        train_preview::populate(train, &captured.track, &poses, &mut s.assets, &mut display);
        if display.asset_instances.len() != 6 {
            return Err("Expected two body and four bogie instances".into());
        }
        let components:Vec<_>=poses.components.iter().map(|p| {
            let sample=captured.track.sample_at_distance(p.station_m).unwrap();
            json!({"id":p.id,"station":p.station_m,"position":p.pose.position_m,"orientation":p.pose.orientation,"track_position":sample.position_m,"track_orientation":sample.frame.orientation()})
        }).collect();
        let instances: Vec<_> = display
            .asset_instances
            .iter()
            .map(|i| json!({"component":i.component_id,"matrix":i.transform,"bounds":i.bounds()}))
            .collect();
        let joints: Vec<_> = poses
            .joints
            .iter()
            .map(|j| json!({"id":j.id,"a":j.anchor_a_m,"b":j.anchor_b_m,"error":j.error_m}))
            .collect();
        let mut captures = vec![];
        for (width, height) in [(1440, 960), (960, 640)] {
            let file = format!("{case}-{width}x{height}.png");
            let report = qx_rendering::render_offscreen(
                &captured,
                &mut s,
                &output.join(&file),
                width,
                height,
            )?;
            println!("{file}: {report}");
            captures.push(json!({"file":file,"report":report}));
        }
        records.push(json!({"case":case,"station":station,"components":components,"instances":instances,"joints":joints,"captures":captures}));
    }
    s.train_station_m = Some(captured.track.length_m() * 0.4);
    s.assets.wireframe = false;
    s.assets.joints = false;
    s.assets.couplers = false;
    s.assets.wheels = false;
    let clean_report = qx_rendering::render_offscreen(
        &captured,
        &mut s,
        &output.join("assets-without-overlays.png"),
        1440,
        960,
    )?;
    let models:Vec<_>=s.assets.library.iter().map(|(path,e)|json!({"path":path,"import_ms":e.model.import_ms,"nodes":e.model.nodes.iter().map(|n|json!({"primitive":n.primitive,"matrix":n.transform})).collect::<Vec<_>>(),"primitives":e.model.primitives.iter().map(|p|json!({"positions":p.vertices.iter().map(|v|v.position).collect::<Vec<_>>(),"normals":p.vertices.iter().map(|v|v.normal).collect::<Vec<_>>(),"uv":p.vertices.iter().map(|v|v.uv).collect::<Vec<_>>(),"indices":p.indices,"color":p.material.color})).collect::<Vec<_>>()})).collect();
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    let _ = ctx.run_pass(Default::default(), |ctx| {
        s.ui(ctx, &captured, &mut display);
    });
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"automated_offscreen":true,"manual_desktop_tested":false,"source_project":input.file_name(),"project":"train-inspection.qx.json","train_id":id,"import_and_file_read_ms":import_ms,"models":models,"records":records,"clean_report":clean_report,"mechanical_definition":mechanical,"history_save_reopen_save_as":true}),
        )?,
    )?;
    println!(
        "M8.1 scenario completed; import + file read {import_ms:.3} ms. No desktop interaction."
    );
    Ok(())
}
