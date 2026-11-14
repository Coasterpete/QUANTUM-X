//! Measurement only: never supplies a rail, projection, or physics backend.
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    heartline::AuthoredReference,
    math::{Frame, Quaternion, Vec3},
    solver::{
        GenerationOptions, generate,
        prepared::{Evaluation, PreparedGraph, SourceIdentity},
    },
    transition::{Channel, Function, Side, Transition, Unit},
};

pub const ID: SourceIdentity = SourceIdentity {
    document: 0x04ec,
    generation: 1,
};

pub fn options(step: f64) -> GenerationOptions {
    GenerationOptions {
        max_step_m: step,
        ..Default::default()
    }
}

pub fn prepare(initial: &InitialConditions, sections: &[Section], step: f64) -> PreparedGraph {
    PreparedGraph::prepare(
        ID,
        AuthoredReference::TrackCenterline,
        initial,
        sections,
        options(step),
    )
    .unwrap()
}

fn profile(start: f64, end: f64, function: Function) -> Channel {
    Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition {
            id: 1,
            length: 30.,
            start,
            end,
            amplitude: 0.,
            function,
        }],
    }
}

pub fn fixtures() -> Vec<(&'static str, InitialConditions, Vec<Section>)> {
    let initial = InitialConditions::default();
    let banked = InitialConditions {
        orientation: Quaternion::axis_angle(Vec3::X, 0.6).unwrap(),
        ..initial.clone()
    };
    let mut arc = Section::straight(1, "M1 R12 turn", 12.);
    arc.kind = SectionKind::Arc {
        radius_m: 12.,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    let mut helix = arc.clone();
    helix.kind = SectionKind::Arc {
        radius_m: 8.,
        plane_angle_rad: 0.,
    };
    helix.roll_rate = Channel::constant(Unit::RadiansPerMetre, 12., 0.04);
    let mut rolling = Section::straight(1, "Retained heartline rolling-line fixture", 12.);
    rolling.roll_rate = Channel::constant(Unit::RadiansPerMetre, 12., 0.4);
    let mut rider = Section::straight(1, "M4E-B varying rider controls", 30.);
    rider.kind = SectionKind::RiderLocal {
        pitch_rate: profile(-0.008, 0.02, Function::QuinticSmoothstep {}),
        yaw_rate: profile(0.006, -0.01, Function::Cosine {}),
    };
    rider.roll_rate = profile(0.02, 0.06, Function::Quadratic {});
    let mut force = rider.clone();
    force.kind = SectionKind::Force {
        normal_g: Channel {
            unit: Unit::G,
            ..profile(1., 1.7, Function::QuinticSmoothstep {})
        },
        lateral_g: Channel {
            unit: Unit::G,
            ..profile(0., 0.35, Function::QuinticSmoothstep {})
        },
    };
    vec![
        (
            "straight",
            banked.clone(),
            vec![Section::straight(1, "M1 line", 12.)],
        ),
        ("arc", banked.clone(), vec![arc]),
        ("helix", banked.clone(), vec![helix]),
        ("rolling", initial, vec![rolling]),
        ("rider", banked.clone(), vec![rider]),
        ("force", banked, vec![force]),
    ]
}

pub fn pose(e: &Evaluation) -> [Vec3; 4] {
    [
        e.position_m,
        e.frame.tangent(),
        e.frame.normal(),
        e.frame.lateral(),
    ]
}

pub fn frame_derivatives(frame: Frame, k: [Vec3; 3]) -> [[Vec3; 3]; 3] {
    let [w, dw, ddw] = k;
    [frame.tangent(), frame.normal(), frame.lateral()].map(|a| {
        [
            w.cross(a),
            dw.cross(a) + w.cross(w.cross(a)),
            ddw.cross(a)
                + dw.cross(w.cross(a)) * 2.
                + w.cross(dw.cross(a))
                + w.cross(w.cross(w.cross(a))),
        ]
    })
}

fn flatten(v: impl IntoIterator<Item = Vec3>) -> Vec<f64> {
    v.into_iter().flat_map(|a| [a.x, a.y, a.z]).collect()
}

// Coordinates are part of the evidence; Python uses the actual rounded station
// differences. Each one-sided stencil stays in a single retained fine RK4 cell.
fn stencil(
    source: &PreparedGraph,
    name: &str,
    step: f64,
    label: &str,
    s: f64,
    width: f64,
    mode: i32,
) {
    let side = if s == source.length_m() || mode < 0 {
        Side::Left
    } else {
        Side::Right
    };
    let e = source.evaluate(ID, s, side).unwrap();
    let rows: Vec<_> = (0..=16)
        .map(|i| {
            let t = match mode {
                -1 => (i as f64 - 16.) / 16.,
                1 => i as f64 / 16.,
                _ => (i as f64 - 8.) / 8.,
            };
            let x = s + width * t;
            let side = if x == source.length_m() {
                Side::Left
            } else {
                Side::Right
            };
            let a = source.evaluate(ID, x, side).unwrap();
            assert!(a.local_rk4_steps <= 2);
            let mut row = vec![x - s];
            row.extend(flatten(pose(&a)));
            row
        })
        .collect();
    let axes = frame_derivatives(e.frame, e.angular_derivatives);
    let derivatives: Vec<_> = (0..3)
        .map(|j| {
            flatten([
                e.position_derivatives[j],
                axes[0][j],
                axes[1][j],
                axes[2][j],
            ])
        })
        .collect();
    println!("M4EC_STENCIL {name} {step} {label} {s} {width} {mode} {rows:?}");
    println!(
        "M4EC_SOURCE {:?}",
        [
            flatten(pose(&e)),
            derivatives.concat(),
            flatten(e.angular_derivatives)
        ]
    );
}

pub fn measure(name: &str, initial: &InitialConditions, sections: &[Section], steps: &[f64]) {
    let snapshot = (initial.clone(), sections.to_vec());
    for &step in steps {
        let source = prepare(initial, sections, step);
        source.validate_interval(ID, 0., source.length_m()).unwrap();
        let fine = generate(initial, sections, options(step * 0.5)).unwrap();
        println!(
            "M4EC_CASE {name} {step} {} {} {:?}",
            source.length_m(),
            source.checkpoint_count(),
            source.boundaries()
        );
        for (chart, range) in source.charts().iter().zip(&fine.smooth_ranges) {
            let count = range.end_sample - range.start_sample;
            let h = (chart.end_station_m - chart.start_station_m) / count as f64;
            println!(
                "M4EC_CHART {name} {step} {} {} {count}",
                chart.start_station_m, chart.end_station_m
            );
            // The prepared evaluator maps exact source input subdivisions, rather
            // than accumulated generated distances. Recreate that public mapping.
            let node = |i| {
                chart.start_station_m
                    + (chart.end_station_m - chart.start_station_m) * (i as f64 / count as f64)
            };
            for i in [1, count / 2, count - 1] {
                let s = node(i);
                for (mode, label) in [
                    (-1, "checkpoint-left"),
                    (1, "checkpoint-right"),
                    (0, "checkpoint-cross"),
                ] {
                    for width in [0.8 * h, 0.4 * h] {
                        stencil(&source, name, step, label, s, width, mode);
                    }
                }
            }
            let ordinary = node(count / 3) + 0.371 * h;
            stencil(&source, name, step, "ordinary", ordinary, 0.3 * h, 0);
            // Both an exact sided source limit and a nearby interior station.
            stencil(
                &source,
                name,
                step,
                "chart-start-right",
                chart.start_station_m,
                0.8 * h,
                1,
            );
            stencil(
                &source,
                name,
                step,
                "chart-end-left",
                chart.end_station_m,
                0.8 * h,
                -1,
            );
            stencil(
                &source,
                name,
                step,
                "near-chart-start",
                chart.start_station_m + 0.1 * h,
                0.08 * h,
                0,
            );
            stencil(
                &source,
                name,
                step,
                "near-chart-end",
                chart.end_station_m - 0.1 * h,
                0.08 * h,
                0,
            );
        }
        for s in [0., 0.0137, source.length_m() * 0.4137, source.length_m()] {
            let side = if s == source.length_m() {
                Side::Left
            } else {
                Side::Right
            };
            let e = source.evaluate(ID, s, side).unwrap();
            println!(
                "M4EC_REFINEMENT {name} {step} {s} {} {} {:?}",
                e.refinement_position_difference_m,
                e.refinement_orientation_difference_rad,
                e.refinement_angular_differences
            );
        }
        assert!(source.matches_source(initial, sections, options(step)));
    }
    assert_eq!(snapshot, (initial.clone(), sections.to_vec()));
}
