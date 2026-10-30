//! Public welcome startup and preserved development diagnostics.
mod authoring;
mod benchmark;
mod export;
mod physics_diagnostic;
use qx_document::{
    command::{Command, Editor},
    persistence,
    sample::foundation_project,
};
use qx_geometry::solver::GenerationOptions;
use qx_style::Catalog;
use std::{error::Error, path::PathBuf};

const USAGE: &str = "QUANTUM Dynamics authoring diagnostics (development only)

  QUANTUM Dynamics.exe                       Welcome Screen (no document)
  quantum-x edit [project.qx.json]            Direct development editor
  quantum-x <project.qx.json>                 Direct project-file launch
  quantum-x demo <output-directory>
  quantum-x examples <output-directory>
  quantum-x generate <project.qx.json> <output-directory> [max-step-metres]
  quantum-x inspect <project.qx.json>
  quantum-x heartline-demo <output-directory>
  quantum-x render-editor <project.qx.json> <output.png> [track|train [width height [section-id channel]]]
  quantum-x render-welcome <output.png>
  quantum-x editor-workflow-scenario <output-directory>
  quantum-x ui-reference-scenario <project.qx.json> <output-directory>
  quantum-x --startup-check [edit [project.qx.json] | project.qx.json]
  quantum-x move-point <input.qx.json> <output.qx.json> <section-id> <point-id> <x> <y> <z>
  quantum-x authoring-scenario <output-directory>
  quantum-x transition-scenario <new-output-directory>
  quantum-x track-scenario <output-directory>
  quantum-x library [assets-root]
  quantum-x --version
  quantum-x benchmark
  quantum-x physics-diagnostic

Exports authored JSON, numerical CSV, diagnostic OBJ, SVG plots, and summary.
The design reference speed is not a simulated train velocity.";
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let launch = authoring::startup::launch(&std::env::current_exe()?, &args);
    if args.len() == 3 && args[0] == "ui-reference-scenario" {
        return authoring::ui_reference_scenario::run(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
        );
    }
    if args
        .first()
        .is_some_and(|a| a == "editor-workflow-scenario")
        && args.len() == 2
    {
        return authoring::workflow_scenario::run(PathBuf::from(&args[1]));
    }
    if launch != authoring::startup::Launch::Command {
        return authoring::startup::run(launch);
    }
    if args.first().is_some_and(|a| a == "--startup-check") {
        println!(
            "{}",
            serde_json::to_string_pretty(&authoring::startup::startup_check(&args[1..])?)?
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "render-welcome" {
        return authoring::startup::render_welcome(&PathBuf::from(&args[1]));
    }
    if args.len() == 1 && args[0] == "--version" {
        println!(
            "{} {} (repository {}; development CLI quantum-x)",
            qx_rendering::identity::product().name,
            env!("CARGO_PKG_VERSION"),
            qx_rendering::identity::product().repository
        );
        return Ok(());
    }
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        println!(
            "{USAGE}\nM8.1 acceptance: quantum-x train-asset-scenario <authored.qx.json> <output-directory>\nM8.2 acceptance: quantum-x train-playback-scenario <output-directory>"
        );
        return Ok(());
    }
    if (args[0] == "library" && args.len() <= 2) || (args[0] == "--self-check" && args.len() == 1) {
        let root = args
            .get(1)
            .map_or_else(authoring::library_ui::assets_root, PathBuf::from);
        let library = qx_style::library::Library::open(&root)?;
        for entry in library
            .entries
            .iter()
            .filter(|e| e.kind == qx_style::library::LibraryKind::Train && e.content_ready)
        {
            qx_document::library::load_train_definition(&library, &entry.style_id)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "product": qx_rendering::identity::product().name,
                "version": env!("CARGO_PKG_VERSION"),
                "repository": qx_rendering::identity::product().repository,
                "assets_root": root,
                "catalog_version": library.catalog.version,
                "track_families": library.catalog.families.len(),
                "train_families": library.catalog.train_families.len(),
                "entries": library.entries,
            }))?
        );
        return Ok(());
    }
    let (catalog, warning) = authoring::library_ui::load_catalog();
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    if args[0] == "train-playback-scenario" && args.len() == 2 {
        return authoring::train_playback_scenario(PathBuf::from(&args[1]));
    }
    if args[0] == "train-asset-scenario" && args.len() == 3 {
        return authoring::train_asset_scenario(PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    }
    if args[0] == "physics-diagnostic" && args.len() == 1 {
        return physics_diagnostic::run();
    }
    if args[0] == "track-scenario" && args.len() == 2 {
        return authoring::track_scenario(PathBuf::from(&args[1]));
    }
    if args[0] == "transition-scenario" && args.len() == 2 {
        return authoring::transition_scenario::run(PathBuf::from(&args[1]));
    }
    if args[0] == "authoring-scenario" && args.len() == 2 {
        return authoring::authoring_scenario(PathBuf::from(&args[1]));
    }
    if args[0] == "render-editor" && [3, 4, 6, 8].contains(&args.len()) {
        let profile = args
            .get(3)
            .map_or(Some("track"), |s| s.to_str())
            .ok_or("workspace must be UTF-8")?;
        let (width, height) = if args.len() >= 6 {
            (
                args[4].to_str().ok_or("width must be UTF-8")?.parse()?,
                args[5].to_str().ok_or("height must be UTF-8")?.parse()?,
            )
        } else {
            (1440, 960)
        };
        let selection = if args.len() == 8 {
            Some((
                args[6]
                    .to_str()
                    .ok_or("section ID must be UTF-8")?
                    .parse()?,
                args[7].to_str().ok_or("channel must be UTF-8")?,
            ))
        } else {
            None
        };
        return authoring::render(
            PathBuf::from(&args[1]),
            PathBuf::from(&args[2]),
            profile,
            width,
            height,
            selection,
        );
    }
    if args[0] == "heartline-demo" && args.len() == 2 {
        return export_project(
            qx_document::sample::heartline_reference_project(),
            PathBuf::from(&args[1]),
            0.1,
            &catalog,
        );
    }
    if args[0] == "inspect" && args.len() == 2 {
        let path = PathBuf::from(&args[1]);
        return qx_rendering::inspect(path, move |path| {
            let p = persistence::load(path, &catalog).map_err(|e| e.to_string())?;
            authoring::inspection_scene(&p, &catalog)
        })
        .map_err(Into::into);
    }
    if args[0] == "examples" && args.len() == 2 {
        let output = PathBuf::from(&args[1]);
        for (name, project) in [
            (
                "spline-comparison",
                qx_document::sample::spline_comparison_project(),
            ),
            (
                "mixed-authoring",
                qx_document::sample::mixed_authoring_project(),
            ),
            (
                "numerical-reference",
                qx_document::sample::numerical_reference_project(),
            ),
        ] {
            export_project(project, output.join(name), 0.1, &catalog)?;
        }
        return Ok(());
    }
    if args[0] == "benchmark" && args.len() == 1 {
        return benchmark::run(&catalog);
    }
    if args[0] == "move-point" && args.len() == 8 {
        let number = |i: usize| args[i].to_str().ok_or("numeric argument must be UTF-8");
        let p = persistence::load(&PathBuf::from(&args[1]), &catalog)?;
        let mut editor = Editor::new(p, catalog.clone(), 16)?;
        editor.execute([Command::EditSpline {
            section_id: qx_geometry::authoring::SectionId(number(3)?.parse()?),
            edit: qx_geometry::curve_edit::CurveEdit::MovePoint {
                id: number(4)?.parse()?,
                position_m: qx_geometry::math::Vec3::new(
                    number(5)?.parse()?,
                    number(6)?.parse()?,
                    number(7)?.parse()?,
                ),
            },
        }])?;
        editor
            .project()
            .generate(&catalog, GenerationOptions::default())?;
        persistence::save(&PathBuf::from(&args[2]), editor.project(), &catalog)?;
        println!("Control moved, validated, regenerated and saved. R reloads it in the inspector.");
        return Ok(());
    }
    let (project, output, step) = match args[0].to_str() {
        Some("demo") if args.len() == 2 => {
            // Exercise the same public commands that a future editor will use.
            let mut project = foundation_project();
            let tail = project
                .sections
                .pop()
                .expect("built-in fixture has sections");
            let after = project.sections.last().map(|s| s.id);
            let mut editor = Editor::new(project, catalog.clone(), 16)?;
            editor.execute([Command::InsertSection {
                after,
                section: tail,
            }])?;
            if !editor.undo() || !editor.redo() {
                return Err("diagnostic undo/redo failed".into());
            }
            (editor.project().clone(), PathBuf::from(&args[1]), 0.1)
        }
        Some("generate") if args.len() == 3 || args.len() == 4 => {
            let step = if args.len() == 4 {
                args[3]
                    .to_str()
                    .ok_or("max-step must be UTF-8 numeric text")?
                    .parse::<f64>()?
            } else {
                0.1
            };
            (
                persistence::load(&PathBuf::from(&args[1]), &catalog)?,
                PathBuf::from(&args[2]),
                step,
            )
        }
        _ => return Err(format!("invalid arguments\n\n{USAGE}").into()),
    };
    export_project(project, output, step, &catalog)
}
fn export_project(
    project: qx_document::Project,
    output: PathBuf,
    step: f64,
    catalog: &Catalog,
) -> Result<(), Box<dyn Error>> {
    let started = std::time::Instant::now();
    let options = GenerationOptions {
        max_step_m: step,
        ..Default::default()
    };
    let track = project.generate(catalog, options)?;
    println!(
        "Generation time: {:.3} ms (single run, includes validation/preparation).",
        started.elapsed().as_secs_f64() * 1000.
    );
    std::fs::create_dir_all(&output)?;
    let saved = output.join("project.qx.json");
    persistence::save(&saved, &project, catalog)?;
    let reopened = persistence::load(&saved, catalog)?;
    if project != reopened || track != reopened.generate(catalog, options)? {
        return Err("save/reopen regeneration was not identical".into());
    }
    let refinement = (|| -> Result<f64, Box<dyn Error>> {
        let fine = project.generate(
            catalog,
            GenerationOptions {
                max_step_m: step * 0.5,
                ..options
            },
        )?;
        let delta = track
            .samples
            .iter()
            .map(|s| {
                fine.sample_at_distance(s.distance_m.min(fine.length_m()))
                    .map(|p| (p.position_m - s.position_m).length())
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .fold(0.0, f64::max);
        Ok(delta)
    })()
    .map_or_else(
        |e| export::Refinement::Unavailable(e.to_string()),
        export::Refinement::Complete,
    );
    export::write_all(&output, &project, &track, step, &refinement)?;
    println!(
        "Generated {} sections, {} centerline samples, {:.6} m.\nSave/reopen and regeneration: identical.\n{}\n{} diagnostic warnings; see summary.txt.\nOutputs: {}",
        project.sections.len(),
        track.samples.len(),
        track.length_m(),
        refinement.description(),
        track.diagnostics.len(),
        output.canonicalize()?.display()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{}: {error}", qx_rendering::identity::product().name);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod identity_tests {
    #[test]
    fn public_double_click_opens_welcome_and_legacy_cli_keeps_help() {
        use crate::authoring::startup::{Launch, launch};
        use std::path::Path;
        assert_eq!(
            launch(Path::new("QUANTUM Dynamics.exe"), &[]),
            Launch::Welcome
        );
        assert_eq!(launch(Path::new("quantum-x.exe"), &[]), Launch::Command);
        assert!(super::USAGE.contains("QUANTUM Dynamics"));
        assert!(super::USAGE.contains("quantum-x generate"));
    }
}
