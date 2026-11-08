//! Independent construction, not an invocation of assembly, chord or station
//! projection. Bracketed scalar roots construct a witness for the ORIGINAL dimensions.
use qx_document::train::{Train, mechanism::JointModel};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3, rotation_between},
    solver::GeneratedTrack,
};
use qx_physics::{
    Pose,
    consist::{ComponentPose, ConsistInspection, JointResidual},
};
use std::collections::HashMap;

pub fn point(p: Pose, v: Vec3) -> Vec3 {
    p.position_m + Frame::new(p.orientation).unwrap().local_to_world(v)
}

fn root(mut lo: f64, mut hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let mut a = f(lo);
    let mut b = f(hi);
    let negative = a < 0.;
    assert_ne!(
        negative,
        b < 0.,
        "independent fixture root is not bracketed"
    );
    for iteration in 0..64 {
        // Bracketed scalar interpolation with periodic bisection. All roots
        // here are simple, and every witness is checked by world geometry.
        let secant = lo + (hi - lo) * (-a / (b - a));
        let mid = if iteration % 4 == 3 || secant <= lo || secant >= hi {
            (lo + hi) * 0.5
        } else {
            secant
        };
        let value = f(mid);
        if value.abs() < 1e-13 {
            return mid;
        }
        if (value < 0.) == negative {
            lo = mid;
            a = value;
        } else {
            hi = mid;
            b = value;
        }
    }
    panic!("independent fixture root did not converge")
}

fn station(track: &GeneratedTrack, p: Vec3, mut s: f64) -> f64 {
    for _ in 0..32 {
        let a = track.sample_at_distance(s).unwrap();
        let delta = (p - a.position_m).dot(a.frame.tangent());
        s += delta;
        if delta.abs() < 1e-13 {
            break;
        }
    }
    s
}

// A rigid .4 m gear chord. Roll about its X axis preserves both guide points;
// zeroing the independent swing/twist formula satisfies the rail attitude.
fn gear(track: &GeneratedTrack, center: f64) -> Pose {
    let span = root(0.39, 0.41, |h| {
        (track
            .sample_at_distance(center + h * 0.5)
            .unwrap()
            .position_m
            - track
                .sample_at_distance(center - h * 0.5)
                .unwrap()
                .position_m)
            .length()
            - 0.4
    });
    let a = track
        .sample_at_distance(center - span * 0.5)
        .unwrap()
        .position_m;
    let b = track
        .sample_at_distance(center + span * 0.5)
        .unwrap()
        .position_m;
    let position_m = (a + b) * 0.5;
    let rail = track
        .sample_at_distance(station(track, position_m, center))
        .unwrap()
        .frame;
    let mut q = rotation_between(rail.tangent(), b - a)
        .unwrap()
        .compose(rail.orientation());
    let relative = rail.orientation().conjugate().compose(q);
    let twist = 2. * relative.x.atan2(relative.w);
    q = q
        .compose(Quaternion::axis_angle(Vec3::X, -twist).unwrap())
        .normalized()
        .unwrap();
    Pose {
        position_m,
        orientation: q,
    }
}

fn car(track: &GeneratedTrack, front: f64) -> [Pose; 3] {
    let a = gear(track, front);
    let rear = root(front - 2.5, front - 2.3, |s| {
        (gear(track, s).position_m - a.position_m).length() - 2.4
    });
    let b = gear(track, rear);
    let q = rotation_between(
        Frame::new(a.orientation).unwrap().tangent(),
        a.position_m - b.position_m,
    )
    .unwrap()
    .compose(a.orientation)
    .normalized()
    .unwrap();
    let body = Pose {
        orientation: q,
        position_m: b.position_m
            - Frame::new(q)
                .unwrap()
                .local_to_world(Vec3::new(-1.2, -0.8, 0.)),
    };
    [body, a, b]
}

/// Directed scalar geometry constructs an independent witness. It is test-only
/// and assumes this conventional fixture, never a production graph algorithm.
/// Asymmetric hitches change fixture anchors BEFORE construction, not in a solve.
pub fn witness(train: &mut Train, track: &GeneratedTrack, asymmetric: bool) -> ConsistInspection {
    if asymmetric {
        for j in &mut train.connections {
            if j.id.starts_with("coupler-") {
                j.anchor_a_m.y += if j.id.ends_with("front") { 0.13 } else { -0.07 };
                j.anchor_a_m.z += if j.id.ends_with("front") { 0.09 } else { -0.04 };
            }
        }
    }
    let mut poses = HashMap::new();
    // Match the unchanged production pin's initial station, independently:
    // car chord seed, front gear's first reference, closest track station.
    let span = root(2.3, 2.5, |h| {
        (track.sample_at_distance(200. + h * 0.5).unwrap().position_m
            - track.sample_at_distance(200. - h * 0.5).unwrap().position_m)
            .length()
            - 2.4
    });
    let seed_front = track.sample_at_distance(200. + span * 0.5).unwrap();
    let pin = station(
        track,
        seed_front.position_m - seed_front.frame.tangent() * 0.2,
        200. + span * 0.5,
    );
    let mut center = root(201., 201.4, |s| {
        station(track, point(gear(track, s), Vec3::new(-0.2, 0., 0.)), s) - pin
    });
    for n in 0..train.cars.len() {
        if n > 0 {
            let front = train
                .connections
                .iter()
                .find(|j| j.id == format!("coupler-{n}-front"))
                .unwrap();
            let rear = train
                .connections
                .iter()
                .find(|j| j.id == format!("coupler-{n}-rear"))
                .unwrap();
            let a = point(poses[&front.component_a], front.anchor_a_m);
            let previous = center;
            center = root(previous - 4.4, previous - 3.6, |s| {
                (point(car(track, s)[0], rear.anchor_a_m) - a).length() - 1.
            });
            let b = point(car(track, center)[0], rear.anchor_a_m);
            let q = rotation_between(Vec3::X, a - b).unwrap();
            poses.insert(
                format!("drawbar-{n}"),
                Pose {
                    position_m: (a + b) * 0.5,
                    orientation: q,
                },
            );
        }
        let [body, front, rear] = car(track, center);
        poses.insert(format!("car-{}", n + 1), body);
        poses.insert(format!("gear-{}-1", n + 1), front);
        poses.insert(format!("gear-{}-2", n + 1), rear);
    }
    let components = train
        .components
        .iter()
        .map(|c| {
            let pose = poses[&c.id];
            ComponentPose {
                id: c.id.clone(),
                station_m: station(track, pose.position_m, 200.),
                pose,
            }
        })
        .collect();
    let joints = train
        .connections
        .iter()
        .map(|j| {
            let a = point(poses[&j.component_a], j.anchor_a_m);
            let b = point(poses[&j.component_b], j.anchor_b_m);
            JointResidual {
                id: j.id.clone(),
                anchor_a_m: a,
                anchor_b_m: b,
                error_m: (a - b).length(),
            }
        })
        .collect();
    ConsistInspection { components, joints }
}

/// Independently evaluate every edge, both gear guides and rail twist. No
/// production diagnostic residual or assembly success is used as the oracle.
pub fn verify(train: &Train, track: &GeneratedTrack, w: &ConsistInspection) -> (f64, f64, f64) {
    verify_with_tolerance(train, track, w, 2e-10)
}

pub fn verify_with_tolerance(
    train: &Train,
    track: &GeneratedTrack,
    w: &ConsistInspection,
    tolerance: f64,
) -> (f64, f64, f64) {
    let poses: HashMap<_, _> = w.components.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut gap: f64 = 0.;
    let mut guide: f64 = 0.;
    let mut angular: f64 = 0.;
    for j in &train.connections {
        let a = poses[j.component_a.as_str()].pose;
        let b = poses[j.component_b.as_str()].pose;
        gap = gap.max((point(a, j.anchor_a_m) - point(b, j.anchor_b_m)).length());
        if let JointModel::RollLock { frame_a, frame_b } = j.model {
            let q = a
                .orientation
                .compose(frame_a)
                .conjugate()
                .compose(b.orientation.compose(frame_b));
            angular = angular.max((2. * q.x.atan2(q.w)).abs());
        } else {
            assert!(matches!(j.model, JointModel::Ball));
        }
    }
    for c in &train.components {
        let p = poses[c.id.as_str()];
        for g in &c.guide_references_m {
            let v = point(p.pose, *g);
            guide = guide.max(
                (v - track
                    .sample_at_distance(station(track, v, p.station_m))
                    .unwrap()
                    .position_m)
                    .length(),
            );
        }
        if !c.guide_references_m.is_empty() {
            let rail = track
                .sample_at_distance(station(track, p.pose.position_m, p.station_m))
                .unwrap()
                .frame;
            let q = rail.orientation().conjugate().compose(p.pose.orientation);
            angular = angular.max((2. * q.x.atan2(q.w)).abs());
        }
    }
    assert!(
        gap <= tolerance && guide <= tolerance && angular <= tolerance,
        "independent check: gap={gap} guide={guide} angular={angular}"
    );
    (gap, guide, angular)
}

/// A real five-body shared-gear chain, with a constructive curved witness;
/// unlike the M1 redundant extra-edge fixture this has no contradictory span.
pub fn shared(track: &GeneratedTrack) -> (Train, ConsistInspection) {
    use qx_document::train::{DevelopmentArrangement, mechanism::Placement};
    let mut train = Train::development_consist(
        "feasible-shared".into(),
        2,
        DevelopmentArrangement::Conventional,
    )
    .unwrap();
    train
        .components
        .retain(|c| c.id != "gear-2-1" && c.id != "drawbar-1");
    train.connections.retain(|c| !c.id.starts_with("coupler-"));
    train.cars[1].bogie_ids[0] = "gear-1-2".into();
    train
        .connections
        .iter_mut()
        .find(|j| j.id == "suspension-2-1")
        .unwrap()
        .component_b = "gear-1-2".into();
    for c in &mut train.components {
        if (c.id == "car-2" || c.id == "gear-2-2")
            && let Some(Placement::Track { offset_m, .. }) = &mut c.placement
        {
            *offset_m += 1.6;
        }
    }
    train.validate().unwrap();
    let [a, front, middle] = car(track, 201.2);
    let middle_station = station(track, middle.position_m, 200.);
    let [b, _, rear] = car(track, middle_station);
    let poses = HashMap::from([
        ("car-1", a),
        ("gear-1-1", front),
        ("gear-1-2", middle),
        ("car-2", b),
        ("gear-2-2", rear),
    ]);
    let components = train
        .components
        .iter()
        .map(|c| ComponentPose {
            id: c.id.clone(),
            pose: poses[c.id.as_str()],
            station_m: station(track, poses[c.id.as_str()].position_m, 200.),
        })
        .collect();
    let joints = train
        .connections
        .iter()
        .map(|j| {
            let a = point(poses[j.component_a.as_str()], j.anchor_a_m);
            let b = point(poses[j.component_b.as_str()], j.anchor_b_m);
            JointResidual {
                id: j.id.clone(),
                anchor_a_m: a,
                anchor_b_m: b,
                error_m: (a - b).length(),
            }
        })
        .collect();
    (train, ConsistInspection { components, joints })
}
