//! Read-only reconstruction of CURRENT impulse rows. Checked against the
//! existing velocity diagnostic, plus independent position finite differences.
use super::matrix::Matrix;
use qx_document::train::mechanism::JointModel;
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    solver::GeneratedTrack,
};
use qx_physics::{Pose, TrackAttitude, TrackGuide, World};

pub const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
pub fn point(p: Pose, local: Vec3) -> Vec3 {
    p.position_m + Frame::new(p.orientation).unwrap().local_to_world(local)
}
pub fn perturb(p: Pose, d: &[f64]) -> Pose {
    let w = Vec3::new(d[3], d[4], d[5]);
    Pose {
        position_m: p.position_m + Vec3::new(d[0], d[1], d[2]),
        orientation: if w.length() == 0. {
            p.orientation
        } else {
            Quaternion::axis_angle(w, w.length())
                .unwrap()
                .compose(p.orientation)
                .normalized()
                .unwrap()
        },
    }
}
pub fn twist(a: Quaternion, b: Quaternion) -> f64 {
    let q = a.conjugate().compose(b);
    assert!(q.w * q.w + q.x * q.x > 1e-12);
    (2. * q.x.atan2(q.w) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}
pub fn roll_axis(a: Frame, b: Frame) -> Vec3 {
    let q = a.orientation().conjugate().compose(b.orientation());
    let d = q.w * q.w + q.x * q.x;
    assert!(d > 1e-12);
    a.local_to_world(Vec3::new(
        1.,
        (q.w * q.z + q.x * q.y) / d,
        (q.x * q.z - q.w * q.y) / d,
    ))
}
pub fn row(n: usize, body: usize, linear: Vec3, angular: Vec3) -> Vec<f64> {
    let mut r = vec![0.; 6 * n];
    for (i, v) in [
        linear.x, linear.y, linear.z, angular.x, angular.y, angular.z,
    ]
    .into_iter()
    .enumerate()
    {
        r[6 * body + i] = v;
    }
    r
}
pub fn point_row(n: usize, body: usize, radius: Vec3, normal: Vec3) -> Vec<f64> {
    row(n, body, normal, radius.cross(normal))
}
pub fn projection(track: &GeneratedTrack, p: Vec3, mut s: f64) -> f64 {
    for _ in 0..12 {
        let t = track.sample_at_distance(s).unwrap();
        let d = (p - t.position_m).dot(t.frame.tangent());
        assert!(d.abs() < 2.);
        s = (s + d).clamp(0., track.length_m());
        if d.abs() < 1e-10 {
            return s;
        }
    }
    panic!("research projection did not converge");
}

pub fn current(
    world: &World,
    track: &GeneratedTrack,
    guides: &[TrackGuide],
    attitudes: &[TrackAttitude],
) -> Matrix {
    let count = world.bodies.len();
    let mut rows = vec![];
    for j in &world.joints {
        let a = world.bodies[j.body_a].pose;
        let b = world.bodies[j.body_b].pose;
        for n in AXES {
            let ra = point(a, j.anchor_a_m) - a.position_m;
            let rb = point(b, j.anchor_b_m) - b.position_m;
            let mut r = point_row(count, j.body_b, rb, n);
            let s = point_row(count, j.body_a, ra, n);
            for (x, y) in r.iter_mut().zip(s) {
                *x -= y;
            }
            rows.push(r);
        }
    }
    for j in &world.angular_joints {
        let (qa, qb) = match j.model {
            JointModel::Fixed { frame_a, frame_b }
            | JointModel::Hinge {
                frame_a, frame_b, ..
            }
            | JointModel::RollLock { frame_a, frame_b } => (frame_a, frame_b),
            JointModel::Ball => unreachable!(),
        };
        let a = Frame::new(world.bodies[j.body_a].pose.orientation.compose(qa)).unwrap();
        let b = Frame::new(world.bodies[j.body_b].pose.orientation.compose(qb)).unwrap();
        let axes = match j.model {
            JointModel::RollLock { .. } => vec![roll_axis(a, b)],
            JointModel::Fixed { .. } => vec![a.tangent(), a.normal(), a.lateral()],
            _ => vec![a.tangent(), a.normal()],
        };
        for n in axes {
            let mut r = row(count, j.body_b, Vec3::ZERO, n);
            let s = row(count, j.body_a, Vec3::ZERO, n);
            for (x, y) in r.iter_mut().zip(s) {
                *x -= y;
            }
            rows.push(r);
        }
    }
    for g in guides {
        let p = world.bodies[g.body].pose;
        let r = point(p, g.local_point_m) - p.position_m;
        let s = projection(track, p.position_m + r, g.station_m);
        let t = track.sample_at_distance(s).unwrap();
        for n in [t.frame.normal(), t.frame.lateral()] {
            rows.push(point_row(count, g.body, r, n));
        }
    }
    for a in attitudes {
        let p = world.bodies[a.body].pose;
        let r = point(p, a.local_point_m) - p.position_m;
        let s = projection(track, p.position_m + r, a.station_m);
        let t = track.sample_at_distance(s).unwrap();
        let lo = (s - 0.001).max(0.);
        let hi = (s + 0.001).min(track.length_m());
        let qa = track.sample_at_distance(lo).unwrap().frame.orientation();
        let qb = track.sample_at_distance(hi).unwrap().frame.orientation();
        let derivative = qb.compose(qa.conjugate()).rotation_vector().unwrap() / (hi - lo);
        let axis = roll_axis(
            t.frame,
            Frame::new(p.orientation.compose(a.local_frame)).unwrap(),
        );
        let delta = p.position_m + r - t.position_m;
        let denominator = 1. - delta.dot(derivative.cross(t.frame.tangent()));
        assert!(denominator > 1e-6);
        let linear = t.frame.tangent() * (-axis.dot(derivative) / denominator);
        rows.push(row(count, a.body, linear, axis + r.cross(linear)));
    }
    rows
}

/// Independent signed position equations for exact straight-track fixtures.
/// Curved current guide velocity rows approximate the actual sampler derivative;
/// do not assert exact nonlinear integrability there.
pub fn straight_residual(
    world: &World,
    guides: &[TrackGuide],
    attitudes: &[TrackAttitude],
    delta: &[f64],
) -> Vec<f64> {
    let poses: Vec<_> = world
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| perturb(b.pose, &delta[6 * i..6 * i + 6]))
        .collect();
    let mut r = vec![];
    for j in &world.joints {
        let d = point(poses[j.body_b], j.anchor_b_m) - point(poses[j.body_a], j.anchor_a_m);
        r.extend([d.x, d.y, d.z]);
    }
    for j in &world.angular_joints {
        match j.model {
            JointModel::RollLock { frame_a, frame_b } => r.push(twist(
                poses[j.body_a].orientation.compose(frame_a),
                poses[j.body_b].orientation.compose(frame_b),
            )),
            JointModel::Hinge {
                frame_a, frame_b, ..
            } => {
                let a = Frame::new(poses[j.body_a].orientation.compose(frame_a)).unwrap();
                let b = Frame::new(poses[j.body_b].orientation.compose(frame_b)).unwrap();
                let e = a.lateral().cross(b.lateral());
                r.extend([e.dot(a.tangent()), e.dot(a.normal())]);
            }
            _ => panic!("not in research straight fixtures"),
        }
    }
    for g in guides {
        let p = point(poses[g.body], g.local_point_m);
        r.extend([p.y, p.z]);
    }
    for a in attitudes {
        r.push(twist(
            Quaternion::IDENTITY,
            poses[a.body].orientation.compose(a.local_frame),
        ));
    }
    r
}
