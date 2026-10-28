//! Development-only authoring diagnostics and native inspection; no final editor UX.
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

const USAGE: &str = "QUANTUM-X authoring diagnostics (development only)\n\n  quantum-x demo <output-directory>\n  quantum-x examples <output-directory>\n  quantum-x generate <project.qx.json> <output-directory> [max-step-metres]\n  quantum-x inspect <project.qx.json>\n  quantum-x edit [project.qx.json]\n  quantum-x heartline-demo <output-directory>\n  quantum-x render-editor <project.qx.json> <output.png> [track|train [width height [section-id channel]]]\n  quantum-x move-point <input.qx.json> <output.qx.json> <section-id> <point-id> <x> <y> <z>\n  quantum-x authoring-scenario <output-directory>\n  quantum-x track-scenario <output-directory>\n  quantum-x benchmark\n  quantum-x physics-diagnostic\n\nExports authored JSON, numerical CSV, diagnostic OBJ, SVG plots, and summary.\nThe design reference speed is not a simulated train velocity.";
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        println!(
            "{USAGE}\nM8.1 acceptance: quantum-x train-asset-scenario <authored.qx.json> <output-directory>"
        );
        return Ok(());
    }
    let catalog = Catalog::built_in()?;
    if args[0] == "train-asset-scenario" && args.len() == 3 {
        return authoring::train_asset_scenario(PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    }
    if args[0] == "physics-diagnostic" && args.len() == 1 {
        return physics_diagnostic::run();
    }
    if args[0] == "track-scenario" && args.len() == 2 {
        return authoring::track_scenario(PathBuf::from(&args[1]));
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
    if args[0] == "edit" && args.len() <= 2 {
        return authoring::run(args.get(1).map(PathBuf::from));
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
        eprintln!("QUANTUM-X: {error}");
        std::process::exit(1);
    }
}
