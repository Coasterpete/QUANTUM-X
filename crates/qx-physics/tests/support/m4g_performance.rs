//! Serial, test-only complete-step observations, never a deadline guarantee.
use super::*;
use std::time::{Duration, Instant};

#[test]
#[ignore = "serial native Release repeated complete-step profiling on unchanged 72m source"]
fn repeated_complete_step_costs() {
    let (initial, sections, lead) = extended::fixture("foundation");
    let r = rail(&initial, &sections);
    let gravity = Vec3::new(0., -9.81, 0.);
    // One entire warm-up run, then five complete runs. Pair every timed step
    // with the inherited engine outside its timer; verify exact numerical output.
    for run in 0..6 {
        let mut measured = profiled_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
        let mut control = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
        for tick in 1..=480 {
            r.timing.set(true);
            r.rail_time.set(Duration::ZERO);
            let clock = Instant::now();
            let report = measured.step(&r, gravity).unwrap();
            let complete = clock.elapsed();
            let physical = r.rail_time.get();
            r.timing.set(false);
            let p = &measured.profile;
            let mechanical = p.evaluate + p.curvature - p.evaluate_rail - p.curvature_rail;
            let post = p.advance_and_validate + measured.tracking_time
                - p.validation_rail
                - (physical - p.rail);
            let other = complete.saturating_sub(physical + mechanical + p.impulses + post);
            assert!(report.max_position < 2e-5 && report.max_velocity < 0.003);
            let control_clock = Instant::now();
            let control_report = control.step(&r, gravity).unwrap();
            let control_ms = control_clock.elapsed().as_secs_f64() * 1000.;
            assert_eq!(report, control_report);
            assert_eq!(measured.state, control.state);
            assert_eq!(measured.hints(), control.hints());
            assert_eq!(
                measured
                    .envelopes
                    .iter()
                    .map(|e| [e.rate, e.innovation, e.half])
                    .collect::<Vec<_>>(),
                control
                    .envelopes
                    .iter()
                    .map(|e| [e.rate, e.innovation, e.half])
                    .collect::<Vec<_>>()
            );
            if run > 0 {
                println!(
                    "M4G_STEP_COST {{\"run\":{run},\"tick\":{tick},\"control_ms\":{control_ms},\"complete_ms\":{},\"rail_ms\":{},\"constraints_ms\":{},\"impulses_ms\":{},\"validation_tracking_ms\":{},\"other_ms\":{}}}",
                    complete.as_secs_f64() * 1000.,
                    physical.as_secs_f64() * 1000.,
                    mechanical.as_secs_f64() * 1000.,
                    p.impulses.as_secs_f64() * 1000.,
                    post.as_secs_f64() * 1000.,
                    other.as_secs_f64() * 1000.
                );
            }
        }
    }
}
