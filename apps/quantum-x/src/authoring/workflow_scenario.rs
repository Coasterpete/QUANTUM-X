//! Noninteractive lifecycle evidence and real GPU/UI captures, never desktop input.
use super::{
    coaster_properties,
    recent::Recent,
    startup::{App, Page},
};
use qx_document::persistence;
use qx_rendering::{EditorSurface, InspectionScene};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

pub(crate) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("choose a new scenario output directory".into());
    }
    std::fs::create_dir_all(&output)?;
    let mut app = App::offscreen()?;
    app.recent = Recent::default();
    if app.page != Page::Welcome || app.session.is_some() {
        return Err("Welcome opened a document".into());
    }
    capture(&mut app, &output.join("welcome.png"), 1280, 800)?;
    app.begin_new();
    app.setup.name = "Workflow verification fixture".into();
    capture(&mut app, &output.join("new-project.png"), 960, 640)?;
    app.cancel_setup();
    if app.session.is_some() {
        return Err("Cancel created a document".into());
    }
    app.begin_new();
    app.setup.name = "Workflow verification fixture".into();
    app.create();
    if app.page != Page::Editor {
        return Err(app.status.into());
    }
    let original = app.session.as_ref().unwrap().editor.project().clone();
    if !original.style.is_unassigned() {
        return Err("New Project assigned a style".into());
    }
    let unassigned_path = output.join("unassigned.qx.json");
    app.session.as_mut().unwrap().save_to(&unassigned_path);
    if persistence::load(&unassigned_path, &app.catalog)? != original {
        return Err("unassigned round trip changed the project".into());
    }
    coaster_properties::open(app.session.as_mut().unwrap());
    for (width, height) in [(1440, 960), (960, 640)] {
        capture(
            &mut app,
            &output.join(format!("properties-unassigned-{width}.png")),
            width,
            height,
        )?;
    }
    let s = app.session.as_mut().unwrap();
    s.metadata_draft.description = "Automated workflow fixture; no authored coaster layout.".into();
    s.style_draft = qx_style::StyleSelection {
        family_id: "monorail".into(),
        style_id: "monorail-200".into(),
    };
    if !coaster_properties::apply(s) {
        return Err(s.status.clone().into());
    }
    let assigned = s.editor.project().clone();
    if assigned.sections != original.sections
        || assigned.construction != original.construction
        || assigned.trains != original.trains
    {
        return Err("style assignment altered authoring data".into());
    }
    s.undo(false);
    if s.editor.project() != &original {
        return Err("properties undo failed".into());
    }
    s.undo(true);
    if s.editor.project() != &assigned {
        return Err("properties redo failed".into());
    }
    let assigned_path = output.join("properties.qx.json");
    s.save_to(&assigned_path);
    app.load_project(assigned_path, None);
    if app.session.as_ref().unwrap().editor.project() != &assigned {
        return Err("properties reopen failed".into());
    }
    coaster_properties::open(app.session.as_mut().unwrap());
    for (width, height) in [(1440, 960), (960, 640)] {
        capture(
            &mut app,
            &output.join(format!("properties-assigned-{width}.png")),
            width,
            height,
        )?;
    }
    for scale in [1.5, 2.] {
        app.session.as_mut().unwrap().workbench.window.ui_scale = scale;
        capture(
            &mut app,
            &output.join(format!("properties-scale-{scale}.png")),
            1440,
            960,
        )?;
    }
    let evidence = serde_json::json!({
        "automated_offscreen": true, "manual_desktop_tested": false,
        "welcome_without_document": true, "creation_requires_only_name": true,
        "cancel_creates_no_document": true, "new_style_unassigned": true,
        "properties_undo_redo_roundtrip": true, "geometry_construction_trains_preserved": true,
        "schema_version": assigned.schema_version, "single_coaster": true,
        "title": app.window_title(), "style": assigned.style
    });
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_string_pretty(&evidence)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn capture(app: &mut App, path: &Path, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
    let scene = if app.page == Page::Editor {
        let s = app.session.as_ref().unwrap();
        super::session::scene(s.editor.project(), &app.catalog, s.options)?
    } else {
        InspectionScene::empty()
    };
    // Consume the lifecycle scene before rendering so offscreen capture uses the
    // exact supplied geometry, with no native window or preference/history write.
    app.poll_scene();
    println!(
        "{}",
        qx_rendering::render_offscreen(&scene, app, path, width, height)?
    );
    Ok(())
}
