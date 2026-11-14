//! Exact-key experiment. No approximation, tolerance or solver change.
use super::*;
use qx_physics::FIXED_STEP_SECONDS as DT;
use std::time::Instant;
use surface::{Role, Surface};

fn jet_bits(j: surface::Jet) -> Vec<u64> {
    let q = j.frame.orientation();
    [j.p, j.d, j.dd, j.n, j.dn, j.k]
        .into_iter()
        .flat_map(array)
        .chain([q.w, q.x, q.y, q.z])
        .map(f64::to_bits)
        .collect()
}

pub(super) fn state_bits(s: &original_dynamics::State) -> Vec<u64> {
    s.poses
        .iter()
        .flat_map(|p| {
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
        .chain(s.velocity.iter().copied())
        .map(f64::to_bits)
        .collect()
}

fn report_bits(s: &original_dynamics::Step) -> Vec<u64> {
    s.loads
        .iter()
        .copied()
        .chain([s.max_position, s.max_velocity, s.energy, s.linear_balance])
        .map(f64::to_bits)
        .collect()
}

pub(super) fn pair(
    a: &measured_tracking::Tracked,
    b: &tracking::Tracked,
    x: &original_dynamics::Step,
    y: &original_dynamics::Step,
) {
    assert_eq!(report_bits(x), report_bits(y));
    assert_eq!(state_bits(&a.state), state_bits(&b.state));
    assert_eq!(
        a.hints().into_iter().map(f64::to_bits).collect::<Vec<_>>(),
        b.hints().into_iter().map(f64::to_bits).collect::<Vec<_>>()
    );
    assert_eq!(
        a.envelopes
            .iter()
            .flat_map(|e| [e.rate, e.innovation, e.half])
            .map(f64::to_bits)
            .collect::<Vec<_>>(),
        b.envelopes
            .iter()
            .flat_map(|e| [e.rate, e.innovation, e.half])
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        a.model
            .supports
            .iter()
            .map(|s| s.half_window.to_bits())
            .collect::<Vec<_>>(),
        b.model
            .supports
            .iter()
            .map(|s| s.half_window.to_bits())
            .collect::<Vec<_>>()
    );
    assert_eq!(a.ticks, b.ticks);
}

#[test]
fn exact_keys_offsets_charts_generations_hints_and_failures() {
    let (i, s, _) = energy::fixture("foundation");
    let r = rail(&i, &s);
    r.reuse.set(true);
    let datum = Surface {
        rail: "right",
        face: "running",
        offset: Vec3::new(0., 0., 0.6),
        role: Role::Running,
    };
    let join = r
        .source
        .segments()
        .iter()
        .find(|c| c.chart == 1)
        .unwrap()
        .start_station_m;
    for station in [
        0.,
        5.,
        join - f64::EPSILON * join,
        join,
        join + f64::EPSILON * join,
        r.length(),
    ] {
        for offset in [
            Vec3::new(0., 0., 0.6),
            Vec3::new(0., -0.08, 0.6),
            Vec3::new(0., 0., -0.6),
        ] {
            for role in [Role::Running, Role::Side] {
                let surface = Surface {
                    offset,
                    role,
                    ..datum
                };
                r.reuse.set(false);
                let control = r.differential(surface, station).unwrap();
                r.reuse.set(true);
                let cached = r.differential(surface, station).unwrap();
                assert_eq!(jet_bits(control.jet), jet_bits(cached.jet));
                assert_eq!(
                    array(control.ddd).map(f64::to_bits),
                    array(cached.ddd).map(f64::to_bits)
                );
                assert_eq!(
                    array(control.ddn).map(f64::to_bits),
                    array(cached.ddn).map(f64::to_bits)
                );
                assert_eq!(
                    jet_bits(cached.jet),
                    jet_bits(r.jet(surface, station).unwrap())
                );
            }
        }
    }
    let x = r.jet(datum, join).unwrap().p;
    // Same point with changed hint/window/offset/face/role must not alias.
    for hint in [join - 0.001, join, join + 0.001] {
        for half in [0.02, 0.04] {
            r.reuse.set(false);
            let control = r.project(datum, x, hint, half).unwrap();
            r.reuse.set(true);
            let cached = r.project(datum, x, hint, half).unwrap();
            assert_eq!(control.0.to_bits(), cached.0.to_bits());
            assert_eq!(jet_bits(control.1), jet_bits(cached.1));
        }
    }
    let before = r.counts.get()[5];
    r.project(datum, x, join, 0.04).unwrap();
    assert_eq!(r.counts.get()[5], before + 1);
    // A populated cache must not return the old root outside a changed window.
    r.reuse.set(false);
    let remote = r.project(datum, x, join + 1., 0.04).unwrap_err();
    r.reuse.set(true);
    assert_eq!(r.project(datum, x, join + 1., 0.04).unwrap_err(), remote);
    let point = r.jet(datum, 20.).unwrap().p + Vec3::new(0.03, 0.02, 0.01);
    let shifted = Surface {
        offset: Vec3::new(0.1, -0.08, 0.6),
        ..datum
    };
    for surface in [
        datum,
        shifted,
        Surface {
            role: Role::Side,
            ..shifted
        },
    ] {
        r.reuse.set(false);
        let original = r.project(surface, point, 20., 0.4).unwrap();
        r.reuse.set(true);
        let reused = r.project(surface, point, 20., 0.4).unwrap();
        assert_eq!(original.0.to_bits(), reused.0.to_bits());
        assert_eq!(jet_bits(original.1), jet_bits(reused.1));
    }
    let mut other = datum;
    other.face = "other-face";
    r.project(other, x, join, 0.04).unwrap();
    assert_eq!(r.counts.get()[5], before + 1);
    assert_eq!(
        r.project(datum, x, join, 0.).unwrap_err(),
        surface::Failure::Invalid
    );
    assert_eq!(
        r.jet(datum, f64::NAN).unwrap_err(),
        surface::Failure::Invalid
    );
    assert_eq!(
        r.jet(
            Surface {
                offset: Vec3::new(3., 0., 0.),
                ..datum
            },
            join
        )
        .unwrap_err(),
        surface::Failure::Invalid
    );
    let changed = SourceIdentity {
        generation: ID.generation + 1,
        ..ID
    };
    let graph = || {
        ContinuousGraph::prepare(
            changed,
            AuthoredReference::TrackCenterline,
            &i,
            &s,
            GenerationOptions::default(),
        )
        .unwrap()
    };
    assert!(Rail::new(graph(), ID).is_err());
    let fresh = Rail::new(graph(), changed).unwrap();
    fresh.reuse.set(true);
    fresh.jet(datum, 5.).unwrap();
    assert_eq!(fresh.counts.get()[1], 0);
    assert_eq!(
        r.source.evaluate(changed, 5., Side::Right).unwrap_err().0,
        "stale source identity"
    );
    // Even a caller's erroneously reused token cannot share entries across immutable instances.
    let mut altered = i.clone();
    altered.position_m.y += 1.;
    let new_source = rail(&altered, &s);
    new_source.reuse.set(true);
    assert!((new_source.jet(datum, 5.).unwrap().p - r.jet(datum, 5.).unwrap().p).length() > 0.99);
    r.begin();
    assert_eq!(r.counts.get(), [0; 8]);
    r.jet(datum, 5.).unwrap();
    assert_eq!(r.counts.get()[3], 0);
}

#[test]
#[ignore = "serial Release: warm-up plus five paired 480-tick runs on each unchanged fixture"]
fn repeated_complete_steps() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = energy::fixture(name);
        let cached = rail(&i, &s);
        let control = rail(&i, &s);
        cached.reuse.set(true);
        for run in 0..6 {
            cached.begin();
            control.begin();
            let mut a =
                measured_tracking::Tracked::new(model(&cached, lead, true), &cached, 6.).unwrap();
            let mut b = tracking::Tracked::new(model(&control, lead, true), &control, 6.).unwrap();
            for tick in 1..=480 {
                let timed_cached = |a: &mut measured_tracking::Tracked| {
                    let start = Instant::now();
                    cached.begin();
                    let r = a.step(&cached, energy::G).unwrap();
                    (r, start.elapsed())
                };
                let timed_control = |b: &mut tracking::Tracked| {
                    let start = Instant::now();
                    let r = b.step(&control, energy::G).unwrap();
                    (r, start.elapsed())
                };
                let ((x, ct), (y, ot)) = if (run + tick) % 2 == 0 {
                    (timed_cached(&mut a), timed_control(&mut b))
                } else {
                    let original = timed_control(&mut b);
                    (timed_cached(&mut a), original)
                };
                pair(&a, &b, &x, &y);
                assert!(x.max_position < 2e-5 && x.max_velocity < 0.003);
                if run > 0 {
                    println!(
                        "M4H_COST {{\"fixture\":\"{name}\",\"run\":{run},\"tick\":{tick},\"cached_ms\":{},\"original_ms\":{},\"counts\":{:?}}}",
                        ct.as_secs_f64() * 1000.,
                        ot.as_secs_f64() * 1000.,
                        cached.counts.get()
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "separate instrumented Release attribution, not complete-step deadline evidence"]
fn geometry_and_diagnostic_attribution() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = energy::fixture(name);
        for cached in [false, true] {
            let r = rail(&i, &s);
            let control = rail(&i, &s);
            r.reuse.set(cached);
            r.profiling.set(true);
            let mut a = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            let mut b = tracking::Tracked::new(model(&control, lead, true), &control, 6.).unwrap();
            for tick in 1..=480 {
                r.begin();
                let start = Instant::now();
                let x = a.step(&r, energy::G).unwrap();
                let complete = start.elapsed();
                let counts = r.counts.get();
                let costs = r.costs.get().map(|d| d.as_secs_f64() * 1000.);
                let y = b.step(&control, energy::G).unwrap();
                pair(&a, &b, &x, &y);
                // Separate reaction/J/Jdot extraction. Never included in mechanical timer.
                r.begin();
                let start = Instant::now();
                let diagnostic =
                    original_dynamics::instantaneous(&a.model, &r, &a.state, energy::G).unwrap();
                let reaction_time = start.elapsed();
                let dc = r.counts.get();
                let times = r.costs.get().map(|d| d.as_secs_f64() * 1000.);
                let original =
                    original_dynamics::instantaneous(&b.model, &control, &b.state, energy::G)
                        .unwrap();
                assert_eq!(
                    diagnostic
                        .0
                        .iter()
                        .chain(&diagnostic.1)
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    original
                        .0
                        .iter()
                        .chain(&original.1)
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>()
                );
                println!(
                    "M4H_PROFILE {{\"fixture\":\"{name}\",\"cached\":{cached},\"tick\":{tick},\"complete_ms\":{},\"counts\":{counts:?},\"costs_ms\":{costs:?},\"reaction_ms\":{},\"diagnostic_counts\":{dc:?},\"diagnostic_costs_ms\":{times:?},\"deadline_ms\":{}}}",
                    complete.as_secs_f64() * 1000.,
                    reaction_time.as_secs_f64() * 1000.,
                    DT * 1000.
                );
            }
        }
    }
}
