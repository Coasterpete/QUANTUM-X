//! R0.3 test-only capture of the unchanged production solver. No Debug-format hashes.
use qx_document::{operations::Operations, train::Train};
use qx_geometry::{
    authoring::{InitialConditions, Section},
    math::Vec3,
    solver::{GenerationOptions, generate},
};
use qx_physics::{Diagnostics, FIXED_STEP_SECONDS, TrackSimulation};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub initial: InitialConditions,
    pub sections: Vec<Section>,
    pub train: Train,
    pub operations: Operations,
    pub station_m: f64,
    pub checkpoints: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub tick: u64,
    pub discrete: BTreeMap<String, String>,
    pub scalars: BTreeMap<String, f64>,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Golden {
    pub format_version: u32,
    pub fixed_step_seconds: f64,
    pub input: Input,
    pub states: Vec<State>,
}

fn vector(values: &mut BTreeMap<String, f64>, prefix: &str, v: Vec3) {
    for (axis, x) in [("x", v.x), ("y", v.y), ("z", v.z)] {
        values.insert(format!("{prefix}.{axis}"), x);
    }
}

pub fn state(sim: &TrackSimulation, diagnostics: Option<&Diagnostics>) -> State {
    let mut scalars = BTreeMap::new();
    let mut discrete = BTreeMap::new();
    let snapshot = sim.snapshot().unwrap();
    discrete.insert("iterations".into(), sim.world().iterations.to_string());
    discrete.insert("substeps".into(), sim.world().substeps.to_string());
    vector(&mut scalars, "gravity_mps2", sim.world().gravity_mps2);
    for (i, (b, p)) in sim
        .world()
        .bodies
        .iter()
        .zip(&snapshot.components)
        .enumerate()
    {
        let prefix = format!("body[{i}]");
        discrete.insert(format!("{prefix}.id"), p.id.clone());
        scalars.insert(format!("{prefix}.station_m"), p.station_m);
        scalars.insert(format!("{prefix}.mass_kg"), b.mass.mass_kg);
        vector(
            &mut scalars,
            &format!("{prefix}.position_m"),
            b.pose.position_m,
        );
        let q = b.pose.orientation;
        for (axis, x) in [("w", q.w), ("x", q.x), ("y", q.y), ("z", q.z)] {
            scalars.insert(format!("{prefix}.orientation.{axis}"), x);
        }
        for (name, v) in [
            ("linear_velocity_mps", b.linear_velocity_mps),
            ("angular_velocity_radps", b.angular_velocity_radps),
            ("force_n", b.force_n),
            ("torque_nm", b.torque_nm),
        ] {
            vector(&mut scalars, &format!("{prefix}.{name}"), v);
        }
    }
    for (i, g) in sim.guides().iter().enumerate() {
        discrete.insert(format!("guide[{i}].body"), g.body.to_string());
        scalars.insert(format!("guide[{i}].station_m"), g.station_m);
        vector(
            &mut scalars,
            &format!("guide[{i}].local_point_m"),
            g.local_point_m,
        );
    }
    for j in &snapshot.joints {
        scalars.insert(format!("joint[{}].error_m", j.id), j.error_m);
        vector(
            &mut scalars,
            &format!("joint[{}].anchor_a_m", j.id),
            j.anchor_a_m,
        );
        vector(
            &mut scalars,
            &format!("joint[{}].anchor_b_m", j.id),
            j.anchor_b_m,
        );
    }
    let t = sim.telemetry();
    for (name, value) in [
        ("elapsed_s", t.elapsed_s),
        ("speed_mps", t.speed_mps),
        ("distance_traveled_m", t.distance_traveled_m),
        (
            "longitudinal_acceleration_mps2",
            t.longitudinal_acceleration_mps2,
        ),
        ("applied_device_force_n", t.applied_device_force_n),
        ("resistance_force_n", t.resistance_force_n),
    ] {
        scalars.insert(format!("telemetry.{name}"), value);
    }
    for (name, v) in [
        ("acceleration_mps2", t.acceleration_mps2),
        ("specific_force_mps2", t.specific_force_mps2),
        ("local_specific_force_mps2", t.local_specific_force_mps2),
    ] {
        vector(&mut scalars, &format!("telemetry.{name}"), v);
    }
    for d in &t.devices {
        let prefix = format!("device[{}]", d.id);
        discrete.insert(
            format!("{prefix}.occupied_bodies"),
            d.occupied_bodies.to_string(),
        );
        discrete.insert(
            format!("{prefix}.force_limited"),
            d.force_limited.to_string(),
        );
        for (name, x) in [
            ("commanded_acceleration_mps2", d.commanded_acceleration_mps2),
            ("requested_force_n", d.requested_force_n),
            ("applied_force_n", d.applied_force_n),
        ] {
            scalars.insert(format!("{prefix}.{name}"), x);
        }
    }
    if let Some(d) = diagnostics {
        for (name, x) in [
            ("max_joint_error_m", d.max_joint_error_m),
            ("max_guide_error_m", d.max_guide_error_m),
            ("max_angular_error_rad", d.max_angular_error_rad),
            ("max_attitude_error_rad", d.max_attitude_error_rad),
        ] {
            scalars.insert(format!("diagnostics.{name}"), x);
        }
        for (name, values) in [
            ("joint_force_on_b_n", &d.joint_force_on_b_n),
            ("guide_force_n", &d.guide_force_n),
            ("angular_torque_on_b_nm", &d.angular_torque_on_b_nm),
            ("attitude_torque_nm", &d.attitude_torque_nm),
            ("attitude_force_n", &d.attitude_force_n),
        ] {
            for (i, v) in values.iter().enumerate() {
                vector(&mut scalars, &format!("diagnostics.{name}[{i}]"), *v);
            }
        }
    }
    assert!(
        scalars.values().all(|x| x.is_finite()),
        "nonfinite replay state"
    );
    let mut result = State {
        tick: sim.ticks(),
        discrete,
        scalars,
        fingerprint: String::new(),
    };
    result.fingerprint = fingerprint(&result);
    result
}

/// FNV-1a/64 over tick LE, sorted UTF-8 keys/values with NUL terminators, then
/// sorted scalar keys and IEEE-754 f64 bits LE. A regression digest, not security.
pub fn fingerprint(s: &State) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut bytes = |bytes: &[u8]| {
        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    };
    bytes(&s.tick.to_le_bytes());
    for (k, v) in &s.discrete {
        bytes(k.as_bytes());
        bytes(&[0]);
        bytes(v.as_bytes());
        bytes(&[0]);
    }
    for (k, v) in &s.scalars {
        bytes(k.as_bytes());
        bytes(&[0]);
        bytes(&v.to_bits().to_le_bytes());
    }
    format!("{hash:016x}")
}

pub fn compare_exact(expected: &State, actual: &State) {
    assert_eq!(expected.tick, actual.tick);
    assert_eq!(
        expected.discrete, actual.discrete,
        "tick {} discrete state",
        actual.tick
    );
    assert_eq!(
        expected.scalars.keys().collect::<Vec<_>>(),
        actual.scalars.keys().collect::<Vec<_>>()
    );
    for (key, a) in &expected.scalars {
        let b = actual.scalars[key];
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "tick {} {key}: expected {a:.17e}, actual {b:.17e}",
            actual.tick
        );
    }
    assert_eq!(expected.fingerprint, actual.fingerprint);
}

pub fn run(input: &Input) -> Vec<State> {
    assert_eq!(input.checkpoints.first(), Some(&0));
    assert!(input.checkpoints.windows(2).all(|w| w[0] < w[1]));
    assert!(
        *input.checkpoints.last().unwrap() <= 480,
        "bounded R0.3 replay"
    );
    // Pin the production graph-query resolution; no research path or alternate initializer.
    let options = GenerationOptions {
        max_step_m: 0.1,
        max_samples: 200_000,
        max_rotation_per_step_rad: 0.15,
        curvature_warning_per_m: 0.5,
        spline_arc_length: Default::default(),
    };
    let build = || {
        let track = generate(&input.initial, &input.sections, options).unwrap();
        TrackSimulation::with_operations(
            input.train.clone(),
            Arc::new(track),
            input.station_m,
            input.operations.clone(),
        )
        .unwrap()
    };
    let mut a = build();
    let mut b = build(); // Independent initialization, not only cloned state.
    assert_eq!(FIXED_STEP_SECONDS.to_bits(), (1.0_f64 / 240.0).to_bits());
    assert_eq!((a.world().substeps, a.world().iterations), (4, 32));
    assert!(a.velocity_initialization().is_none());
    let initial = state(&a, None);
    compare_exact(&initial, &state(&b, None));
    let mut states = vec![initial.clone()];
    for tick in 1..=*input.checkpoints.last().unwrap() {
        let da = a.step().unwrap();
        let db = b.step().unwrap();
        let actual = state(&a, Some(&da));
        compare_exact(&actual, &state(&b, Some(&db)));
        if input.checkpoints.contains(&tick) {
            states.push(actual);
        }
    }
    a.reset();
    compare_exact(&initial, &state(&a, None));
    // Reset must replay the same trajectory, not merely restore its first pose.
    let mut checkpoint = 1;
    for tick in 1..=*input.checkpoints.last().unwrap() {
        let d = a.step().unwrap();
        if input.checkpoints.contains(&tick) {
            compare_exact(&states[checkpoint], &state(&a, Some(&d)));
            checkpoint += 1;
        }
    }
    states
}
