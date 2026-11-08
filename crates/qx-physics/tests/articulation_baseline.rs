#[path = "support/articulation.rs"]
mod fixtures;

use fixtures::*;
use qx_physics::{
    assembly_diagnostics::AssemblyDiagnostics,
    consist::{ConsistInspection, inspect, inspect_with_diagnostics},
};
use std::{hint::black_box, time::Instant};

// Fingerprint exact f64 pose/station/anchor bits, independently of diagnostics.
fn fingerprint(p: &ConsistInspection) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut add = |v: f64| {
        hash = (hash ^ v.to_bits()).wrapping_mul(0x100000001b3);
    };
    for c in &p.components {
        let v = c.pose.position_m;
        let q = c.pose.orientation;
        for x in [c.station_m, v.x, v.y, v.z, q.w, q.x, q.y, q.z] {
            add(x);
        }
    }
    for j in &p.joints {
        for v in [j.anchor_a_m, j.anchor_b_m] {
            for x in [v.x, v.y, v.z] {
                add(x);
            }
        }
        add(j.error_m);
    }
    hash
}

#[test]
#[ignore = "manual Release baseline; timings are observations, never CI thresholds"]
fn release_assembly_baseline() {
    let straight = straight();
    let curve = arc(45., 0.25, 500., false);
    let twisting = arc(45., 0.25, 500., true);
    for (name, track, count) in [
        ("straight-2", &straight, 2),
        ("straight-8", &straight, 8),
        ("straight-16", &straight, 16),
        ("straight-32", &straight, 32),
        ("curve-2", &curve, 2),
        ("curve-8", &curve, 8),
        ("twisting-8", &twisting, 8),
    ] {
        let train = conventional(count);
        let warm = inspect(&train, track, 200.);
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..5 {
                let _ = black_box(inspect(black_box(&train), black_box(track), 200.));
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 5.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{name}: median_us={:.3} min_us={:.3} max_us={:.3} fingerprint={:?} error={:?}",
            samples[3],
            samples[0],
            samples[6],
            warm.as_ref().ok().map(fingerprint),
            warm.err()
        );
    }
}

#[test]
#[ignore = "manual Release observations; kept separate from ordinary baseline timing"]
fn release_instrumented_assembly_baseline() {
    let straight = straight();
    let curve = arc(45., 0.25, 500., false);
    let twisting = arc(45., 0.25, 500., true);
    for (name, track, count) in [
        ("straight-2", &straight, 2),
        ("straight-8", &straight, 8),
        ("straight-16", &straight, 16),
        ("straight-32", &straight, 32),
        ("curve-2", &curve, 2),
        ("curve-8", &curve, 8),
        ("twisting-8", &twisting, 8),
    ] {
        let train = conventional(count);
        inspect_with_diagnostics(&train, track, 200., &mut AssemblyDiagnostics::default()).unwrap();
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..5 {
                let mut d = AssemblyDiagnostics::default();
                let _ = black_box(inspect_with_diagnostics(
                    black_box(&train),
                    black_box(track),
                    200.,
                    &mut d,
                ));
                black_box(d);
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 5.);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{name} instrumented: median_us={:.3} min_us={:.3} max_us={:.3}",
            samples[3], samples[0], samples[6]
        );
    }
}
