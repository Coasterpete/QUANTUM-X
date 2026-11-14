//! Exact discrete ledger, not an energy-conserving integrator or production policy.
use super::*;
use energy_dynamics::Sample;
use qx_physics::FIXED_STEP_SECONDS as DT;

pub(super) const G: Vec3 = Vec3::new(0., -9.81, 0.);
pub(super) fn fixture(name: &str) -> (InitialConditions, Vec<Section>, f64) {
    if name == "helix" {
        let (_, i, s) = fixtures::fixtures().remove(2);
        (i, s, 8.025)
    } else {
        assert_eq!(name, "foundation");
        let p = qx_document::sample::foundation_project();
        (p.initial, p.sections[..2].to_vec(), 11.625)
    }
}

fn energies(s: &original_dynamics::State) -> [f64; 3] {
    let mut e = [0.; 3];
    for (p, v) in s.poses.iter().zip(s.velocity.as_chunks::<6>().0) {
        e[0] += 50. * v[..3].iter().map(|x| x * x).sum::<f64>();
        e[1] += 10. * v[3..].iter().map(|x| x * x).sum::<f64>();
        e[2] -= 100. * G.dot(p.position_m);
    }
    e
}

// Wc = lambda.J.v+ - .5 p.M^-1.p, p=J^T lambda.
// Jv+ = -h gamma - beta c/h + residual after finite PGS sweeps.
// Terms: gravity kick, gravity path, constraint midpoint, curvature target,
// stabilization target, finite-sweep residual, negative impulse quadratic.
fn ledger(s: &Sample, h: f64) -> ([f64; 7], Vec<f64>) {
    let mut p = vec![0.; s.before.velocity.len()];
    for (row, l) in s.j.iter().zip(&s.impulse) {
        for (p, j) in p.iter_mut().zip(row) {
            *p += j * l;
        }
    }
    let inv: Vec<_> = (0..p.len())
        .map(|k| if k % 6 < 3 { 0.01 } else { 0.05 })
        .collect();
    let mut out = [0.; 7];
    let gravity = [G.x, G.y, G.z, 0., 0., 0.];
    for k in 0..p.len() {
        let v = s.before.velocity[k];
        let u = v + gravity[k % 6] * h;
        out[0] += gravity[k % 6] * h * (v + u) * 0.5 / inv[k];
        out[2] += p[k] * (u + s.after.velocity[k]) * 0.5;
        out[6] -= 0.5 * p[k] * inv[k] * p[k];
        assert!((s.after.velocity[k] - u - inv[k] * p[k]).abs() < 2e-12);
    }
    for (a, b) in s.before.poses.iter().zip(&s.after.poses) {
        out[1] += 100. * G.dot(b.position_m - a.position_m);
    }
    let speed = multiply(&s.j, &s.after.velocity);
    for (i, v) in speed.iter().enumerate() {
        out[3] -= s.impulse[i] * h * s.gamma[i];
        out[4] -= s.impulse[i] * 0.2 * s.c[i] / h;
        out[5] += s.impulse[i] * (v + h * s.gamma[i] + 0.2 * s.c[i] / h);
    }
    assert!((out[2] - out[3..].iter().sum::<f64>()).abs() < 2e-9);
    let delta = energies(&s.after).iter().sum::<f64>() - energies(&s.before).iter().sum::<f64>();
    assert!((delta - out[0] + out[1] - out[2]).abs() < 1e-8);
    (out, p)
}

#[test]
fn recorded_step_is_exact_and_failed_capture_is_transactional() {
    let (i, s, lead) = fixture("helix");
    let r = rail(&i, &s);
    let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    let mut c = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    t.capture = true;
    for _ in 0..3 {
        assert_eq!(t.step(&r, G).unwrap(), c.step(&r, G).unwrap());
        assert_eq!(t.state, c.state);
        assert_eq!(t.hints(), c.hints());
        for sample in &t.samples {
            ledger(sample, DT / 4.);
        }
    }
    let before = t.state.clone();
    let hints = t.hints();
    let envelopes = t.envelopes.clone();
    let samples = format!("{:?}", t.samples);
    t.substeps = 3;
    assert_eq!(t.step(&r, G).unwrap_err(), surface::Failure::Invalid);
    assert_eq!(t.state, before);
    assert_eq!(t.hints(), hints);
    assert_eq!(t.envelopes, envelopes);
    assert_eq!(format!("{:?}", t.samples), samples);
    assert_eq!(t.ticks, 3);
}

#[test]
#[ignore = "Release 480-tick energy ledger, unchanged 4/16/64-step controls and raw reconstruction"]
fn energy_refinement_evidence() {
    for name in ["helix", "foundation"] {
        let (i, s, lead) = fixture(name);
        let r = rail(&i, &s);
        for n in [4, 16, 64] {
            let mut t = measured_tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            let mut c = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
            t.substeps = n;
            c.substeps = n;
            t.capture = true;
            let start = energies(&t.state).iter().sum::<f64>();
            let mut sums = [0.; 7];
            let mut max_p = 0_f64;
            let mut max_v = 0_f64;
            let mut continuous = 0.;
            let mut old_power = 0.;
            for tick in 1..=480 {
                let a = t.step(&r, G).unwrap();
                assert_eq!(a, c.step(&r, G).unwrap());
                assert_eq!(t.state, c.state);
                assert_eq!(t.hints(), c.hints());
                assert_eq!(
                    t.envelopes
                        .iter()
                        .map(|e| [e.rate, e.innovation, e.half])
                        .collect::<Vec<_>>(),
                    c.envelopes
                        .iter()
                        .map(|e| [e.rate, e.innovation, e.half])
                        .collect::<Vec<_>>()
                );
                assert!(a.max_position < 2e-5 && a.max_velocity < 0.003);
                max_p = max_p.max(a.max_position);
                max_v = max_v.max(a.max_velocity);
                let e = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap();
                let (_, loads) =
                    original_dynamics::instantaneous(&t.model, &r, &t.state, G).unwrap();
                let power = contract::work(&e.j, &t.state.velocity, &loads);
                continuous += (old_power + power) * 0.5 * DT;
                old_power = power;
                for (sub, sample) in t.samples.iter().enumerate() {
                    let (parts, p) = ledger(sample, DT / n as f64);
                    for (sum, x) in sums.iter_mut().zip(parts) {
                        *sum += x;
                    }
                    let checkpoint = [1, 120, 240, 258, 259, 360, 480].contains(&tick) && sub == 0;
                    if n == 4 || checkpoint {
                        let positions = |s: &original_dynamics::State| {
                            s.poses
                                .iter()
                                .map(|p| array(p.position_m))
                                .collect::<Vec<_>>()
                        };
                        println!(
                            "M4H_SAMPLE {{\"fixture\":\"{name}\",\"n\":{n},\"tick\":{tick},\"sub\":{sub},\"v0\":{:?},\"v1\":{:?},\"p0\":{:?},\"p1\":{:?},\"impulse\":{p:?},\"lambda\":{:?},\"c\":{:?},\"gamma\":{:?},\"speed\":{:?},\"parts\":{parts:?}}}",
                            sample.before.velocity,
                            sample.after.velocity,
                            positions(&sample.before),
                            positions(&sample.after),
                            sample.impulse,
                            sample.c,
                            sample.gamma,
                            multiply(&sample.j, &sample.after.velocity)
                        );
                    }
                    if checkpoint {
                        let sparse: Vec<Vec<_>> = sample
                            .j
                            .iter()
                            .map(|r| {
                                r.iter()
                                    .copied()
                                    .enumerate()
                                    .filter(|(_, v)| *v != 0.)
                                    .map(|(k, v)| [k as f64, v])
                                    .collect()
                            })
                            .collect();
                        println!(
                            "M4H_J {{\"fixture\":\"{name}\",\"n\":{n},\"tick\":{tick},\"rows\":{sparse:?}}}"
                        );
                    }
                }
                let parts = energies(&t.state);
                let drift = parts.iter().sum::<f64>() - start;
                assert!((drift - (sums[0] - sums[1] + sums[2])).abs() < 1e-7);
                println!(
                    "M4H_ENERGY {{\"fixture\":\"{name}\",\"n\":{n},\"tick\":{tick},\"energy\":{parts:?},\"drift\":{drift},\"ledger\":{sums:?},\"continuous_work\":{continuous},\"power\":{power},\"station\":{},\"p\":{max_p},\"v\":{max_v}}}",
                    t.hints()[0]
                );
            }
        }
    }
}
