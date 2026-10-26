//! Opt-in reproducible validation fixture. Never injected into a new document.
use super::{
    graph_canvas, panels,
    session::{Session, blank_project},
};
use qx_geometry::{authoring::ChannelKey, transition::Function};
use std::{error::Error, path::PathBuf};

pub(super) fn build() -> Result<Session, Box<dyn Error>> {
    let mut session = Session::new(blank_project(), None, qx_style::Catalog::built_in()?)?;
    session.metadata_draft.name = "M6 authoring validation fixture".into();
    session.metadata_draft.description = "Opt-in synthetic straight/hill workflow validation; design-speed geometry, not a production ride or train simulation.".into();
    session.execute(vec![qx_document::command::Command::SetMetadata(
        session.metadata_draft.clone(),
    )]);
    session.draft.name = "10 m straight".into();
    if !session.apply_draft() {
        return Err(session.status.into());
    }
    panels::insert_section(&mut session, 0, false);
    if session.selected.0 != 2 {
        return Err(session.status.into());
    }
    session.draft = qx_document::editing::resize(&session.draft, 80.)?;
    session.draft.name = "Force-vector hill (validation fixture)".into();
    // These are the same explicit Split, function and numeric-value operations
    // available in the Transition Editor; no alternative generation path.
    session.draft =
        qx_document::editing::split_hermite(&session.draft, ChannelKey::Normal, 1, 0.5)?;
    session.draft =
        qx_document::editing::split_hermite(&session.draft, ChannelKey::Normal, 1, 0.5)?;
    session.draft =
        qx_document::editing::split_hermite(&session.draft, ChannelKey::Normal, 2, 0.5)?;
    for (transition, (start, end)) in session
        .draft
        .channel_mut(ChannelKey::Normal)?
        .transitions
        .iter_mut()
        .zip([(1., 2.), (2., 0.), (0., 0.), (0., 1.)])
    {
        transition.function = Function::QuinticSmoothstep {};
        transition.start = start;
        transition.end = end;
    }
    for (key, peak) in [(ChannelKey::Lateral, 0.1), (ChannelKey::Roll, 0.006)] {
        session.draft = qx_document::editing::split_hermite(&session.draft, key, 1, 0.5)?;
        for (transition, (start, end)) in session
            .draft
            .channel_mut(key)?
            .transitions
            .iter_mut()
            .zip([(0., peak), (peak, 0.)])
        {
            transition.function = Function::QuinticSmoothstep {};
            transition.start = start;
            transition.end = end;
        }
    }
    if !session.apply_draft() {
        return Err(session.status.into());
    }
    let before = session.editor.project().clone();
    session.draft = graph_canvas::boundary(&session.draft, ChannelKey::Normal, 0, 22.)?;
    if !session.apply_draft() {
        return Err(session.status.into());
    }
    let after = session.editor.project().clone();
    session.undo(false);
    if session.editor.project() != &before {
        return Err("scenario undo did not restore the authored profiles".into());
    }
    session.undo(true);
    if session.editor.project() != &after {
        return Err("scenario redo did not restore the edited boundary".into());
    }
    Ok(session)
}
pub(super) fn run(output: PathBuf) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&output)?;
    let catalog = qx_style::Catalog::built_in()?;
    let new_path = output.join("new-project.qx.json");
    qx_document::persistence::save(&new_path, &blank_project(), &catalog)?;
    for (width, height) in [(1440, 960), (960, 640)] {
        super::render(
            new_path.clone(),
            output.join(format!("new-project-{width}.png")),
            "track",
            width,
            height,
            Some((1, "roll")),
        )?;
    }
    let mut session = build()?;
    let project = session.editor.project().clone();
    let generated = project.generate(&session.catalog, session.options)?;
    let path = output.join("project.qx.json");
    session.save_to(&path);
    if session.dirty() {
        return Err(session.status.into());
    }
    session.open(Some(path.clone()), true);
    if session.editor.project() != &project
        || session
            .editor
            .project()
            .generate(&session.catalog, session.options)?
            != generated
    {
        return Err("scenario save/reopen changed authored state or generated geometry".into());
    }
    crate::export_project(project, output.clone(), 0.1, &session.catalog)?;
    for (width, height) in [(1440, 960), (960, 640)] {
        for channel in ["normal", "lateral", "roll"] {
            super::render(
                path.clone(),
                output.join(format!("hill-{channel}-{width}.png")),
                "track",
                width,
                height,
                Some((2, channel)),
            )?;
        }
    }
    let peak = generated
        .samples
        .iter()
        .map(|s| s.position_m.y)
        .fold(f64::NEG_INFINITY, f64::max);
    println!(
        "M6 scenario: {} m, {} samples, peak elevation {:.9} m; boundary 22/18/20/20 m; Apply/undo/redo/save/reopen identical.",
        generated.length_m(),
        generated.samples.len(),
        peak
    );
    Ok(())
}
