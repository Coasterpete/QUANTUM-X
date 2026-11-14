//! Exercise public production APIs and the inherited B1 engine against closed forms.
use super::*;
use analytic::{BANK, Case, R, S0, THETA};
use qx_geometry::{
    authoring::SectionKind,
    math::{Frame, Quaternion},
    solver::{GeneratedTrack, generate},
};
use qx_physics::{Body, MassProperties, Pose, TrackAttitude, TrackGuide, World};

pub fn authored(case: Case) -> (InitialConditions, Vec<Section>) {
    let mut i = InitialConditions::default();
    let mut s = Section::straight(
        1,
        "M4I analytical reference",
        if case == Case::Incline { 120. } else { 110. },
    );
    match case {
        Case::Incline => i.orientation = Quaternion::axis_angle(Vec3::Z, -THETA).unwrap(),
        Case::Circle => {
            s.kind = SectionKind::Arc {
                radius_m: R,
                plane_angle_rad: std::f64::consts::FRAC_PI_2,
            }
        }
        Case::Banked => {
            i.orientation = Quaternion::axis_angle(Vec3::X, BANK).unwrap();
            s.kind = SectionKind::Arc {
                radius_m: R,
                plane_angle_rad: std::f64::consts::FRAC_PI_2 - BANK,
            };
        }
    }
    (i, vec![s])
}
pub fn production(
    case: Case,
    track: &GeneratedTrack,
    carriage: bool,
    omega: bool,
    n: u32,
) -> (World, Vec<TrackGuide>, Vec<TrackAttitude>) {
    let sample = track.sample_at_distance(S0).unwrap();
    let half: f64 = if carriage { 0.3 } else { 0. };
    let radius = case.radius(half);
    let ds = if case == Case::Incline {
        half
    } else {
        R * (half / R).asin()
    };
    let guides = if carriage {
        vec![
            TrackGuide {
                body: 0,
                local_point_m: Vec3::X * half,
                station_m: S0 + ds,
            },
            TrackGuide {
                body: 0,
                local_point_m: -Vec3::X * half,
                station_m: S0 - ds,
            },
        ]
    } else {
        vec![TrackGuide {
            body: 0,
            local_point_m: Vec3::ZERO,
            station_m: S0,
        }]
    };
    let attitudes = if carriage {
        vec![TrackAttitude {
            body: 0,
            local_point_m: Vec3::ZERO,
            station_m: S0,
            local_frame: Quaternion::IDENTITY,
        }]
    } else {
        vec![]
    };
    let b = Body {
        mass: MassProperties {
            mass_kg: 100.,
            principal_inertia_kg_m2: Vec3::new(20., 20., 20.),
        },
        pose: Pose {
            position_m: case.position(S0, radius),
            orientation: sample.frame.orientation(),
        },
        linear_velocity_mps: sample.frame.tangent() * case.speed(),
        angular_velocity_radps: if omega {
            case.omega(case.speed(), radius)
        } else {
            Vec3::ZERO
        },
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    };
    (
        World {
            bodies: vec![b],
            joints: vec![],
            angular_joints: vec![],
            guides: vec![],
            gravity_mps2: G,
            iterations: 32,
            substeps: n,
        },
        guides,
        attitudes,
    )
}
fn single(r: &Rail) -> Model {
    let mut m = model(r, S0, false);
    // The original five declared supports on one axle, with no bearings.
    let owner = m.axles[0].0;
    let e = r.source.evaluate(ID, S0, Side::Right).unwrap();
    let pose = Pose {
        position_m: e.position_m,
        orientation: e.frame.orientation(),
    };
    m.supports.retain(|s| s.owner == owner);
    for s in &mut m.supports {
        s.owner = 0;
        s.station = S0 + s.local.x;
    }
    m.poses = vec![pose];
    m.axles = vec![(0, S0)];
    m.bearings.clear();
    m
}
fn emit(case: Case, engine: &str, mode: &str, n: usize, tick: usize, motion: (Pose, Vec3, Vec3)) {
    let (p, v, w) = motion;
    println!(
        "M4I_REFERENCE {{\"case\":\"{}\",\"engine\":\"{engine}\",\"mode\":\"{mode}\",\"n\":{n},\"tick\":{tick},\"x\":{:?},\"v\":{:?},\"w\":{:?}}}",
        case.name(),
        array(p.position_m),
        array(v),
        array(w)
    );
}
fn run(ticks: usize, output: bool) {
    for case in [Case::Incline, Case::Circle, Case::Banked] {
        let (i, s) = authored(case);
        let track = generate(&i, &s, GenerationOptions::default()).unwrap();
        let r = rail(&i, &s);
        r.reuse.set(true);
        for s in [0.0137, 5.0237, 29.9237, 80.0137, 109.9137] {
            let (p, t, n) = case.frame(s);
            let a = track.sample_at_distance(s).unwrap();
            let b = r.source.evaluate(ID, s, Side::Right).unwrap();
            assert!((a.position_m - p).length() < 1e-9 && (b.position_m - p).length() < 1e-9);
            assert!(
                (a.frame.tangent() - t).length() < 1e-9 && (b.frame.tangent() - t).length() < 1e-9
            );
            assert!(
                (a.frame.normal() - n).length() < 1e-9 && (b.frame.normal() - n).length() < 1e-9
            );
        }
        for n in [4, 16] {
            for (carriage, seed) in [(false, false), (true, false), (true, true)] {
                let (mut w, mut guides, mut attitudes) =
                    production(case, &track, carriage, seed, n);
                let mode = if !carriage {
                    "point"
                } else if seed {
                    "carriage-compatible"
                } else {
                    "carriage-zero-omega"
                };
                let mut max_speed: f64 = 0.;
                let mut max_reaction: f64 = 0.;
                for tick in 0..=ticks {
                    let b = &w.bodies[0];
                    let station = case.station(b.pose.position_m);
                    max_speed = max_speed
                        .max((b.linear_velocity_mps.length() - case.speed_at(station)).abs());
                    if output {
                        emit(
                            case,
                            "production",
                            mode,
                            n as usize,
                            tick,
                            (b.pose, b.linear_velocity_mps, b.angular_velocity_radps),
                        );
                    }
                    if tick == ticks {
                        break;
                    }
                    let d = w
                        .step_on_track(&track, &mut guides, &mut attitudes)
                        .unwrap();
                    let f = d
                        .guide_force_n
                        .iter()
                        .chain(&d.attitude_force_n)
                        .fold(Vec3::ZERO, |a, b| a + *b);
                    if tick > 24 {
                        let b = &w.bodies[0];
                        let expected = case.reaction(
                            case.station(b.pose.position_m),
                            case.speed_at(case.station(b.pose.position_m)),
                            case.radius(if carriage { 0.3 } else { 0. }),
                        );
                        max_reaction =
                            max_reaction.max((f - expected).length() / expected.length());
                    }
                }
                // Characterization bounds against the independent continuum, not acceptance-policy replacements.
                assert!(
                    max_speed < 0.008 && max_reaction < 0.002,
                    "{case:?}/{mode}/{n}: {max_speed} {max_reaction}"
                );
                if !carriage || seed {
                    let station = case.station(w.bodies[0].pose.position_m);
                    assert!(
                        (station
                            - case.station_at(
                                ticks as f64 * DT,
                                case.radius(if carriage { 0.3 } else { 0. })
                            ))
                        .abs()
                            < 0.004
                    );
                }
                println!(
                    "M4I_REFERENCE_SUMMARY {{\"case\":\"{}\",\"mode\":\"{mode}\",\"n\":{n},\"speed_error\":{max_speed},\"reaction_relative\":{max_reaction}}}",
                    case.name()
                );
            }
            let mut t = measured_tracking::Tracked::new(single(&r), &r, case.speed()).unwrap();
            t.substeps = n as usize;
            let mut rejected = 0;
            let mut max_speed: f64 = 0.;
            for tick in 0..=ticks {
                let v = &t.state.velocity;
                let p = t.state.poses[0];
                let linear = Vec3::new(v[0], v[1], v[2]);
                max_speed = max_speed
                    .max((linear.length() - case.speed_at(case.station(p.position_m))).abs());
                if output {
                    emit(
                        case,
                        "b1",
                        "single",
                        n as usize,
                        tick,
                        (p, linear, Vec3::new(v[3], v[4], v[5])),
                    );
                }
                if tick == ticks {
                    break;
                }
                r.begin();
                let d = t.step(&r, G).unwrap();
                // These are ungated reference observations, never accepted playback states.
                if d.max_position >= 2e-5 || d.max_velocity >= 0.003 {
                    rejected += 1;
                }
            }
            assert!(max_speed < 0.001);
            if n == 16 {
                assert_eq!(rejected, 0);
            } else if case != Case::Incline {
                assert!(rejected > 0);
            }
            println!(
                "M4I_B1_REFERENCE {{\"case\":\"{}\",\"n\":{n},\"ticks\":{ticks},\"outside_original_quality_gate\":{rejected},\"speed_error\":{max_speed}}}",
                case.name()
            );
        }
    }
}
#[test]
fn analytic_geometry_and_short_public_engine_controls() {
    run(48, false);
}
#[test]
#[ignore = "native Release six-second analytical references with raw-state export"]
fn six_second_independent_references() {
    run(1440, true);
}

#[test]
fn production_start_and_velocity_only_projection_counterfactual() {
    use qx_physics::assembly_diagnostics::velocity_residuals;
    for case in [Case::Incline, Case::Circle, Case::Banked] {
        let (i, s) = authored(case);
        let track = generate(&i, &s, GenerationOptions::default()).unwrap();
        let (mut w, g, a) = production(case, &track, true, false, 4);
        let original = w.clone();
        let before = velocity_residuals(&w, &track, &g, &a).unwrap();
        // Five rows of the existing point-guide + rail-roll constraints at this
        // constant-bank seated pose. This experiment has no force kick, position
        // correction or advance; only velocity variables are changed.
        let mut j = vec![];
        for guide in &g {
            let b = &w.bodies[guide.body];
            let arm = Frame::new(b.pose.orientation)
                .unwrap()
                .local_to_world(guide.local_point_m);
            let f = track.sample_at_distance(guide.station_m).unwrap().frame;
            for normal in [f.normal(), f.lateral()] {
                let torque = arm.cross(normal);
                j.push(vec![
                    normal.x, normal.y, normal.z, torque.x, torque.y, torque.z,
                ]);
            }
        }
        let tangent = case.frame(S0).1;
        j.push(vec![0., 0., 0., tangent.x, tangent.y, tangent.z]);
        let b = &w.bodies[0];
        let v = vec![
            b.linear_velocity_mps.x,
            b.linear_velocity_mps.y,
            b.linear_velocity_mps.z,
            0.,
            0.,
            0.,
        ];
        let (dv, _) = contract::correction(
            &j,
            &[0.01, 0.01, 0.01, 0.05, 0.05, 0.05],
            &matrix::multiply(&j, &v)
                .iter()
                .map(|x| -x)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let v: Vec<_> = v.iter().zip(dv).map(|(v, d)| v + d).collect();
        w.bodies[0].linear_velocity_mps = Vec3::new(v[0], v[1], v[2]);
        w.bodies[0].angular_velocity_radps = Vec3::new(v[3], v[4], v[5]);
        let after = velocity_residuals(&w, &track, &g, &a).unwrap();
        assert_eq!(w.bodies[0].pose, original.bodies[0].pose);
        assert!(
            after.max_guide_transverse_speed_mps < 1e-8 && after.max_rail_twist_speed_radps < 1e-8
        );
        let rho = case.radius(0.3);
        let expected = if case == Case::Incline {
            case.speed()
        } else {
            case.speed() * 100. * rho * rho / (100. * rho * rho + 20.)
        };
        assert!((w.bodies[0].linear_velocity_mps.length() - expected).abs() < 1e-8);
        assert!((w.bodies[0].angular_velocity_radps - case.omega(expected, rho)).length() < 1e-8);
        println!(
            "M4I_INITIALIZATION {{\"case\":\"{}\",\"before_guide\":{},\"after_guide\":{},\"projected_speed\":{expected}}}",
            case.name(),
            before.max_guide_transverse_speed_mps,
            after.max_guide_transverse_speed_mps
        );
        if case == Case::Incline {
            assert!(before.max_guide_transverse_speed_mps < 1e-9);
        } else {
            assert!(before.max_guide_transverse_speed_mps > 0.2);
        }
        // Observe actual public playback, including its multi-body initialization.
        let train = qx_document::train::Train::development_consist(
            "M4I reference".into(),
            1,
            qx_document::train::DevelopmentArrangement::Conventional,
        )
        .unwrap();
        let mut sim =
            qx_physics::TrackSimulation::new(train, std::sync::Arc::new(track), 15., case.speed())
                .unwrap();
        let initial = sim.world().clone();
        sim.step().unwrap();
        let first = sim.world().clone();
        sim.reset();
        assert_eq!(sim.world(), &initial);
        sim.step().unwrap();
        assert_eq!(sim.world(), &first);
    }
}
