//! Actual application GPU captures at repeatable sizes, with no native desktop
//! input or preference/history writes. The supplied project is never modified.
use super::{
    startup::{App, Page},
    workbench::{Pane, Profile},
};
use qx_geometry::authoring::{ChannelKey, SectionKind};
use qx_rendering::{EditorSurface, InspectionScene};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

pub(crate) fn run(input: PathBuf, output: PathBuf) -> Result<(), Box<dyn Error>> {
    if output.exists() {
        return Err("choose a new UI-reference output directory".into());
    }
    std::fs::create_dir_all(&output)?;
    let original_bytes = std::fs::read(&input)?;
    let mut app = App::offscreen()?;
    let mut captures = vec![];
    let sizes = [
        (1440, 960, 1.),
        (960, 640, 1.),
        (1440, 960, 1.5),
        (1440, 960, 2.),
        (960, 640, 1.5),
        (960, 640, 2.),
    ];
    for (width, height, scale) in sizes {
        scale_to(&mut app, scale);
        app.page = Page::Welcome;
        captures.push(capture(&mut app, &output, "welcome", width, height, scale)?);
    }
    scale_to(&mut app, 1.);
    for (page, name) in [
        (Page::Library, "library"),
        (Page::Projects, "editor-choice"),
        (Page::Info, "info"),
        (Page::News, "news"),
    ] {
        app.page = page;
        captures.push(capture(&mut app, &output, name, 960, 640, 1.)?);
    }
    app.show_configuration();
    captures.push(capture(&mut app, &output, "setup", 960, 640, 1.)?);
    app.begin_new();
    app.setup.name = "Owner-named project".into();
    captures.push(capture(&mut app, &output, "new-project", 960, 640, 1.)?);
    app.cancel_setup();
    if app.session.is_some() {
        return Err("Welcome destinations created a document".into());
    }
    app.load_project(input.clone(), None);
    if app.page != Page::Editor {
        return Err(app.status.into());
    }
    let original = app.session.as_ref().unwrap().editor.project().clone();
    {
        let s = app.session.as_mut().unwrap();
        s.assets.load_blocking(s.editor.project(), &s.path);
        if !s.assets.errors.is_empty() {
            return Err(format!("asset errors: {:?}", s.assets.errors).into());
        }
        if let Some(section) = original
            .sections
            .iter()
            .find(|s| matches!(s.kind, SectionKind::Force { .. }))
        {
            s.select(section.id);
            s.select_channel(ChannelKey::Normal);
        }
    }
    for (profile, name) in [(Profile::Track, "editor"), (Profile::Train, "train")] {
        for (width, height, scale) in sizes {
            let s = app.session.as_mut().unwrap();
            s.workbench = Default::default();
            s.workbench.active = profile;
            s.workbench.window.ui_scale = scale;
            s.development = false;
            captures.push(capture(&mut app, &output, name, width, height, scale)?);
        }
    }
    let s = app.session.as_mut().unwrap();
    s.workbench = Default::default();
    s.workbench.active = Profile::Train;
    s.development = true;
    captures.push(capture(
        &mut app,
        &output,
        "train-development",
        1440,
        960,
        1.,
    )?);
    let s = app.session.as_mut().unwrap();
    s.workbench.active = Profile::Track;
    s.workbench.layouts[0].dock(Pane::Diagnostics);
    captures.push(capture(&mut app, &output, "diagnostics", 1440, 960, 1.)?);
    if app.session.as_ref().unwrap().editor.project() != &original
        || std::fs::read(&input)? != original_bytes
    {
        return Err("presentation changed the project".into());
    }
    let report = serde_json::json!({"automated_offscreen":true,"manual_desktop_tested":false,
        "project_unchanged":true,"no_implicit_document":true,"schema_version":original.schema_version,
        "captures":captures,"input":input});
    std::fs::write(
        output.join("evidence.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn scale_to(app: &mut App, scale: f32) {
    let mut preferences = app.window_preferences();
    preferences.ui_scale = scale;
    app.window_changed(preferences);
}
fn capture(
    app: &mut App,
    root: &Path,
    name: &str,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let file = format!("{name}-{width}-scale-{scale}.png");
    let scene = if app.page == Page::Editor {
        let s = app.session.as_ref().unwrap();
        super::session::scene(s.editor.project(), &s.catalog, s.options)?
    } else {
        InspectionScene::empty()
    };
    app.poll_scene();
    println!(
        "{}",
        qx_rendering::render_offscreen(&scene, app, &root.join(&file), width, height)?
    );
    Ok(serde_json::json!({"file":file,"size":[width,height],"scale":scale,"page":name}))
}
