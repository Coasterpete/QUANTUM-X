//! Diagnostic decompositions only. No result here authorizes a step.
use super::*;
use energy_dynamics::Sample;
use matrix::{max_abs, multiply};
use original_dynamics::{State, Step};

pub fn acceptable(s: &Step) -> bool {
    s.max_position < 2e-5 && s.max_velocity < 0.003
}
pub fn difference(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    assert!(a.iter().chain(b).all(|x| x.is_finite()));
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0., f64::max)
}
pub fn state_bits(s: &State) -> Vec<u64> {
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
pub fn energy(s: &State) -> f64 {
    s.poses
        .iter()
        .zip(s.velocity.as_chunks::<6>().0)
        .map(|(p, v)| {
            analytic::energy(
                p.position_m,
                Vec3::new(v[0], v[1], v[2]),
                Vec3::new(v[3], v[4], v[5]),
            )
        })
        .sum()
}
/// Frozen spatial velocity is advanced with the same exponential pose convention.
/// Differentiate only GAP VALUES; no Jacobian or impulse forms this reference.
fn gap_curvature(m: &Model, r: &Rail, s: &Sample, epsilon: f64) -> Vec<f64> {
    let c = |scale: f64| {
        m.evaluate(
            r,
            &m.shifted(
                &s.before.poses,
                &s.after
                    .velocity
                    .iter()
                    .map(|v| scale * v)
                    .collect::<Vec<_>>(),
            ),
            Kind::B1,
        )
        .unwrap()
        .c
    };
    let a = c(epsilon);
    let b = c(-epsilon);
    a.iter()
        .zip(b)
        .zip(&s.c)
        .map(|((a, b), c)| (a - 2. * c + b) / (epsilon * epsilon))
        .collect()
}
pub fn classify(
    m: &Model,
    r: &Rail,
    s: &Sample,
    h: f64,
    emit: bool,
    label: &str,
    sub: usize,
) -> [f64; 3] {
    let e = m.evaluate(r, &s.after.poses, Kind::B1).unwrap();
    let j0v1 = multiply(&s.j, &s.after.velocity);
    let residual = multiply(&e.j, &s.after.velocity);
    let frozen = m
        .curvature(r, &s.before.poses, &s.after.velocity, Kind::B1, 1e-5)
        .unwrap();
    let second = gap_curvature(m, r, s, 2e-4);
    let refined = gap_curvature(m, r, s, 1e-4);
    // Retain the inherited independent second-difference curvature gate.
    assert!(difference(&second, &frozen) < 2e-5);
    assert!(difference(&refined, &frozen) < 2e-5);
    let mut sweep = vec![];
    let mut stabilization = vec![];
    let mut curvature = vec![];
    let mut ec = vec![];
    let mut er = vec![];
    for k in 0..s.c.len() {
        let bias = -0.2 * s.c[k] / h;
        let finite = j0v1[k] + h * s.gamma[k] - bias;
        let curve = h * (frozen[k] - s.gamma[k]);
        stabilization.push(bias);
        sweep.push(finite);
        curvature.push(curve);
        er.push(residual[k] - bias - finite - curve);
        ec.push(
            e.c[k]
                - ((1. - 0.2) * s.c[k] + h * finite - h * h * s.gamma[k] + 0.5 * h * h * frozen[k]),
        );
    }
    assert!(ec.iter().chain(&er).all(|x| x.is_finite()));
    if emit {
        println!(
            "M4I_CLASS {{\"label\":\"{label}\",\"sub\":{sub},\"h\":{h},\"c0\":{:?},\"c1\":{:?},\"gamma\":{:?},\"frozen\":{frozen:?},\"gap_second\":{second:?},\"gap_second_refined\":{refined:?},\"r\":{residual:?},\"stabilization\":{stabilization:?},\"sweep\":{sweep:?},\"curvature\":{curvature:?},\"unexplained_c\":{ec:?},\"unexplained_r\":{er:?},\"rail_rows\":{}}}",
            s.c,
            e.c,
            s.gamma,
            m.supports.len()
        );
    }
    [max_abs(&ec), max_abs(&er), max_abs(&sweep)]
}

pub fn regularity(m: &Model, r: &Rail, s: &State, emit: bool) {
    let mut supports = vec![];
    for p in &m.supports {
        let x = guidance::point(s.poses[p.owner], p.local);
        let (station, j) = r.project(p.surface, x, p.station, p.half_window).unwrap();
        let delta = x - j.p;
        let f = delta.dot(j.d);
        let d = j.d.dot(j.d) - delta.dot(j.dd);
        let margin = (station - (p.station - p.half_window).max(0.))
            .min((p.station + p.half_window).min(r.length()) - station);
        assert!(f.abs() < 2e-13 && d > 1e-8 && margin > 0. && station > 0. && station < r.length());
        let source = r.source.evaluate(ID, station, Side::Right).unwrap();
        assert_eq!(source.source, ID);
        supports.push([
            station,
            delta.dot(j.n),
            f,
            d,
            margin,
            source.chart as f64,
            source.refinement_position_difference_m,
            source.refinement_orientation_difference_rad,
        ]);
    }
    let mut bearings = vec![];
    for b in &m.bearings {
        let qa = s.poses[b.a].orientation.compose(b.frame_a);
        let qb = s.poses[b.b].orientation.compose(b.frame_b);
        let swing = qa
            .conjugate()
            .compose(qb)
            .rotation_vector()
            .unwrap()
            .length();
        let gap = (guidance::point(s.poses[b.a], b.local_a)
            - guidance::point(s.poses[b.b], b.local_b))
        .length();
        let roll = if b.roll { guidance::twist(qa, qb) } else { 0. };
        assert!(swing < b.max_swing && gap < 2e-5 && roll.abs() < 2e-5);
        bearings.push([b.a as f64, b.b as f64, gap, swing, roll]);
    }
    if emit {
        println!("M4I_REGULAR {{\"supports\":{supports:?},\"bearings\":{bearings:?}}}");
    }
}

#[test]
fn continuity_rejects_nonfinite_and_detects_ten_micrometres() {
    let (i, s, lead) = fixture("helix");
    let r = rail(&i, &s);
    r.reuse.set(true);
    let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    t.capture = true;
    for _ in 0..3 {
        r.begin();
        assert!(acceptable(&t.step(&r, G).unwrap()));
    }
    let gap = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap().c;
    let clean = t.state.clone();
    assert_eq!(
        difference(
            &gap,
            &t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap().c
        ),
        0.
    );
    t.state.poses[8].position_m = t.state.poses[8].position_m + Vec3::Y * 1e-5;
    let disturbed = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap().c;
    assert!(difference(&gap, &disturbed) > 9.9e-6);
    assert_ne!(state_bits(&clean), state_bits(&t.state));
    assert!(std::panic::catch_unwind(|| difference(&[0.], &[f64::NAN])).is_err());
}

#[test]
#[ignore = "native Release tick-563 replay, disturbance at tick 301 and whole 4/16/64-step trajectories"]
fn complete_trajectory_classification_and_refinement() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = fixture(name);
        let r = rail(&i, &s);
        r.reuse.set(true);
        let mut finals = vec![];
        for n in [4, 16, 64] {
            let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            let mut control = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            control.substeps = n;
            t.substeps = n;
            t.capture = true;
            let start = energy(&t.state);
            let mut previous: Option<(State, Vec<f64>)> = None;
            let mut handoff: f64 = 0.;
            let mut max_p: f64 = 0.;
            let mut max_v: f64 = 0.;
            let mut unexplained = [0_f64; 3];
            let clock = std::time::Instant::now();
            for tick in 1..=563 {
                r.begin();
                let old = t.state.clone();
                let supports = t.model.supports.clone();
                let envelopes = t.envelopes.clone();
                let samples = t.samples.clone();
                let report = t.step(&r, G).unwrap();
                let other = control.step(&r, G).unwrap();
                assert_eq!(report, other);
                assert_eq!(state_bits(&t.state), state_bits(&control.state));
                assert_eq!(t.hints(), control.hints());
                // Diagnostics must use the actual frozen hints/windows of this attempted step.
                let final_supports = t.model.supports.clone();
                for ((p, old), e) in t.model.supports.iter_mut().zip(&supports).zip(&t.envelopes) {
                    p.station = old.station;
                    p.half_window = e.half;
                }
                for (k, sample) in t.samples.iter().enumerate() {
                    if let Some((p, c)) = &previous {
                        assert_eq!(state_bits(p), state_bits(&sample.before));
                        handoff = handoff.max(difference(c, &sample.c));
                    }
                    let end = t
                        .model
                        .evaluate(&r, &sample.after.poses, Kind::B1)
                        .unwrap()
                        .c;
                    previous = Some((sample.after.clone(), end));
                    if (n == 4 || k == n - 1) && [1, 24, 300, 480, 562, 563].contains(&tick) {
                        let label = format!("{name}/{n}/{tick}");
                        let d = classify(&t.model, &r, sample, DT / n as f64, true, &label, k);
                        for (a, b) in unexplained.iter_mut().zip(d) {
                            *a = a.max(b);
                        }
                        if tick == 563 {
                            convergence::local_system(
                                &t.model,
                                &r,
                                sample,
                                DT / n as f64,
                                &label,
                                k,
                            );
                        }
                    }
                }
                max_p = max_p.max(report.max_position);
                max_v = max_v.max(report.max_velocity);
                if tick == 300 && n == 4 && name == "helix" {
                    // Exact Claude negative: perturb body 8 in a separate snapshot before tick 301.
                    let mut disturbed = t.state.clone();
                    disturbed.poses[8].position_m = disturbed.poses[8].position_m + Vec3::Y * 1e-5;
                    let clean = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap().c;
                    let bad = t.model.evaluate(&r, &disturbed.poses, Kind::B1).unwrap().c;
                    let jump = difference(&clean, &bad);
                    assert!(jump > 9.9e-6);
                    println!(
                        "M4I_DISTURBANCE {{\"tick\":301,\"displacement\":0.00001,\"gap_jump\":{jump}}}"
                    );
                    let mut negative =
                        measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
                    negative.state = disturbed;
                    negative.model.supports = final_supports.clone();
                    negative.ticks = 300;
                    negative.capture = true;
                    let before_jump = state_bits(&t.state);
                    let mut peak = [0_f64; 3];
                    for tick in 301..=330 {
                        r.begin();
                        let old = negative.model.supports.clone();
                        assert!(acceptable(&negative.step(&r, G).unwrap()));
                        let saved = negative.model.supports.clone();
                        for ((p, o), e) in negative
                            .model
                            .supports
                            .iter_mut()
                            .zip(old)
                            .zip(&negative.envelopes)
                        {
                            p.station = o.station;
                            p.half_window = e.half;
                        }
                        if tick == 301 {
                            assert_ne!(state_bits(&negative.samples[0].before), before_jump);
                            assert!(difference(&clean, &negative.samples[0].c) > 9.9e-6);
                        }
                        if [301, 302, 330].contains(&tick) {
                            let d = classify(
                                &negative.model,
                                &r,
                                &negative.samples[0],
                                DT / 4.,
                                true,
                                &format!("disturbed/4/{tick}"),
                                0,
                            );
                            for (a, b) in peak.iter_mut().zip(d) {
                                *a = a.max(b);
                            }
                        }
                        negative.model.supports = saved;
                    }
                    println!("M4I_DISTURBED_PREFIX {{\"accepted\":330,\"unexplained\":{peak:?}}}");
                }
                if tick == 563 {
                    regularity(&t.model, &r, &t.state, true);
                    println!(
                        "M4I_FINAL {{\"fixture\":\"{name}\",\"n\":{n},\"candidate_tick\":563,\"accepted\":{},\"max_p\":{max_p},\"max_v\":{max_v},\"handoff\":{handoff},\"unexplained\":{unexplained:?},\"drift\":{},\"poses\":{:?},\"velocity\":{:?},\"elapsed_ms\":{}}}",
                        acceptable(&report),
                        energy(&t.state) - start,
                        t.state
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
                            .collect::<Vec<_>>(),
                        t.state.velocity,
                        clock.elapsed().as_secs_f64() * 1000.
                    );
                    finals.push(t.state.clone());
                }
                t.model.supports = final_supports;
                if !acceptable(&report) {
                    assert_eq!((name, n, tick), ("helix", 4, 563));
                    assert!((report.max_velocity - 0.003003486171).abs() < 1e-12);
                    assert!((report.max_position - 0.000016489395727).abs() < 1e-12);
                    // The rejected candidate is evidence only. Restore the entire transaction.
                    t.state = old.clone();
                    t.model.supports = supports;
                    t.envelopes = envelopes;
                    t.samples = samples;
                    t.ticks = tick - 1;
                    assert_eq!(state_bits(&t.state), state_bits(&old));
                    assert_eq!(t.ticks, 562);
                    break;
                }
            }
            assert!(handoff < 1e-12);
            assert_eq!(t.ticks, if name == "helix" && n == 4 { 562 } else { 563 });
        }
        let dist = |a: &State, b: &State| {
            a.poses
                .iter()
                .zip(&b.poses)
                .map(|(a, b)| (a.position_m - b.position_m).length())
                .fold(0., f64::max)
        };
        let coarse = dist(&finals[0], &finals[1]);
        let fine = dist(&finals[1], &finals[2]);
        assert!(fine < coarse);
        // The independent R8 report predicts first order. Foundation's leading
        // error can cancel; measure its order without imposing the R8 result.
        if name == "helix" {
            assert!(coarse / fine > 3.5 && coarse / fine < 4.5);
        }
        println!(
            "M4I_REFINEMENT {{\"fixture\":\"{name}\",\"coarse_difference\":{coarse},\"fine_difference\":{fine},\"ratio\":{}}}",
            coarse / fine
        );
    }
}
