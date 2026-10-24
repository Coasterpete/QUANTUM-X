//! Development-only headless diagnostic entry point; no editor UX is defined here.
mod export;
use qx_document::{
    command::{Command, Editor},
    persistence,
    sample::foundation_project,
};
use qx_geometry::solver::GenerationOptions;
use qx_style::Catalog;
use std::{error::Error, path::PathBuf};

const USAGE: &str = "QUANTUM-X core foundation diagnostics (development only)\n\n  quantum-x demo <output-directory>\n  quantum-x generate <project.qx.json> <output-directory> [max-step-metres]\n\nExports authored JSON, centerline/measurement/transition CSV, diagnostic OBJ, SVG plots, and summary.\nThe design reference speed is not a simulated train velocity.";
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        println!("{USAGE}");
        return Ok(());
    }
    let catalog = Catalog::built_in()?;
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
    let options = GenerationOptions {
        max_step_m: step,
        ..Default::default()
    };
    let track = project.generate(&catalog, options)?;
    std::fs::create_dir_all(&output)?;
    let saved = output.join("project.qx.json");
    persistence::save(&saved, &project, &catalog)?;
    let reopened = persistence::load(&saved, &catalog)?;
    if project != reopened || track != reopened.generate(&catalog, options)? {
        return Err("save/reopen regeneration was not identical".into());
    }
    let refinement = (|| -> Result<f64, Box<dyn Error>> {
        let fine = project.generate(
            &catalog,
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
