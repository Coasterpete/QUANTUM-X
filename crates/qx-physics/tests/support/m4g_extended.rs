//! M4G qualification only. No production solver or inherited equations change.
use super::*;
use original_dynamics::Step;

const GRAVITY: Vec3 = Vec3::new(0., -9.81, 0.);

struct Record {
    state: original_dynamics::State,
    report: Step,
    hints: Vec<f64>,
    envelopes: Vec<tracking::Envelope>,
}

fn record(t: &tracking::Tracked, report: Step) -> Record {
    Record {
        state: t.state.clone(),
        report,
        hints: t.hints(),
        envelopes: t.envelopes.clone(),
    }
}

pub(super) fn fixture(name: &str) -> (InitialConditions, Vec<Section>, f64) {
    if name == "helix" {
        let (_, initial, sections) = fixtures::fixtures().remove(2);
        (initial, sections, 8.025)
    } else {
        assert_eq!(name, "foundation");
        let p = qx_document::sample::foundation_project();
        (p.initial, p.sections[..2].to_vec(), 11.625)
    }
}

fn run(name: &str) -> (Rail, tracking::Tracked, Vec<Record>) {
    let (initial, sections, lead) = fixture(name);
    let r = rail(&initial, &sections);
    let mut t = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    let e = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap();
    let initial = Step {
        loads: vec![0.; e.c.len()],
        max_position: max_abs(&e.c),
        max_velocity: max_abs(&multiply(&e.j, &t.state.velocity)),
        energy: original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, GRAVITY),
        linear_balance: 0.,
    };
    let mut records = vec![record(&t, initial)];
    for _ in 0..480 {
        let report = qualified_step(&mut t, &r).unwrap();
        records.push(record(&t, report));
    }
    (r, t, records)
}

/// Extend the inherited transaction to include its stricter qualification gates.
/// A rejected candidate never becomes a recorded/accepted physical state.
fn qualified_step(t: &mut tracking::Tracked, r: &Rail) -> surface::Result<Step> {
    let before = t.state.clone();
    let supports = t.model.supports.clone();
    let envelopes = t.envelopes.clone();
    let ticks = t.ticks;
    let result = t.step(r, GRAVITY).and_then(|report| {
        if report.max_position >= 2e-5 || report.max_velocity >= 0.003 {
            println!(
                "M4G_REJECT candidate_tick={} p={} v={}",
                ticks + 1,
                report.max_position,
                report.max_velocity
            );
            Err(surface::Failure::Residual)
        } else {
            Ok(report)
        }
    });
    if result.is_err() {
        t.state = before;
        t.model.supports = supports;
        t.envelopes = envelopes;
        t.ticks = ticks;
    }
    result
}

#[test]
fn duration_ladder_and_exact_replay() {
    let (_, initial, sections) = fixtures::fixtures().remove(2);
    let p = qx_document::sample::foundation_project();
    for (name, initial, sections, lead) in [
        ("R8-twist-original-12m", initial, sections, 8.025),
        (
            "foundation-prefix-72m",
            p.initial,
            p.sections[..2].to_vec(),
            11.625,
        ),
    ] {
        let r = rail(&initial, &sections);
        let mut t = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
        let mut repeat = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
        let start = original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, GRAVITY);
        let (mut mp, mut mv, mut me) = (0_f64, 0_f64, 0_f64);
        for tick in 1..=480 {
            let before = t.state.clone();
            let hints = t.hints();
            let envelopes = t.envelopes.clone();
            let result = qualified_step(&mut t, &r);
            let replay = qualified_step(&mut repeat, &r);
            assert_eq!(result, replay);
            assert_eq!(t.state, repeat.state);
            assert_eq!(t.hints(), repeat.hints());
            assert_eq!(t.envelopes, repeat.envelopes);
            match result {
                Ok(report) => {
                    mp = mp.max(report.max_position);
                    mv = mv.max(report.max_velocity);
                    me = me.max((report.energy - start).abs());
                    assert!(t.hints().iter().all(|s| *s > 0. && *s < r.length()));
                    if [120, 240, 480].contains(&tick) {
                        println!(
                            "M4G_TARGET {name} tick={tick} p={mp} v={mv} drift_J={me} stations={:?}",
                            t.hints()
                        );
                    }
                }
                Err(reason) => {
                    assert_eq!(t.state, before);
                    assert_eq!(t.hints(), hints);
                    assert_eq!(t.envelopes, envelopes);
                    assert_eq!(t.ticks, tick - 1);
                    println!(
                        "M4G_STOP {name} first_failure={tick} last_valid={} reason={reason:?} p={mp} v={mv} drift_J={me} stations={hints:?}",
                        t.ticks
                    );
                    break;
                }
            }
        }
        assert_eq!(
            t.ticks, 480,
            "{name} must pass every qualified duration target"
        );
    }
}

/// Extract only AFTER integration; all poses/velocities are recorded states.
/// Diagnostic source projections never set physical poses or station hints.
fn diagnostics(
    name: &str,
    r: &Rail,
    t: &mut tracking::Tracked,
    records: &[Record],
) -> (Vec<String>, std::time::Duration, Vec<[f64; 2]>) {
    let mut output = Vec::new();
    let mut serialization = std::time::Duration::ZERO;
    let mut samples = Vec::new();
    use qx_geometry::math::Frame;
    let (_, _, lead) = fixture(name);
    let labels = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        true,
    );
    let roles: Vec<_> = (0..t.state.poses.len())
        .map(|i| {
            if labels.cars.contains(&i) {
                "Car"
            } else if labels.bogies.contains(&i) {
                "Bogie"
            } else {
                "Carrier"
            }
        })
        .collect();
    let track: Vec<_> = (0..=360)
        .map(|i| {
            let s = r.length() * i as f64 / 360.;
            [-0.6, 0., 0.6].map(|z| {
                array(
                    r.jet(
                        Surface {
                            rail: "display-only",
                            face: "running",
                            offset: Vec3::new(0., 0., z),
                            role: Role::Running,
                        },
                        s,
                    )
                    .unwrap()
                    .p,
                )
            })
        })
        .collect();
    let running: Vec<_> = t
        .model
        .supports
        .iter()
        .map(|s| s.surface.role == Role::Running)
        .collect();
    let edges: Vec<_> = t.model.bearings.iter().map(|b| [b.a, b.b]).collect();
    let locals: Vec<_> = t.model.supports.iter().map(|s| array(s.local)).collect();
    let bearing_locals: Vec<_> = t
        .model
        .bearings
        .iter()
        .map(|b| [array(b.local_a), array(b.local_b)])
        .collect();
    let charts: Vec<_> = r
        .source
        .segments()
        .iter()
        .map(|c| {
            [
                c.start_station_m,
                c.end_station_m,
                c.chart as f64,
                r.source
                    .evaluate(ID, (c.start_station_m + c.end_station_m) * 0.5, Side::Right)
                    .unwrap()
                    .section
                    .0 as f64,
            ]
        })
        .collect();
    let format_clock = std::time::Instant::now();
    output.push(format!(
        "M4G_SCENE {{\"title\":\"{name}: unchanged graph source / shared bogie\",\"fixture\":\"{name}\",\"source\":[{},{}],\"domain\":[0,{}],\"dt\":{},\"roles\":{roles:?},\"running\":{running:?},\"edges\":{edges:?},\"track\":{track:?},\"locals\":{locals:?},\"bearing_locals\":{bearing_locals:?},\"charts\":{charts:?}}}",
        ID.document,
        ID.generation,
        r.length(),
        qx_physics::FIXED_STEP_SECONDS
    ));
    serialization += format_clock.elapsed();
    let mut body_hints: Vec<_> = labels.poses.iter().map(|p| p.position_m.x).collect();
    for (tick, record) in records.iter().enumerate() {
        let tick_clock = std::time::Instant::now();
        let serialization_start = serialization;
        for (s, hint) in t.model.supports.iter_mut().zip(&record.hints) {
            s.station = *hint;
        }
        let state = &record.state;
        let e = t.model.evaluate(r, &state.poses, Kind::B1).unwrap();
        let (acceleration, reactions) =
            original_dynamics::instantaneous(&t.model, r, state, GRAVITY).unwrap();
        let gamma = t
            .model
            .curvature(r, &state.poses, &state.velocity, Kind::B1, 1e-5)
            .unwrap();
        // Independent matrix verifier consumes genuine long-run states, including
        // near turnaround and each target, with finite stabilized residuals.
        if [0, 24, 120, 240, 360, 480].contains(&tick) {
            let format_clock = std::time::Instant::now();
            output.push(format!(
                "M4G_SYSTEM {{\"name\":\"{name}-{tick}\",\"j\":{:?},\"v\":{:?},\"gamma\":{gamma:?},\"loads\":{reactions:?},\"acceleration\":{acceleration:?}}}",
                e.j, state.velocity
            ));
            serialization += format_clock.elapsed();
            let finite = finite_jacobian(state.velocity.len(), 1e-6, |d| {
                t.model
                    .evaluate(r, &t.model.shifted(&state.poses, d), Kind::B1)
                    .unwrap()
                    .c
            });
            assert!(error(&e.j.concat(), &finite.concat()) < 3e-7);
            let refined = t
                .model
                .curvature(r, &state.poses, &state.velocity, Kind::B1, 5e-6)
                .unwrap();
            assert!(error(&gamma, &refined) < 1e-6);
        }
        let bodies: Vec<_> = state
            .poses
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (s, j) = r
                    .project(
                        Surface {
                            rail: "center-measurement",
                            face: "normal",
                            offset: Vec3::ZERO,
                            role: Role::Running,
                        },
                        p.position_m,
                        body_hints[i],
                        0.8,
                    )
                    .unwrap();
                body_hints[i] = s;
                let angle = j
                    .frame
                    .orientation()
                    .conjugate()
                    .compose(p.orientation)
                    .rotation_vector()
                    .unwrap()
                    .length()
                    .to_degrees();
                let q = p.orientation;
                [
                    p.position_m.x,
                    p.position_m.y,
                    p.position_m.z,
                    q.w,
                    q.x,
                    q.y,
                    q.z,
                    angle,
                ]
            })
            .collect();
        let mut charts = Vec::new();
        let mut sections = Vec::new();
        let mut stationarity = Vec::new();
        let mut denominators = Vec::new();
        let mut reaction_forces = Vec::new();
        let supports: Vec<_> = t
            .model
            .supports
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let p = state.poses[s.owner];
                let x = guidance::point(p, s.local);
                let (station, j) = r.project(s.surface, x, s.station, s.half_window).unwrap();
                assert!((station - record.hints[i]).abs() < 2e-12);
                let source = r.source.evaluate(ID, station, Side::Right).unwrap();
                assert_eq!(source.source, ID);
                charts.push(source.chart);
                sections.push(source.section.0);
                let delta = x - j.p;
                let f = delta.dot(j.d);
                let d = j.d.dot(j.d) - delta.dot(j.dd);
                assert!(f.abs() < 2e-13 && d > 1e-8 && station > 0. && station < r.length());
                stationarity.push(f);
                denominators.push(d);
                let g = j.n + j.d * ((delta.dot(j.dn) - j.d.dot(j.n)) / d);
                reaction_forces.push(array(g * reactions[i]));
                let angle = Frame::new(p.orientation)
                    .unwrap()
                    .tangent()
                    .dot(j.d / j.d.length())
                    .clamp(-1., 1.)
                    .acos()
                    .to_degrees();
                [
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
                    angle,
                ]
            })
            .collect();
        let anchors: Vec<_> = t
            .model
            .bearings
            .iter()
            .map(|b| {
                let a = guidance::point(state.poses[b.a], b.local_a);
                let c = guidance::point(state.poses[b.b], b.local_b);
                [a.x, a.y, a.z, c.x, c.y, c.z]
            })
            .collect();
        let bearing_gap = anchors
            .iter()
            .map(|a| (Vec3::new(a[0], a[1], a[2]) - Vec3::new(a[3], a[4], a[5])).length())
            .fold(0., f64::max);
        let swing: Vec<_> = t
            .model
            .bearings
            .iter()
            .map(|b| {
                state.poses[b.a]
                    .orientation
                    .compose(b.frame_a)
                    .conjugate()
                    .compose(state.poses[b.b].orientation.compose(b.frame_b))
                    .rotation_vector()
                    .unwrap()
                    .length()
            })
            .collect();
        let roll: Vec<_> = t
            .model
            .bearings
            .iter()
            .filter(|b| b.roll)
            .map(|b| {
                guidance::twist(
                    state.poses[b.a].orientation.compose(b.frame_a),
                    state.poses[b.b].orientation.compose(b.frame_b),
                )
            })
            .collect();
        let envelopes: Vec<_> = record
            .envelopes
            .iter()
            .map(|e| [e.rate, e.innovation, e.half])
            .collect();
        let velocity_rows = multiply(&e.j, &state.velocity);
        let power = contract::work(&e.j, &state.velocity, &reactions);
        let format_clock = std::time::Instant::now();
        output.push(format!(
            "M4G_FRAME {{\"tick\":{tick},\"time\":{},\"bodies\":{bodies:?},\"velocity\":{:?},\"supports\":{supports:?},\"anchors\":{anchors:?},\"max_position\":{},\"max_velocity\":{},\"energy_drift\":{},\"bearing_gap\":{bearing_gap},\"swing\":{swing:?},\"roll\":{roll:?},\"constraints\":{:?},\"velocity_rows\":{velocity_rows:?},\"reactions\":{reactions:?},\"reaction_forces\":{reaction_forces:?},\"impulse_loads\":{:?},\"reaction_power\":{power},\"linear_balance\":{},\"charts\":{charts:?},\"sections\":{sections:?},\"stationarity\":{stationarity:?},\"denominators\":{denominators:?},\"envelopes\":{envelopes:?}}}",
            tick as f64 * qx_physics::FIXED_STEP_SECONDS,
            state.velocity,
            record.report.max_position,
            record.report.max_velocity,
            record.report.energy - records[0].report.energy,
            e.c,
            record.report.loads,
            record.report.linear_balance
        ));
        serialization += format_clock.elapsed();
        let format_time = serialization - serialization_start;
        samples.push([
            (tick_clock.elapsed() - format_time).as_secs_f64() * 1000.,
            format_time.as_secs_f64() * 1000.,
        ]);
    }
    (output, serialization, samples)
}

#[test]
#[ignore = "native Release full per-tick qualification and offline diagnostic extraction"]
fn export_extended_states() {
    for name in ["helix", "foundation"] {
        let (r, mut t, records) = run(name);
        use std::io::Write;
        use std::time::Instant;
        let clock = Instant::now();
        let (lines, serialization, samples) = diagnostics(name, &r, &mut t, &records);
        let calculations = clock.elapsed() - serialization;
        let clock = Instant::now();
        let mut out = std::io::stdout().lock();
        for line in &lines {
            writeln!(out, "{line}").unwrap();
        }
        out.flush().unwrap();
        let export = clock.elapsed();
        println!(
            "M4G_RECORD_COST {{\"fixture\":\"{name}\",\"diagnostics_ms\":{},\"serialization_ms\":{},\"export_ms\":{},\"records\":{},\"bytes\":{},\"per_tick_ms\":{samples:?}}}",
            calculations.as_secs_f64() * 1000.,
            serialization.as_secs_f64() * 1000.,
            export.as_secs_f64() * 1000.,
            records.len(),
            lines.iter().map(|s| s.len() + 1).sum::<usize>()
        );
    }
}

#[test]
fn extended_transaction_rejects_late_failure_without_state_or_hint_changes() {
    let (initial, sections, lead) = fixture("helix");
    let r = rail(&initial, &sections);
    let mut t = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    let before = t.state.clone();
    let hints = t.hints();
    let windows: Vec<_> = t.model.supports.iter().map(|s| s.half_window).collect();
    t.substeps = 3;
    for _ in 0..2 {
        assert_eq!(
            qualified_step(&mut t, &r).unwrap_err(),
            surface::Failure::Invalid
        );
        assert_eq!(t.state, before);
        assert_eq!(t.hints(), hints);
        assert_eq!(t.ticks, 0);
        assert!(t.envelopes.is_empty());
        assert_eq!(
            t.model
                .supports
                .iter()
                .map(|s| s.half_window)
                .collect::<Vec<_>>(),
            windows
        );
    }
    t.substeps = 4;
    for v in &mut t.state.velocity {
        *v *= 2.;
    }
    let fast = t.state.clone();
    assert_eq!(
        qualified_step(&mut t, &r).unwrap_err(),
        surface::Failure::Residual
    );
    assert_eq!(t.state, fast);
    assert_eq!(t.hints(), hints);
    assert_eq!(t.ticks, 0);
    assert!(t.envelopes.is_empty());
}
