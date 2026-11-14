//! Scratch analytical-reference harness. Emits raw engine states only (JSON
//! lines). All analytic references are computed separately in analyze.py, so
//! no reference value is derived from engine code.
#![allow(dead_code, unused_imports, clippy::all)]

use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    heartline::AuthoredReference,
    math::{Frame, Quaternion, Vec3},
    solver::{
        GeneratedTrack, GenerationOptions, generate,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
use qx_physics::{
    Body, FIXED_STEP_SECONDS as DT, MassProperties, Pose, TrackAttitude, TrackGuide,
    TrackSimulation, World,
};
use std::{fmt::Write as _, fs::File, io::Write, sync::Arc, time::Instant};

/// Read-only inclusion of the unchanged B1 research modules (same module tree
/// as the repository's test binaries).
mod b1 {
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-geometry/tests/support/rail_fixtures.rs"]
    pub mod rail_fixtures;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_matrix.rs"]
    pub mod matrix;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_guidance.rs"]
    pub mod guidance;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4a_mechanisms.rs"]
    pub mod mechanisms;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4f_surface.rs"]
    pub mod surface;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4b_contract.rs"]
    pub mod contract;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4b_dynamics.rs"]
    pub mod original_dynamics;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4d_dynamics.rs"]
    pub mod refined_dynamics;
    #[path = "C:/Users/coast/.codex/worktrees/train-articulation-m4h/QUANTUM-X/crates/qx-physics/tests/support/m4d_tracking.rs"]
    pub mod tracking;

    use qx_geometry::{math::Vec3, transition::Side};
    use qx_physics::Pose;

    /// One rigid axle carrier with M4B/M4H's five declared supports, built
    /// exactly as `Model::new` does, but without bogie/car bearings.
    pub fn single(rail: &surface::Rail, s0: f64) -> contract::Model {
        let e = rail.source.evaluate(rail.identity, s0, Side::Right).unwrap();
        let pose = Pose {
            position_m: e.position_m,
            orientation: e.frame.orientation(),
        };
        let mut supports = vec![];
        for (x, z, role) in [
            (-0.12, -0.6, surface::Role::Running),
            (0.12, -0.6, surface::Role::Running),
            (0., 0.6, surface::Role::Running),
            (-0.12, 0.6, surface::Role::Side),
            (0.12, 0.6, surface::Role::Side),
        ] {
            let y = if role == surface::Role::Side { -0.08 } else { 0. };
            supports.push(contract::Support {
                owner: 0,
                local: Vec3::new(x, y, z),
                station: s0 + x,
                half_window: 0.8,
                surface: surface::Surface {
                    rail: if z < 0. { "left" } else { "right" },
                    face: if role == surface::Role::Running {
                        "running-plane"
                    } else {
                        "side-plane"
                    },
                    offset: Vec3::new(0., y, z),
                    role,
                },
            });
        }
        contract::Model {
            poses: vec![pose],
            axles: vec![(0, s0)],
            bearings: vec![],
            supports,
        }
    }

    /// Same seeding as source_guidance_m4h.rs `model()`.
    pub fn articulated(rail: &surface::Rail, lead: f64, shared: bool) -> contract::Model {
        let mut m = mechanisms::Mechanism::new(
            mechanisms::Path::Straight,
            lead,
            false,
            Vec3::new(0.05, 0.12, 0.03),
            shared,
        );
        for p in &mut m.poses {
            let local = Vec3::new(0., p.position_m.y, p.position_m.z);
            let e = rail
                .source
                .evaluate(rail.identity, p.position_m.x, Side::Right)
                .unwrap();
            p.position_m = e.position_m + e.frame.local_to_world(local);
            p.orientation = e.frame.orientation();
        }
        contract::Model::new(m)
    }
}

const G: f64 = 9.81;
const M: f64 = 100.;
const I: f64 = 20.;
const ID: SourceIdentity = SourceIdentity {
    document: 0x5ef,
    generation: 1,
};

fn gvec() -> Vec3 {
    Vec3::new(0., -G, 0.)
}

struct Case {
    name: &'static str,
    initial: InitialConditions,
    sections: Vec<Section>,
    s0: f64,
    v0: f64,
    ticks: usize,
}

fn cases() -> Vec<Case> {
    let theta = 0.1;
    let radius = 20.;
    let bank = 0.6;
    let mut circle = Section::straight(1, "horizontal circle", 110.);
    circle.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    let mut banked = Section::straight(1, "banked horizontal circle", 110.);
    // Rider-frame curvature direction cos(a) n + sin(a) l is horizontal for a = pi/2 - bank.
    banked.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: std::f64::consts::FRAC_PI_2 - bank,
    };
    vec![
        Case {
            name: "incline",
            initial: InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::Z, -theta).unwrap(),
                ..Default::default()
            },
            sections: vec![Section::straight(1, "incline", 120.)],
            s0: 5.,
            v0: 5.,
            ticks: 1440,
        },
        Case {
            name: "circle",
            initial: InitialConditions::default(),
            sections: vec![circle],
            s0: 5.,
            v0: 15.,
            ticks: 1440,
        },
        Case {
            name: "banked",
            initial: InitialConditions {
                orientation: Quaternion::axis_angle(Vec3::X, bank).unwrap(),
                ..Default::default()
            },
            sections: vec![banked],
            s0: 5.,
            v0: 15.,
            ticks: 1440,
        },
    ]
}

fn a3(v: Vec3) -> String {
    format!("[{:e},{:e},{:e}]", v.x, v.y, v.z)
}
fn q4(q: Quaternion) -> String {
    format!("[{:e},{:e},{:e},{:e}]", q.w, q.x, q.y, q.z)
}

struct Out(File);
impl Out {
    fn line(&mut self, s: String) {
        writeln!(self.0, "{s}").unwrap();
    }
}

fn geometry(out: &mut Out, case: &Case, track: &GeneratedTrack, source: &ContinuousGraph) {
    let length = track.length_m().min(source.length_m());
    let mut s = 0.0137;
    while s < length - 0.01 {
        let p = track.sample_at_distance(s).unwrap();
        let e = source.evaluate(ID, s, Side::Right).unwrap();
        out.line(format!(
            "{{\"kind\":\"geom\",\"case\":\"{}\",\"s\":{s:e},\"prod_p\":{},\"prod_t\":{},\"prod_n\":{},\"src_p\":{},\"src_t\":{},\"src_n\":{}}}",
            case.name,
            a3(p.position_m),
            a3(p.frame.tangent()),
            a3(p.frame.normal()),
            a3(e.position_m),
            a3(e.frame.tangent()),
            a3(e.frame.normal())
        ));
        s += 0.05;
    }
    out.line(format!(
        "{{\"kind\":\"track\",\"case\":\"{}\",\"prod_samples\":{},\"prod_length\":{:e},\"src_length\":{:e}}}",
        case.name,
        track.samples.len(),
        track.length_m(),
        source.length_m()
    ));
}

fn chord_half(track: &GeneratedTrack, s0: f64, chord: f64) -> f64 {
    let (mut lo, mut hi) = (0.5 * chord * 0.9, 0.5 * chord * 1.1);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let d = (track.sample_at_distance(s0 + mid).unwrap().position_m
            - track.sample_at_distance(s0 - mid).unwrap().position_m)
            .length();
        if d < chord { lo = mid } else { hi = mid }
    }
    0.5 * (lo + hi)
}

fn frame_rate(track: &GeneratedTrack, s: f64) -> Vec3 {
    let h = 1e-3;
    let a = track.sample_at_distance(s - h).unwrap().frame.orientation();
    let b = track.sample_at_distance(s + h).unwrap().frame.orientation();
    b.compose(a.conjugate()).rotation_vector().unwrap() / (2. * h)
}

/// mode: "point" (one guide at COM, no attitude) or "carriage" (two guides at
/// +/-0.3 m plus one rail attitude, like a production running gear).
fn run_prod(
    out: &mut Out,
    case: &Case,
    track: &GeneratedTrack,
    mode: &str,
    substeps: u32,
    seed_omega: bool,
) {
    let s0 = case.s0;
    let smp = track.sample_at_distance(s0).unwrap();
    let (com, mut guides, mut attitudes) = if mode == "carriage" {
        let d = chord_half(track, s0, 0.6);
        let a = track.sample_at_distance(s0 + d).unwrap().position_m;
        let b = track.sample_at_distance(s0 - d).unwrap().position_m;
        (
            (a + b) * 0.5,
            vec![
                TrackGuide { body: 0, local_point_m: Vec3::new(0.3, 0., 0.), station_m: s0 + d },
                TrackGuide { body: 0, local_point_m: Vec3::new(-0.3, 0., 0.), station_m: s0 - d },
            ],
            vec![TrackAttitude {
                body: 0,
                station_m: s0,
                local_frame: Quaternion::IDENTITY,
                local_point_m: Vec3::ZERO,
            }],
        )
    } else {
        (
            smp.position_m,
            vec![TrackGuide { body: 0, local_point_m: Vec3::ZERO, station_m: s0 }],
            vec![],
        )
    };
    let omega = if seed_omega { frame_rate(track, s0) * case.v0 } else { Vec3::ZERO };
    let body = Body {
        mass: MassProperties { mass_kg: M, principal_inertia_kg_m2: Vec3::new(I, I, I) },
        pose: Pose { position_m: com, orientation: smp.frame.orientation() },
        // Exactly the production playback seeding: tangent * speed (playback.rs:66-73).
        linear_velocity_mps: smp.frame.tangent() * case.v0,
        angular_velocity_radps: omega,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    let mut w = World {
        bodies: vec![body],
        joints: vec![],
        angular_joints: vec![],
        guides: vec![],
        gravity_mps2: gvec(),
        iterations: 32,
        substeps,
    };
    let variant = format!("{mode}-sub{substeps}{}", if seed_omega { "-omega" } else { "" });
    let emit = |out: &mut Out, w: &World, tick: usize, perr: f64, aerr: f64, f: Vec3, st: f64| {
        let b = &w.bodies[0];
        out.line(format!(
            "{{\"kind\":\"state\",\"engine\":\"prod\",\"case\":\"{}\",\"variant\":\"{variant}\",\"tick\":{tick},\"x\":{},\"v\":{},\"w\":{},\"q\":{},\"perr\":{perr:e},\"aerr\":{aerr:e},\"diag_force\":{},\"station\":{st:e}}}",
            case.name,
            a3(b.pose.position_m),
            a3(b.linear_velocity_mps),
            a3(b.angular_velocity_radps),
            q4(b.pose.orientation),
            a3(f)
        ));
    };
    emit(out, &w, 0, 0., 0., Vec3::ZERO, s0);
    let clock = Instant::now();
    for tick in 1..=case.ticks {
        match w.step_on_track(track, &mut guides, &mut attitudes) {
            Ok(d) => {
                let mut f = Vec3::ZERO;
                for g in &d.guide_force_n {
                    f = f + *g;
                }
                for g in &d.attitude_force_n {
                    f = f + *g;
                }
                let st = guides.iter().map(|g| g.station_m).sum::<f64>() / guides.len() as f64;
                emit(out, &w, tick, d.max_guide_error_m, d.max_attitude_error_rad, f, st);
            }
            Err(e) => {
                out.line(format!(
                    "{{\"kind\":\"fail\",\"engine\":\"prod\",\"case\":\"{}\",\"variant\":\"{variant}\",\"tick\":{tick},\"error\":{:?}}}",
                    case.name, e
                ));
                break;
            }
        }
    }
    out.line(format!(
        "{{\"kind\":\"timing\",\"engine\":\"prod\",\"case\":\"{}\",\"variant\":\"{variant}\",\"seconds\":{:e}}}",
        case.name,
        clock.elapsed().as_secs_f64()
    ));
}

fn run_b1(out: &mut Out, case: &Case, rail: &b1::surface::Rail, substeps: usize) {
    let variant = format!("single-sub{substeps}");
    let mut t = match b1::tracking::Tracked::new(b1::single(rail, case.s0), rail, case.v0) {
        Ok(t) => t,
        Err(e) => {
            out.line(format!(
                "{{\"kind\":\"fail\",\"engine\":\"b1\",\"case\":\"{}\",\"variant\":\"{variant}\",\"tick\":0,\"error\":\"{:?}\"}}",
                case.name, e
            ));
            return;
        }
    };
    t.substeps = substeps;
    let emit = |out: &mut Out, t: &b1::tracking::Tracked, tick: usize, p: f64, v: f64| {
        let pose = t.state.poses[0];
        let vel = &t.state.velocity;
        out.line(format!(
            "{{\"kind\":\"state\",\"engine\":\"b1\",\"case\":\"{}\",\"variant\":\"{variant}\",\"tick\":{tick},\"x\":{},\"v\":{},\"w\":{},\"q\":{},\"perr\":{p:e},\"verr\":{v:e},\"station\":{:e}}}",
            case.name,
            a3(pose.position_m),
            a3(Vec3::new(vel[0], vel[1], vel[2])),
            a3(Vec3::new(vel[3], vel[4], vel[5])),
            q4(pose.orientation),
            t.hints()[0]
        ));
    };
    emit(out, &t, 0, 0., 0.);
    let clock = Instant::now();
    for tick in 1..=case.ticks {
        match t.step(rail, gvec()) {
            Ok(s) => emit(out, &t, tick, s.max_position, s.max_velocity),
            Err(e) => {
                out.line(format!(
                    "{{\"kind\":\"fail\",\"engine\":\"b1\",\"case\":\"{}\",\"variant\":\"{variant}\",\"tick\":{tick},\"error\":\"{:?}\"}}",
                    case.name, e
                ));
                break;
            }
        }
    }
    out.line(format!(
        "{{\"kind\":\"timing\",\"engine\":\"b1\",\"case\":\"{}\",\"variant\":\"{variant}\",\"seconds\":{:e}}}",
        case.name,
        clock.elapsed().as_secs_f64()
    ));
}

fn body_energy(m: f64, inertia: Vec3, pose: Pose, v: Vec3, w: Vec3, g: Vec3) -> f64 {
    let local = Frame::new(pose.orientation).unwrap().world_to_local(w);
    0.5 * m * v.dot(v)
        + 0.5 * (inertia.x * local.x * local.x + inertia.y * local.y * local.y + inertia.z * local.z * local.z)
        - m * g.dot(pose.position_m)
}

/// Articulated runs: energy-only. No single-body analytic reference applies.
fn run_articulated(out: &mut Out, case: &Case, track: &GeneratedTrack, rail: &b1::surface::Rail) {
    let ticks = 960;
    let lead = 15.;
    // Production development consist through the real playback entry point.
    let train = Train::development_consist("harness".into(), 1, DevelopmentArrangement::Conventional).unwrap();
    match TrackSimulation::new(train, Arc::new(track.clone()), lead, case.v0) {
        Ok(mut sim) => {
            let energy = |w: &World| {
                w.bodies
                    .iter()
                    .map(|b| body_energy(b.mass.mass_kg, b.mass.principal_inertia_kg_m2, b.pose, b.linear_velocity_mps, b.angular_velocity_radps, w.gravity_mps2))
                    .sum::<f64>()
            };
            let bodies = sim.world().bodies.len();
            out.line(format!(
                "{{\"kind\":\"artic\",\"engine\":\"prod\",\"case\":\"{}\",\"tick\":0,\"energy\":{:e},\"bodies\":{bodies},\"speed\":{:e}}}",
                case.name, energy(sim.world()), sim.telemetry().speed_mps
            ));
            for tick in 1..=ticks {
                match sim.step() {
                    Ok(d) => out.line(format!(
                        "{{\"kind\":\"artic\",\"engine\":\"prod\",\"case\":\"{}\",\"tick\":{tick},\"energy\":{:e},\"speed\":{:e},\"perr\":{:e},\"jerr\":{:e},\"aerr\":{:e}}}",
                        case.name, energy(sim.world()), sim.telemetry().speed_mps, d.max_guide_error_m, d.max_joint_error_m, d.max_attitude_error_rad
                    )),
                    Err(e) => {
                        out.line(format!("{{\"kind\":\"fail\",\"engine\":\"prod-artic\",\"case\":\"{}\",\"tick\":{tick},\"error\":{:?}}}", case.name, e));
                        break;
                    }
                }
            }
        }
        Err(e) => out.line(format!("{{\"kind\":\"fail\",\"engine\":\"prod-artic\",\"case\":\"{}\",\"tick\":0,\"error\":{:?}}}", case.name, e)),
    }
    // B1 one car / two bogies / four axle carriers, seeded exactly as M4H.
    match b1::tracking::Tracked::new(b1::articulated(rail, lead, false), rail, case.v0) {
        Ok(mut t) => {
            let energy = |s: &b1::original_dynamics::State| {
                s.poses
                    .iter()
                    .zip(s.velocity.chunks(6))
                    .map(|(p, v)| body_energy(M, Vec3::new(I, I, I), *p, Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]), gvec()))
                    .sum::<f64>()
            };
            out.line(format!(
                "{{\"kind\":\"artic\",\"engine\":\"b1\",\"case\":\"{}\",\"tick\":0,\"energy\":{:e},\"bodies\":{}}}",
                case.name, energy(&t.state), t.state.poses.len()
            ));
            for tick in 1..=ticks {
                match t.step(rail, gvec()) {
                    Ok(s) => out.line(format!(
                        "{{\"kind\":\"artic\",\"engine\":\"b1\",\"case\":\"{}\",\"tick\":{tick},\"energy\":{:e},\"perr\":{:e},\"verr\":{:e}}}",
                        case.name, energy(&t.state), s.max_position, s.max_velocity
                    )),
                    Err(e) => {
                        out.line(format!("{{\"kind\":\"fail\",\"engine\":\"b1-artic\",\"case\":\"{}\",\"tick\":{tick},\"error\":\"{:?}\"}}", case.name, e));
                        break;
                    }
                }
            }
        }
        Err(e) => out.line(format!("{{\"kind\":\"fail\",\"engine\":\"b1-artic\",\"case\":\"{}\",\"tick\":0,\"error\":\"{:?}\"}}", case.name, e)),
    }
}

/// B1 single carrier on the horizontal circle at several speeds: engine
/// velocity residual versus centripetal acceleration kappa*v^2.
fn sweep(out: &mut Out) {
    let options = GenerationOptions::default();
    let case = cases().into_iter().nth(1).unwrap();
    let source = ContinuousGraph::prepare(ID, AuthoredReference::TrackCenterline, &case.initial, &case.sections, options).unwrap();
    let rail = b1::surface::Rail::new(source, ID).unwrap();
    for substeps in [4usize, 16] {
        for v in [3., 6., 9., 12., 15., 18.] {
            let mut t = b1::tracking::Tracked::new(b1::single(&rail, case.s0), &rail, v).unwrap();
            t.substeps = substeps;
            let (mut p, mut vr) = (0f64, 0f64);
            let mut fail = String::new();
            for tick in 1..=240 {
                match t.step(&rail, gvec()) {
                    Ok(s) => {
                        if tick > 24 {
                            p = p.max(s.max_position);
                            vr = vr.max(s.max_velocity);
                        }
                    }
                    Err(e) => {
                        fail = format!("{e:?}@{tick}");
                        break;
                    }
                }
            }
            out.line(format!(
                "{{\"kind\":\"sweep\",\"substeps\":{substeps},\"speed\":{v},\"kv2\":{:e},\"perr\":{p:e},\"verr\":{vr:e},\"fail\":\"{fail}\"}}",
                v * v / 20.
            ));
        }
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("output path");
    let mut out = Out(File::create(path).unwrap());
    if std::env::args().nth(2).as_deref() == Some("sweep") {
        sweep(&mut out);
        return;
    }
    let options = GenerationOptions::default();
    for case in cases() {
        // ONE authored source feeds both engines.
        let track = generate(&case.initial, &case.sections, options).unwrap();
        let source = ContinuousGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            &case.initial,
            &case.sections,
            options,
        )
        .unwrap();
        geometry(&mut out, &case, &track, &source);
        let rail = b1::surface::Rail::new(source, ID).unwrap();
        for substeps in [4, 16] {
            run_prod(&mut out, &case, &track, "point", substeps, false);
            run_prod(&mut out, &case, &track, "carriage", substeps, false);
            run_b1(&mut out, &case, &rail, substeps as usize);
        }
        run_prod(&mut out, &case, &track, "carriage", 4, true);
        run_articulated(&mut out, &case, &track, &rail);
        eprintln!("case {} done", case.name);
    }
}
