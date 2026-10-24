use qx_geometry::{
    authoring::Section,
    curve::{ControlPoint, Curve, clamped_knots},
    math::Vec3,
    solver::{CenterlineSample, GenerationOptions, GraphSample},
};
use std::{error::Error, time::Instant};
pub fn run(catalog: &qx_style::Catalog) -> Result<(), Box<dyn Error>> {
    for (count, length) in [(64, 1000.), (1024, 10000.)] {
        let points = (0..count)
            .map(|i| {
                let x = length * i as f64 / (count - 1) as f64;
                ControlPoint {
                    id: i as u64 + 1,
                    position_m: Vec3::new(
                        x,
                        if i < 3 { 0. } else { (x * 0.002).sin() * 10. },
                        if i < 3 { 0. } else { (x * 0.001).sin() * 5. },
                    ),
                }
            })
            .collect();
        let curve = Curve::BSpline {
            degree: 3,
            points,
            knots: clamped_knots(count, 3)?,
        };
        let mut project = qx_document::sample::foundation_project();
        project.sections = vec![Section::spline(1, "Long-track sampling benchmark", curve)];
        let start = Instant::now();
        let track = project.generate(
            catalog,
            GenerationOptions {
                max_step_m: 0.25,
                ..Default::default()
            },
        )?;
        let elapsed = start.elapsed();
        let bytes = track.samples.len() * std::mem::size_of::<CenterlineSample>()
            + track.graphs.len() * std::mem::size_of::<GraphSample>();
        println!(
            "controls={count}, length={:.6}m, samples={}, generation={:.3}ms, sample+graph payload={}bytes (excludes capacity/strings/preparation/GPU/history), quadrature={:?}",
            track.length_m(),
            track.samples.len(),
            elapsed.as_secs_f64() * 1000.,
            bytes,
            track.numerical
        );
    }
    Ok(())
}
