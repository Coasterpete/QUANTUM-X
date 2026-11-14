//! Isolated test experiments; never replace production or B1 defaults.
use super::*;
use energy_dynamics::Sample;
use matrix::{dot, max_abs, multiply};
use original_dynamics::State;

fn free(s: &State, h: f64) -> Vec<f64> {
    let mut u = s.velocity.clone();
    for v in u.as_chunks_mut::<6>().0 {
        v[0] += G.x * h;
        v[1] += G.y * h;
        v[2] += G.z * h;
    }
    u
}
fn target(c: &[f64], gamma: &[f64], h: f64) -> Vec<f64> {
    c.iter()
        .zip(gamma)
        .map(|(c, g)| -h * g - 0.2 * c / h)
        .collect()
}
struct Solution {
    v: Vec<f64>,
    lambda: Vec<f64>,
}
fn pgs(
    j: &[Vec<f64>],
    inv: &[f64],
    u: &[f64],
    target: &[f64],
    sweeps: usize,
    seed: Option<&[f64]>,
) -> Solution {
    let rows: Vec<_> = j
        .iter()
        .map(|row| {
            let entries: Vec<_> = row
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, x)| *x != 0.)
                .collect();
            let mass: f64 = entries.iter().map(|&(k, x)| x * x * inv[k]).sum();
            assert!(mass > 1e-12);
            (entries, mass)
        })
        .collect();
    let mut v = u.to_vec();
    let mut lambda = vec![0.; j.len()];
    if let Some(seed) = seed {
        assert_eq!(seed.len(), j.len());
        for (i, (row, _)) in rows.iter().enumerate() {
            lambda[i] = seed[i];
            for &(k, x) in row {
                v[k] += inv[k] * x * seed[i];
            }
        }
    }
    for _ in 0..sweeps {
        for (i, (row, mass)) in rows.iter().enumerate() {
            let speed: f64 = row.iter().map(|&(k, x)| x * v[k]).sum();
            // Keep operation order identical to the original bias composition
            // in the control, by receiving gamma/bias separately in replay below.
            let d = (target[i] - speed) / mass;
            lambda[i] += d;
            for &(k, x) in row {
                v[k] += inv[k] * x * d;
            }
        }
    }
    Solution { v, lambda }
}
fn norm(x: &[f64]) -> f64 {
    dot(x, x).sqrt()
}
fn delta(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(a, b)| a - b).collect()
}
pub fn local_system(m: &Model, r: &Rail, s: &Sample, h: f64, label: &str, sub: usize) {
    let inv = m.inverse_mass();
    let u = free(&s.before, h);
    let btarget = target(&s.c, &s.gamma, h);
    let rhs = delta(&btarget, &multiply(&s.j, &u));
    let a: Vec<Vec<f64>> =
        s.j.iter()
            .map(|row| {
                s.j.iter()
                    .map(|other| {
                        row.iter()
                            .zip(other)
                            .zip(&inv)
                            .map(|((a, b), im)| a * b * im)
                            .sum()
                    })
                    .collect()
            })
            .collect();
    // Independent of PGS: pivoted direct square solve. Python later solves the
    // same emitted local system through NumPy, with rank/condition checks.
    let exact = matrix::solve(&a, &rhs);
    assert!(residuals::difference(&multiply(&a, &exact), &rhs) < 1e-9);
    let mut exact_v = u.clone();
    for (row, l) in s.j.iter().zip(&exact) {
        for ((v, j), im) in exact_v.iter_mut().zip(row).zip(&inv) {
            *v += im * j * l;
        }
    }
    let mut errors = vec![];
    for sweeps in [32, 128, 512, 2048] {
        let clock = std::time::Instant::now();
        let z = pgs(&s.j, &inv, &u, &btarget, sweeps, None);
        let ns = clock.elapsed().as_nanos();
        let row_error = delta(&multiply(&s.j, &z.v), &btarget);
        let impulse_error = norm(&delta(&z.lambda, &exact)) / norm(&exact);
        let bearing = max_abs(&row_error[m.supports.len()..]);
        if sweeps == 32 {
            assert!(residuals::difference(&z.lambda, &s.impulse) < 1e-11);
        }
        if sweeps == 2048 {
            assert!(max_abs(&row_error) < 1e-10 && impulse_error < 1e-7);
        }
        let displacement: Vec<_> = z.v.iter().map(|v| v * h).collect();
        let after = State {
            poses: m.shifted(&s.before.poses, &displacement),
            velocity: z.v.clone(),
        };
        let gate = m.evaluate(r, &after.poses, Kind::B1).unwrap();
        errors.push([
            sweeps as f64,
            max_abs(&row_error),
            bearing,
            impulse_error,
            norm(&delta(&z.v, &exact_v)),
            residuals::energy(&after) - residuals::energy(&s.before),
            max_abs(&multiply(&gate.j, &z.v)),
            ns as f64,
        ]);
        println!(
            "M4I_LOCAL_RESULT {{\"label\":\"{label}\",\"sub\":{sub},\"sweeps\":{sweeps},\"lambda\":{:?},\"v\":{:?}}}",
            z.lambda, z.v
        );
    }
    println!(
        "M4I_LOCAL {{\"label\":\"{label}\",\"sub\":{sub},\"h\":{h},\"j\":{:?},\"inv\":{inv:?},\"u\":{u:?},\"target\":{btarget:?},\"exact\":{exact:?},\"original_impulse\":{:?},\"rail_rows\":{},\"errors\":{errors:?}}}",
        s.j,
        s.impulse,
        m.supports.len()
    );
}

#[test]
fn short_local_sweep_reference_and_stability() {
    let (i, s, lead) = fixture("helix");
    let r = rail(&i, &s);
    r.reuse.set(true);
    let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    t.capture = true;
    let hints = t.hints();
    t.step(&r, G).unwrap();
    for ((p, h), e) in t.model.supports.iter_mut().zip(hints).zip(&t.envelopes) {
        p.station = h;
        p.half_window = e.half;
    }
    local_system(&t.model, &r, &t.samples[0], DT / 4., "helix/4/1", 0);
}

/// Frozen-step PGS experiment. This deliberately repeats the original scalar
/// expression rather than reassociating its target, to permit a bitwise control.
fn experiment_step(
    t: &mut measured_tracking::Tracked,
    r: &Rail,
    sweeps: usize,
    warm: &mut Option<Vec<f64>>,
    use_warm: bool,
) -> (f64, f64, Vec<f64>) {
    let h = DT / t.substeps as f64;
    let inv = t.model.inverse_mass();
    let old = t.model.supports.clone();
    let mut envelopes = vec![];
    for p in &t.model.supports {
        let pose = t.state.poses[p.owner];
        let x = guidance::point(pose, p.local);
        let j = r.jet(p.surface, p.station).unwrap();
        let d = j.d.dot(j.d) - (x - j.p).dot(j.dd);
        assert!(d > 1e-8);
        let v = &t.state.velocity[6 * p.owner..6 * p.owner + 6];
        let u =
            Vec3::new(v[0], v[1], v[2]) + Vec3::new(v[3], v[4], v[5]).cross(x - pose.position_m);
        let rate = j.d.dot(u) / d;
        let innovation = rate.abs() * DT * 1.25 + 0.002;
        let half = innovation + 0.01;
        assert!(half <= 0.20 && (j.dd.length() / j.d.length() + j.k.length()) * half <= 0.15);
        envelopes.push(measured_tracking::Envelope {
            rate,
            innovation,
            half,
        });
    }
    for (p, e) in t.model.supports.iter_mut().zip(&envelopes) {
        p.half_window = e.half;
    }
    let mut max_p: f64 = 0.;
    let mut max_v: f64 = 0.;
    let mut integrated = vec![];
    for _ in 0..t.substeps {
        let e = t.model.evaluate(r, &t.state.poses, Kind::B1).unwrap();
        let gamma = t
            .model
            .curvature(r, &t.state.poses, &t.state.velocity, Kind::B1, 1e-5)
            .unwrap();
        let mut v = free(&t.state, h);
        let mut lambda = vec![0.; e.c.len()];
        if use_warm && let Some(seed) = warm.as_ref() {
            lambda.clone_from(seed);
            for (row, l) in e.j.iter().zip(seed) {
                for ((v, j), im) in v.iter_mut().zip(row).zip(&inv) {
                    *v += im * j * l;
                }
            }
        }
        let rows: Vec<_> =
            e.j.iter()
                .map(|row| {
                    let entries: Vec<_> = row
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(_, x)| *x != 0.)
                        .collect();
                    let mass: f64 = entries.iter().map(|&(k, j)| j * j * inv[k]).sum();
                    (entries, mass)
                })
                .collect();
        for _ in 0..sweeps {
            for (k, (row, mass)) in rows.iter().enumerate() {
                let speed: f64 = row.iter().map(|&(k, j)| j * v[k]).sum();
                let d = -(speed + h * gamma[k] + 0.2 * e.c[k] / h) / mass;
                lambda[k] += d;
                for &(k, j) in row {
                    v[k] += inv[k] * j * d;
                }
            }
        }
        if integrated.is_empty() {
            integrated = vec![0.; lambda.len()];
        }
        for (sum, l) in integrated.iter_mut().zip(&lambda) {
            *sum += l;
        }
        *warm = Some(lambda);
        let displacement: Vec<_> = v.iter().map(|v| v * h).collect();
        t.state.poses = t.model.shifted(&t.state.poses, &displacement);
        t.state.velocity = v;
        let end = t.model.evaluate(r, &t.state.poses, Kind::B1).unwrap();
        max_p = max_p.max(max_abs(&end.c));
        max_v = max_v.max(max_abs(&multiply(&end.j, &t.state.velocity)));
        assert!(max_p < 5e-4 && max_v < 0.1 && max_abs(&t.state.velocity) < 100.);
    }
    for ((p, old), e) in t.model.supports.iter_mut().zip(&old).zip(&envelopes) {
        let station = r
            .project(
                p.surface,
                guidance::point(t.state.poses[p.owner], p.local),
                p.station,
                p.half_window,
            )
            .unwrap()
            .0;
        assert!((station - old.station).abs() <= e.innovation);
        assert!((station - old.station - e.rate * DT).abs() <= 0.25 * e.rate.abs() * DT + 0.002);
        p.station = station;
        p.half_window = old.half_window;
    }
    t.envelopes = envelopes;
    t.ticks += 1;
    (max_p, max_v, integrated)
}

#[test]
#[ignore = "serial Release 563-tick 32/128/512-sweep and 32-warm experiments, all original gates retained"]
fn sweep_and_warm_trajectories() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = fixture(name);
        let r = rail(&i, &s);
        r.reuse.set(true);
        for (sweeps, warm_start) in [(32, false), (128, false), (512, false), (32, true)] {
            let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            let mut control = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            let start = residuals::energy(&t.state);
            let mut warm = None;
            let mut times = vec![];
            let mut accumulated = vec![];
            let mut max_p: f64 = 0.;
            let mut max_v: f64 = 0.;
            let mut max_bearing: f64 = 0.;
            let mut max_reaction: f64 = 0.;
            let mut last_drift = 0.;
            let mut stop = 0;
            for tick in 1..=563 {
                let old = t.state.clone();
                let supports = t.model.supports.clone();
                let env = t.envelopes.clone();
                r.begin();
                let clock = std::time::Instant::now();
                let (p, v, impulse) = experiment_step(&mut t, &r, sweeps, &mut warm, warm_start);
                times.push(clock.elapsed().as_secs_f64() * 1000.);
                if sweeps == 32 && !warm_start {
                    let report = control.step(&r, G).unwrap();
                    assert_eq!(p.to_bits(), report.max_position.to_bits());
                    assert_eq!(v.to_bits(), report.max_velocity.to_bits());
                    assert_eq!(
                        residuals::state_bits(&t.state),
                        residuals::state_bits(&control.state)
                    );
                    assert_eq!(t.hints(), control.hints());
                }
                if p >= 2e-5 || v >= 0.003 {
                    stop = tick;
                    t.state = old;
                    t.model.supports = supports;
                    t.envelopes = env;
                    t.ticks = tick - 1;
                    break;
                }
                max_p = max_p.max(p);
                max_v = max_v.max(v);
                if accumulated.is_empty() {
                    accumulated = vec![0.; impulse.len()];
                }
                for (sum, l) in accumulated.iter_mut().zip(&impulse) {
                    *sum += l;
                }
                let e = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap();
                max_bearing =
                    max_bearing.max(max_abs(&multiply(&e.j, &t.state.velocity)[e.rail_rows..]));
                if [1, 24, 300, 480, 562, 563].contains(&tick) {
                    let (_, forces) =
                        original_dynamics::instantaneous(&t.model, &r, &t.state, G).unwrap();
                    max_reaction = max_reaction.max(max_abs(&forces[..e.rail_rows]));
                    regular_check(&t, &r);
                    println!(
                        "M4I_SWEEP_CHECKPOINT {{\"fixture\":\"{name}\",\"sweeps\":{sweeps},\"warm\":{warm_start},\"tick\":{tick},\"average_impulse\":{:?},\"instantaneous_reactions\":{forces:?}}}",
                        impulse.iter().map(|l| l / DT).collect::<Vec<_>>()
                    );
                }
                last_drift = residuals::energy(&t.state) - start;
            }
            if sweeps == 32 && !warm_start && name == "helix" {
                assert_eq!(stop, 563);
            }
            assert!(last_drift.is_finite());
            println!(
                "M4I_SWEEP {{\"fixture\":\"{name}\",\"sweeps\":{sweeps},\"warm\":{warm_start},\"accepted\":{},\"stop\":{stop},\"drift\":{last_drift},\"max_p\":{max_p},\"max_v\":{max_v},\"bearing_velocity\":{max_bearing},\"reaction_max_sampled\":{max_reaction},\"integrated_impulses\":{accumulated:?},\"times_ms\":{times:?}}}",
                t.ticks
            );
        }
    }
}
fn regular_check(t: &measured_tracking::Tracked, r: &Rail) {
    residuals::regularity(&t.model, r, &t.state, false);
}
