//! Scratch replay of the M4H R8 ("helix") bounded-continuation rejection.
//! Uses the unchanged M4H module tree read-only (same modules as
//! crates/qx-physics/tests/source_guidance_m4h.rs). Emits raw evidence as
//! JSON lines; analysis lives in analyze_r8.py.
#![allow(dead_code, unused_imports, clippy::all)]

const W: &str = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates";

#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4b_contract.rs"]
mod contract;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4h_dynamics.rs"]
mod energy_dynamics;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-geometry/tests/support/m4e_c_extension.rs"]
mod fixtures;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_guidance.rs"]
mod guidance;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_matrix.rs"]
mod matrix;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4h_tracking.rs"]
mod measured_tracking;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_mechanisms.rs"]
mod mechanisms;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4b_dynamics.rs"]
mod original_dynamics;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4d_dynamics.rs"]
mod refined_dynamics;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4h_surface.rs"]
mod surface;
#[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4d_tracking.rs"]
mod tracking;

use contract::{Kind, Model};
use matrix::{max_abs, multiply};
use measured_tracking::Tracked;
use qx_geometry::{
    heartline::AuthoredReference,
    math::Vec3,
    solver::{
        GenerationOptions,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
use qx_physics::FIXED_STEP_SECONDS as DT;
use std::{fs::File, io::Write};
use surface::Rail;

// Identical to source_guidance_m4h.rs.
const ID: SourceIdentity = SourceIdentity {
    document: 0x4fb,
    generation: 1,
};
const G: Vec3 = Vec3::new(0., -9.81, 0.);
const BETA: f64 = 0.2;

fn rail() -> Rail {
    let (_, initial, sections) = fixtures::fixtures().remove(2);
    Rail::new(
        ContinuousGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            &initial,
            &sections,
            GenerationOptions::default(),
        )
        .unwrap(),
        ID,
    )
    .unwrap()
}
// Identical to source_guidance_m4h.rs `model(r, 8.025, true)`.
fn model(r: &Rail) -> Model {
    let mut mechanism = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        8.025,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        true,
    );
    for p in &mut mechanism.poses {
        let local = Vec3::new(0., p.position_m.y, p.position_m.z);
        let e = r.source.evaluate(ID, p.position_m.x, Side::Right).unwrap();
        p.position_m = e.position_m + e.frame.local_to_world(local);
        p.orientation = e.frame.orientation();
    }
    Model::new(mechanism)
}
fn gate(s: &original_dynamics::Step) -> bool {
    !(s.max_position >= 2e-5 || s.max_velocity >= 0.003)
}
fn fresh(r: &Rail, substeps: usize) -> Tracked {
    let mut t = Tracked::new(model(r), r, 6.).unwrap();
    t.substeps = substeps;
    t
}
fn energy(s: &original_dynamics::State) -> f64 {
    let mut e = 0.;
    for (p, v) in s.poses.iter().zip(s.velocity.chunks(6)) {
        e += 50. * (v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
            + 10. * (v[3] * v[3] + v[4] * v[4] + v[5] * v[5])
            - 100. * G.dot(p.position_m);
    }
    e
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(a, b)| a - b).collect()
}
fn flat(s: &original_dynamics::State) -> Vec<f64> {
    let mut out = vec![];
    for p in &s.poses {
        let q = p.orientation;
        out.extend([p.position_m.x, p.position_m.y, p.position_m.z, q.w, q.x, q.y, q.z]);
    }
    out
}
fn fmt(v: &[f64]) -> String {
    let parts: Vec<String> = v.iter().map(|x| format!("{x:e}")).collect();
    format!("[{}]", parts.join(","))
}

/// Re-impose the in-step projection hints/windows the step itself used.
fn with_step_windows<T>(t: &mut Tracked, pre: &[f64], f: impl FnOnce(&Model) -> T) -> T {
    let saved: Vec<_> = t.model.supports.iter().map(|s| (s.station, s.half_window)).collect();
    for ((s, h), e) in t.model.supports.iter_mut().zip(pre).zip(&t.envelopes) {
        s.station = *h;
        s.half_window = e.half;
    }
    let out = f(&t.model);
    for (s, (st, hw)) in t.model.supports.iter_mut().zip(saved) {
        s.station = st;
        s.half_window = hw;
    }
    out
}

struct Out(File);
impl Out {
    fn line(&mut self, s: String) {
        writeln!(self.0, "{s}").unwrap();
    }
}

/// Per substep: c0, gamma0, finite-sweep residual r = J0 v1 - target, and the
/// gate quantity R = J(x1) v1 (J(x1) = next sample's J, or the final in-step evaluation).
fn substep_rows(t: &mut Tracked, r: &Rail, pre: &[f64], h: f64) -> Vec<[Vec<f64>; 5]> {
    let samples = t.samples.clone();
    let after = samples.last().unwrap().after.clone();
    let last = with_step_windows(t, pre, |m| m.evaluate(r, &after.poses, Kind::B1).unwrap());
    let mut out = vec![];
    for (k, s) in samples.iter().enumerate() {
        let target: Vec<f64> = s
            .gamma
            .iter()
            .zip(&s.c)
            .map(|(g, c)| -h * g - BETA * c / h)
            .collect();
        let jv1 = multiply(&s.j, &s.after.velocity);
        let resid = sub(&jv1, &target);
        let (j1, c1) = if k + 1 < samples.len() {
            (&samples[k + 1].j, samples[k + 1].c.clone())
        } else {
            (&last.j, last.c.clone())
        };
        let big_r = multiply(j1, &s.after.velocity);
        out.push([s.c.clone(), s.gamma.clone(), resid, big_r, c1]);
    }
    out
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let mut out = Out(File::create(path).unwrap());
    let r = rail();
    let h4 = DT / 4.;
    if std::env::args().nth(2).as_deref() == Some("perturb") {
        // Negative control: a genuine constraint violation that truncation cannot
        // produce. Displace axle carrier body 8 by 10 micrometres (+Y) after tick 300.
        let mut t = fresh(&r, 4);
        t.capture = true;
        for tick in 1..=330 {
            if tick == 301 {
                t.state.poses[8].position_m = t.state.poses[8].position_m + Vec3::new(0., 1e-5, 0.);
            }
            let pre_hints = t.hints();
            let report = t.step(&r, G).unwrap();
            let rows = substep_rows(&mut t, &r, &pre_hints, h4);
            let subs: Vec<String> = rows
                .iter()
                .map(|[c, g, resid, big_r, c1]| {
                    format!("{{\"c\":{},\"gamma\":{},\"sweep\":{},\"R\":{},\"c1\":{}}}", fmt(c), fmt(g), fmt(resid), fmt(big_r), fmt(c1))
                })
                .collect();
            out.line(format!(
                "{{\"kind\":\"tick\",\"n\":4,\"tick\":{tick},\"p\":{:e},\"v\":{:e},\"accepted\":{},\"subs\":[{}]}}",
                report.max_position,
                report.max_velocity,
                gate(&report),
                subs.join(",")
            ));
        }
        return;
    }

    // Row metadata.
    {
        let t = fresh(&r, 4);
        let mut rows = vec![];
        for (i, s) in t.model.supports.iter().enumerate() {
            rows.push(format!("\"support{i}/body{}/{:?}/{}\"", s.owner, s.surface.role, s.surface.rail));
        }
        for (b, bearing) in t.model.bearings.iter().enumerate() {
            for axis in ["x", "y", "z"] {
                rows.push(format!("\"bearing{b}/{}-{}/anchor-{axis}\"", bearing.a, bearing.b));
            }
            if bearing.roll {
                rows.push(format!("\"bearing{b}/{}-{}/roll\"", bearing.a, bearing.b));
            }
        }
        out.line(format!(
            "{{\"kind\":\"meta\",\"rows\":[{}],\"length\":{:e},\"bodies\":{},\"max_swing\":{}}}",
            rows.join(","),
            r.length(),
            t.state.poses.len(),
            t.model.bearings[0].max_swing
        ));
    }

    // 1. Reproduce: 4 substeps, capture, gate every tick, through candidate 563.
    let mut t = fresh(&r, 4);
    t.capture = true;
    let e0 = energy(&t.state);
    let mut candidate_pre = None;
    for tick in 1..=563 {
        let pre_hints = t.hints();
        let pre_state = t.state.clone();
        let report = t.step(&r, G).unwrap();
        let accepted = gate(&report);
        let rows = substep_rows(&mut t, &r, &pre_hints, h4);
        let mut subs = vec![];
        for [c, g, resid, big_r, c1] in &rows {
            subs.push(format!(
                "{{\"c\":{},\"gamma\":{},\"sweep\":{},\"R\":{},\"c1\":{}}}",
                fmt(c),
                fmt(g),
                fmt(resid),
                fmt(big_r),
                fmt(c1)
            ));
        }
        out.line(format!(
            "{{\"kind\":\"tick\",\"n\":4,\"tick\":{tick},\"p\":{:e},\"v\":{:e},\"accepted\":{accepted},\"energy_drift\":{:e},\"hints\":{},\"subs\":[{}]}}",
            report.max_position,
            report.max_velocity,
            energy(&t.state) - e0,
            fmt(&t.hints()),
            subs.join(",")
        ));
        if !accepted {
            assert_eq!(tick, 563, "unexpected earlier rejection");
            candidate_pre = Some((pre_state, pre_hints));
            break;
        }
    }
    let (pre_state, pre_hints) = candidate_pre.expect("tick 563 must reject");
    let cand = t.state.clone();

    // 2. Candidate diagnostics (in-step windows).
    let samples = t.samples.clone();
    let inv = t.model.inverse_mass();
    for (k, s) in samples.iter().enumerate() {
        let (gamma_v1, converged_r, conv_impulse_gap) = with_step_windows(&mut t, &pre_hints, |m| {
            // Taylor: (J(x1)-J(x0)) v1 ~= h gamma(x0, v1).
            let gamma_v1 = m.curvature(&r, &s.before.poses, &s.after.velocity, Kind::B1, 1e-5).unwrap();
            // Fully converged (dense) solve of the same target, same J0/c0/gamma0.
            let mut u = s.before.velocity.clone();
            for v in u.chunks_mut(6) {
                v[0] += G.x * h4;
                v[1] += G.y * h4;
                v[2] += G.z * h4;
            }
            let ju = multiply(&s.j, &u);
            let rhs: Vec<f64> = s
                .gamma
                .iter()
                .zip(&s.c)
                .zip(&ju)
                .map(|((g, c), ju)| -h4 * g - BETA * c / h4 - ju)
                .collect();
            let (dv, lambda) = contract::correction(&s.j, &inv, &rhs).unwrap();
            let v1: Vec<f64> = u.iter().zip(&dv).map(|(a, b)| a + b).collect();
            let disp: Vec<f64> = v1.iter().map(|x| x * h4).collect();
            let x1 = m.shifted(&s.before.poses, &disp);
            let e1 = m.evaluate(&r, &x1, Kind::B1).unwrap();
            let gap: Vec<f64> = lambda.iter().zip(&s.impulse).map(|(a, b)| a - b).collect();
            (gamma_v1, multiply(&e1.j, &v1), gap)
        });
        out.line(format!(
            "{{\"kind\":\"candidate_sub\",\"sub\":{k},\"gamma_v1\":{},\"R_converged\":{},\"impulse_gap\":{},\"impulse\":{}}}",
            fmt(&gamma_v1),
            fmt(&converged_r),
            fmt(&conv_impulse_gap),
            fmt(&s.impulse)
        ));
    }
    // Projection regularity and source refinement at the candidate (in-step windows).
    let mut sup = vec![];
    for (i, s) in t.model.supports.iter().enumerate() {
        let x = guidance::point(cand.poses[s.owner], s.local);
        let half = t.envelopes[i].half;
        let hint = pre_hints[i];
        let (station, j) = r.project(s.surface, x, hint, half).unwrap();
        let delta = x - j.p;
        let d = j.d.dot(j.d) - delta.dot(j.dd);
        let lo = (hint - half).max(0.);
        let hi = (hint + half).min(r.length());
        let e = r.source.evaluate(ID, station, Side::Right).unwrap();
        sup.push(format!(
            "[{i},{},{station:e},{:e},{d:e},{:e},{:e},{:e},{:e},{}]",
            s.owner,
            delta.dot(j.n),
            (station - lo).min(hi - station),
            r.length() - station,
            e.refinement_position_difference_m,
            e.refinement_orientation_difference_rad,
            e.chart
        ));
    }
    let mut bear = vec![];
    for b in &t.model.bearings {
        let qa = cand.poses[b.a].orientation.compose(b.frame_a);
        let qb = cand.poses[b.b].orientation.compose(b.frame_b);
        let swing = qa.conjugate().compose(qb).rotation_vector().unwrap().length();
        let gap = (guidance::point(cand.poses[b.b], b.local_b) - guidance::point(cand.poses[b.a], b.local_a)).length();
        let twist = if b.roll { guidance::twist(qa, qb) } else { 0. };
        bear.push(format!("[{},{},{swing:e},{gap:e},{twist:e},{}]", b.a, b.b, b.roll));
    }
    out.line(format!(
        "{{\"kind\":\"candidate\",\"supports\":[{}],\"bearings\":[{}],\"energy_drift\":{:e},\"velocity\":{}}}",
        sup.join(","),
        bear.join(","),
        energy(&cand) - e0,
        fmt(&cand.velocity)
    ));

    // 3. Local refinement: replay ONLY tick 563 from the identical accepted state.
    let mut states = vec![(4usize, flat(&cand), cand.velocity.clone())];
    for n in [16usize, 64] {
        let mut u = fresh(&r, 4);
        for _ in 1..=562 {
            assert!(gate(&u.step(&r, G).unwrap()));
        }
        assert_eq!(u.state, pre_state);
        u.substeps = n;
        let rep = u.step(&r, G).unwrap();
        out.line(format!(
            "{{\"kind\":\"local\",\"n\":{n},\"p\":{:e},\"v\":{:e},\"accepted\":{}}}",
            rep.max_position,
            rep.max_velocity,
            gate(&rep)
        ));
        states.push((n, flat(&u.state), u.state.velocity.clone()));
    }
    for (n, x, v) in &states {
        out.line(format!("{{\"kind\":\"local_state\",\"n\":{n},\"x\":{},\"v\":{}}}", fmt(x), fmt(v)));
    }

    // 4. Global refinement: whole trajectory at 4/16/64 substeps to tick 600 (gate recorded, not enforced).
    for n in [4usize, 16, 64] {
        let mut u = fresh(&r, n);
        let e0 = energy(&u.state);
        for tick in 1..=600 {
            match u.step(&r, G) {
                Ok(rep) => {
                    let keep = [480usize, 540, 562, 563, 600].contains(&tick);
                    out.line(format!(
                        "{{\"kind\":\"global\",\"n\":{n},\"tick\":{tick},\"p\":{:e},\"v\":{:e},\"accepted\":{},\"energy_drift\":{:e},\"front_hint\":{:e}{}}}",
                        rep.max_position,
                        rep.max_velocity,
                        gate(&rep),
                        energy(&u.state) - e0,
                        u.hints()[0],
                        if keep {
                            format!(",\"x\":{},\"vel\":{}", fmt(&flat(&u.state)), fmt(&u.state.velocity))
                        } else {
                            String::new()
                        }
                    ));
                }
                Err(e) => {
                    out.line(format!("{{\"kind\":\"global_fail\",\"n\":{n},\"tick\":{tick},\"error\":\"{e:?}\"}}"));
                    break;
                }
            }
        }
        eprintln!("global n={n} done");
    }
}
