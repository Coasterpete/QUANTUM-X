//! Bounded duration probe; rejection preserves the complete accepted transaction.
use super::*;
use matrix::{finite_jacobian, max_abs};
use original_dynamics::Step;

fn gate(report: Step) -> surface::Result<Step> {
    if report.max_position >= 2e-5 || report.max_velocity >= 0.003 {
        println!(
            "M4H_LONG_REJECT {{\"p\":{},\"v\":{}}}",
            report.max_position, report.max_velocity
        );
        Err(surface::Failure::Residual)
    } else {
        Ok(report)
    }
}

fn candidate(t: &mut measured_tracking::Tracked, r: &Rail) -> surface::Result<Step> {
    let state = t.state.clone();
    let supports = t.model.supports.clone();
    let envelopes = t.envelopes.clone();
    let ticks = t.ticks;
    let samples = t.samples.clone();
    let result = t.step(r, energy::G).and_then(gate);
    if result.is_err() {
        t.state = state;
        t.model.supports = supports;
        t.envelopes = envelopes;
        t.ticks = ticks;
        t.samples = samples;
    }
    result
}

fn control_candidate(t: &mut tracking::Tracked, r: &Rail) -> surface::Result<Step> {
    let state = t.state.clone();
    let supports = t.model.supports.clone();
    let envelopes = t.envelopes.clone();
    let ticks = t.ticks;
    let result = t.step(r, energy::G).and_then(gate);
    if result.is_err() {
        t.state = state;
        t.model.supports = supports;
        t.envelopes = envelopes;
        t.ticks = ticks;
    }
    result
}

#[test]
fn populated_cache_rejections_preserve_state_and_recorded_evidence() {
    let (i, s, lead) = energy::fixture("helix");
    let r = rail(&i, &s);
    r.reuse.set(true);
    let mut a = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    a.capture = true;
    candidate(&mut a, &r).unwrap();
    a.substeps = 3;
    let check = |a: &mut measured_tracking::Tracked, reason| {
        let before = reuse::state_bits(&a.state);
        let hints = a.hints();
        let envelopes = a.envelopes.clone();
        let samples = format!("{:?}", a.samples);
        let ticks = a.ticks;
        let windows: Vec<_> = a.model.supports.iter().map(|s| s.half_window).collect();
        assert_eq!(candidate(a, &r).unwrap_err(), reason);
        assert_eq!(reuse::state_bits(&a.state), before);
        assert_eq!(a.hints(), hints);
        assert_eq!(a.envelopes, envelopes);
        assert_eq!(format!("{:?}", a.samples), samples);
        assert_eq!(a.ticks, ticks);
        assert_eq!(
            a.model
                .supports
                .iter()
                .map(|s| s.half_window)
                .collect::<Vec<_>>(),
            windows
        );
    };
    check(&mut a, surface::Failure::Invalid);
    check(&mut a, surface::Failure::Invalid);
    a.substeps = 4;
    let mut fast = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    for v in &mut fast.state.velocity {
        *v *= 2.;
    }
    check(&mut fast, surface::Failure::Residual);
    // Identical failed queries must not poison the next accepted transaction.
    r.begin();
    candidate(&mut a, &r).unwrap();
    assert_eq!(a.ticks, 2);
}

#[test]
#[ignore = "Release bounded continuation through at most 720 ticks; original domains/gates unchanged"]
fn bounded_duration_and_independent_checkpoints() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = energy::fixture(name);
        let r = rail(&i, &s);
        let original = rail(&i, &s);
        r.reuse.set(true);
        let mut a = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
        a.capture = true;
        let mut b = tracking::Tracked::new(model(&original, lead, true), &original, 6.).unwrap();
        let start = energy::energies(&a.state).iter().sum::<f64>();
        let mut sums = [0.; 7];
        let mut last = 0;
        let locals: Vec<_> = a.model.supports.iter().map(|s| array(s.local)).collect();
        let edges: Vec<_> = a.model.bearings.iter().map(|b| [b.a, b.b]).collect();
        let bearing_locals: Vec<_> = a
            .model
            .bearings
            .iter()
            .map(|b| [array(b.local_a), array(b.local_b)])
            .collect();
        let roll_indices: Vec<_> = a
            .model
            .bearings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.roll)
            .map(|(k, _)| k)
            .collect();
        let charts: Vec<_> = r
            .source
            .segments()
            .iter()
            .map(|c| [c.start_station_m, c.end_station_m, c.chart as f64])
            .collect();
        println!(
            "M4H_LONG_SCENE {{\"fixture\":\"{name}\",\"length\":{},\"locals\":{locals:?},\"edges\":{edges:?},\"bearing_locals\":{bearing_locals:?},\"roll_indices\":{roll_indices:?},\"charts\":{charts:?},\"initial_energy\":{start}}}",
            r.length()
        );
        for tick in 1..=720 {
            r.begin();
            let before = reuse::state_bits(&a.state);
            let hints = a.hints();
            let env = a.envelopes.clone();
            let windows: Vec<_> = a.model.supports.iter().map(|s| s.half_window).collect();
            let samples = format!("{:?}", a.samples);
            let x = candidate(&mut a, &r);
            let y = control_candidate(&mut b, &original);
            assert_eq!(x, y);
            match x {
                Err(reason) => {
                    assert!(tick > 480, "required M4G duration must finish");
                    assert_eq!(reuse::state_bits(&a.state), before);
                    assert_eq!(a.hints(), hints);
                    assert_eq!(a.envelopes, env);
                    assert_eq!(a.ticks, tick - 1);
                    assert_eq!(
                        a.model
                            .supports
                            .iter()
                            .map(|s| s.half_window)
                            .collect::<Vec<_>>(),
                        windows
                    );
                    assert_eq!(format!("{:?}", a.samples), samples);
                    assert_eq!(reuse::state_bits(&a.state), reuse::state_bits(&b.state));
                    println!(
                        "M4H_LONG_STOP {{\"fixture\":\"{name}\",\"candidate_tick\":{tick},\"accepted\":{},\"reason\":\"{reason:?}\"}}",
                        a.ticks
                    );
                    break;
                }
                Ok(report) => {
                    reuse::pair(&a, &b, &report, y.as_ref().unwrap());
                    for sample in &a.samples {
                        let (parts, _) =
                            energy::ledger(sample, qx_physics::FIXED_STEP_SECONDS / 4.);
                        for (sum, x) in sums.iter_mut().zip(parts) {
                            *sum += x;
                        }
                    }
                    let e = a.model.evaluate(&r, &a.state.poses, Kind::B1).unwrap();
                    let (_, loads) =
                        original_dynamics::instantaneous(&a.model, &r, &a.state, energy::G)
                            .unwrap();
                    if [480, 600, 720].contains(&tick) {
                        checkpoint(name, tick, &a, &r);
                    }
                    let mut support = vec![];
                    for (index, s) in a.model.supports.iter().enumerate() {
                        let x = guidance::point(a.state.poses[s.owner], s.local);
                        let (station, j) =
                            r.project(s.surface, x, s.station, s.half_window).unwrap();
                        let delta = x - j.p;
                        let f = delta.dot(j.d);
                        let d = j.d.dot(j.d) - delta.dot(j.dd);
                        assert!(
                            f.abs() < 2e-13 && d > 1e-8 && station > 0. && station < r.length()
                        );
                        let source = r.source.evaluate(ID, station, Side::Right).unwrap();
                        assert_eq!(source.source, ID);
                        let grad = j.n + j.d * ((delta.dot(j.dn) - j.d.dot(j.n)) / d);
                        let force = (grad * loads[index]).length();
                        support.push([
                            s.owner as f64,
                            x.x,
                            x.y,
                            x.z,
                            j.p.x,
                            j.p.y,
                            j.p.z,
                            j.n.x,
                            j.n.y,
                            j.n.z,
                            station,
                            delta.dot(j.n),
                            f,
                            d,
                            source.chart as f64,
                            force,
                        ]);
                    }
                    let bodies: Vec<_> = a
                        .state
                        .poses
                        .iter()
                        .map(|p| {
                            let q = p.orientation;
                            [
                                p.position_m.x,
                                p.position_m.y,
                                p.position_m.z,
                                q.w,
                                q.x,
                                q.y,
                                q.z,
                            ]
                        })
                        .collect();
                    let anchors: Vec<_> = a
                        .model
                        .bearings
                        .iter()
                        .map(|b| {
                            let x = guidance::point(a.state.poses[b.a], b.local_a);
                            let y = guidance::point(a.state.poses[b.b], b.local_b);
                            [x.x, x.y, x.z, y.x, y.y, y.z]
                        })
                        .collect();
                    let swing: Vec<_> = a
                        .model
                        .bearings
                        .iter()
                        .map(|b| {
                            a.state.poses[b.a]
                                .orientation
                                .compose(b.frame_a)
                                .conjugate()
                                .compose(a.state.poses[b.b].orientation.compose(b.frame_b))
                                .rotation_vector()
                                .unwrap()
                                .length()
                        })
                        .collect();
                    let roll: Vec<_> = a
                        .model
                        .bearings
                        .iter()
                        .filter(|b| b.roll)
                        .map(|b| {
                            guidance::twist(
                                a.state.poses[b.a].orientation.compose(b.frame_a),
                                a.state.poses[b.b].orientation.compose(b.frame_b),
                            )
                        })
                        .collect();
                    let drift = energy::energies(&a.state).iter().sum::<f64>() - start;
                    assert!((drift - sums[0] + sums[1] - sums[2]).abs() < 1e-7);
                    assert!(
                        max_abs(&e.c) < 2e-5 && max_abs(&multiply(&e.j, &a.state.velocity)) < 0.003
                    );
                    let envelopes: Vec<_> = a
                        .envelopes
                        .iter()
                        .map(|e| [e.rate, e.innovation, e.half])
                        .collect();
                    println!(
                        "M4H_LONG_FRAME {{\"fixture\":\"{name}\",\"tick\":{tick},\"bodies\":{bodies:?},\"velocity\":{:?},\"supports\":{support:?},\"anchors\":{anchors:?},\"swing\":{swing:?},\"roll\":{roll:?},\"constraints\":{:?},\"velocity_rows\":{:?},\"ledger\":{sums:?},\"drift\":{drift},\"momentum\":{},\"envelopes\":{envelopes:?}}}",
                        a.state.velocity,
                        e.c,
                        multiply(&e.j, &a.state.velocity),
                        report.linear_balance
                    );
                    last = tick;
                }
            }
        }
        assert!(last >= 480);
        if ![480, 600, 720].contains(&last) {
            checkpoint(name, last, &a, &r);
        }
        println!("M4H_LONG_DONE {{\"fixture\":\"{name}\",\"accepted\":{last}}}");
    }
}

fn checkpoint(name: &str, tick: usize, a: &measured_tracking::Tracked, r: &Rail) {
    let e = a.model.evaluate(r, &a.state.poses, Kind::B1).unwrap();
    let (acceleration, loads) =
        original_dynamics::instantaneous(&a.model, r, &a.state, energy::G).unwrap();
    let gamma = a
        .model
        .curvature(r, &a.state.poses, &a.state.velocity, Kind::B1, 1e-5)
        .unwrap();
    let finite = finite_jacobian(a.state.velocity.len(), 1e-6, |d| {
        a.model
            .evaluate(r, &a.model.shifted(&a.state.poses, d), Kind::B1)
            .unwrap()
            .c
    });
    assert!(
        e.j.iter()
            .flatten()
            .zip(finite.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .fold(0., f64::max)
            < 3e-7
    );
    let refined = a
        .model
        .curvature(r, &a.state.poses, &a.state.velocity, Kind::B1, 5e-6)
        .unwrap();
    assert!(
        gamma
            .iter()
            .zip(refined)
            .map(|(a, b)| (a - b).abs())
            .fold(0., f64::max)
            < 1e-6
    );
    println!(
        "M4H_LONG_SYSTEM {{\"fixture\":\"{name}\",\"tick\":{tick},\"j\":{:?},\"v\":{:?},\"gamma\":{gamma:?},\"loads\":{loads:?},\"acceleration\":{acceleration:?}}}",
        e.j, a.state.velocity
    );
}
