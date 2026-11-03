//! Initial-condition assembly only. Rigid transforms preserve every authored
//! dimension. Bounded chord searches seed a local constraint projection; neither
//! this module nor its station variables is used to advance physical motion.
use crate::{
    Pose, Result,
    consist::{ConsistInspection, JointResidual},
};
use qx_document::train::{
    Train,
    mechanism::{JointModel, Placement},
};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3, rotation_between},
    solver::GeneratedTrack,
};
use std::collections::HashMap;

const TOLERANCE: f64 = 1e-5;

fn frame(p: Pose) -> Frame {
    Frame::new(p.orientation).expect("validated assembly rotation")
}
fn point(p: Pose, a: Vec3) -> Vec3 {
    p.position_m + frame(p).local_to_world(a)
}
fn rotate(p: &mut Pose, v: Vec3) -> Result<()> {
    let angle = v.length();
    if !angle.is_finite() {
        return Err("assembly rotation overflow; check physical dimensions and anchors".into());
    }
    if angle > 1e-15 {
        p.orientation = Quaternion::axis_angle(v / angle, angle.min(0.15))
            .and_then(|q| q.compose(p.orientation).normalized())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
/// Rotate a rigid pose about a mechanical anchor or guide centroid. This keeps
/// angular projection from needlessly displacing that point as rotation about
/// the mass center would. Used only for initial-condition assembly.
fn rotate_about(p: &mut Pose, pivot_m: Vec3, v: Vec3) -> Result<()> {
    let angle = v.length();
    if !angle.is_finite() || !pivot_m.is_finite() {
        return Err("assembly rotation overflow; check physical dimensions and anchors".into());
    }
    if angle > 1e-15 {
        let q = Quaternion::axis_angle(v / angle, angle.min(0.15)).map_err(|e| e.to_string())?;
        let rotation = Frame::new(q).map_err(|e| e.to_string())?;
        p.position_m = pivot_m + rotation.local_to_world(p.position_m - pivot_m);
        p.orientation = q
            .compose(p.orientation)
            .normalized()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Nearest forward chord root, never a remote branch of a loop. Explicitly
/// bounded work and open endpoints; no extrapolation or implicit circuit wrap.
fn chord(track: &GeneratedTrack, center: f64, length: f64) -> Result<(f64, f64)> {
    if !length.is_finite() || length < 1e-8 {
        return Err("coincident rigid pivots cannot define a chord".into());
    }
    let max = (4. * length).min(2. * center.min(track.length_m() - center));
    let residual = |span: f64| -> Result<f64> {
        let a = track
            .sample_at_distance(center - span * 0.5)
            .map_err(|e| e.to_string())?;
        let b = track
            .sample_at_distance(center + span * 0.5)
            .map_err(|e| e.to_string())?;
        Ok((b.position_m - a.position_m).length() - length)
    };
    let mut lo = 0.;
    for n in 1..=96 {
        let hi = max * n as f64 / 96.;
        if residual(hi)? >= 0. {
            let mut hi = hi;
            for _ in 0..48 {
                let mid = (lo + hi) * 0.5;
                if residual(mid)? < 0. {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Ok((center - (lo + hi) * 0.25, center + (lo + hi) * 0.25));
        }
        lo = hi;
    }
    Err(format!(
        "no local rigid chord of {length:.6} m at station {center:.6}; curvature or open endpoint is incompatible"
    ))
}

struct Guide {
    body: usize,
    local: Vec3,
    station: f64,
    initial: f64,
}

pub(crate) fn assemble(
    train: &Train,
    track: &GeneratedTrack,
    mut result: ConsistInspection,
) -> Result<ConsistInspection> {
    if train
        .components
        .iter()
        .map(|c| c.guide_references_m.len().max(1))
        .sum::<usize>()
        > 4096
    {
        return Err("assembly supports at most 4096 guide references".into());
    }
    let ids: HashMap<_, _> = train
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    let mut poses: Vec<_> = result.components.iter().map(|c| c.pose).collect();
    let mut stations: Vec<_> = result.components.iter().map(|c| c.station_m).collect();
    let mut seeded = vec![false; poses.len()];
    // The two-pivot fast seed is local to a car, never a restriction on the
    // component graph. Shared gear is placed once and reconciled below.
    for car in &train.cars {
        if car.bogie_ids.len() != 2 {
            continue;
        }
        let body = ids[car.body_id.as_str()];
        let mut pivots = Vec::new();
        for id in &car.bogie_ids {
            let gear = ids[id.as_str()];
            let j = train.connections.iter().find_map(|j| {
                if j.component_a == car.body_id && j.component_b == *id {
                    Some((j.anchor_a_m, j.anchor_b_m))
                } else if j.component_b == car.body_id && j.component_a == *id {
                    Some((j.anchor_b_m, j.anchor_a_m))
                } else {
                    None
                }
            });
            if let Some((a, b)) = j {
                pivots.push((gear, a, b));
            }
        }
        if pivots.len() != 2 || pivots.iter().any(|(i, _, _)| seeded[*i]) {
            continue;
        }
        pivots.sort_by(|a, b| stations[a.0].total_cmp(&stations[b.0]));
        let (rear, front) = (pivots[0], pivots[1]);
        // Off-axis gear anchors need the general graph projection, since the
        // anchor locus is no longer the centerline itself.
        if rear.2.length() > 1e-12 || front.2.length() > 1e-12
            || [rear.0,front.0].iter().any(|i|matches!(&train.components[*i].placement,Some(Placement::Track {frame,..}) if frame.translation_m.length()>1e-12)) {
            continue;
        }
        let center = (stations[rear.0] + stations[front.0]) * 0.5;
        let (rs, fs) = chord(track, center, (front.1 - rear.1).length())
            .map_err(|e| format!("car {}: {e}", car.body_id))?;
        for (i, s) in [(rear.0, rs), (front.0, fs)] {
            let sample = track.sample_at_distance(s).map_err(|e| e.to_string())?;
            poses[i].position_m = sample.position_m;
            let rotation = match &train.components[i].placement {
                Some(Placement::Track { frame, .. }) => frame.rotation,
                _ => Quaternion::IDENTITY,
            };
            poses[i].orientation = sample
                .frame
                .orientation()
                .compose(rotation)
                .normalized()
                .map_err(|e| e.to_string())?;
            stations[i] = s;
            seeded[i] = true;
        }
        let rear_frame = track
            .sample_at_distance(rs)
            .map_err(|e| e.to_string())?
            .frame;
        let front_frame = track
            .sample_at_distance(fs)
            .map_err(|e| e.to_string())?
            .frame;
        let correction = match &train.components[body].placement {
            Some(Placement::Track { frame, .. }) => frame.rotation,
            _ => Quaternion::IDENTITY,
        };
        let base = Frame::new(
            rear_frame
                .orientation()
                .slerp(front_frame.orientation(), 0.5)
                .map_err(|e| e.to_string())?
                .compose(correction),
        )
        .map_err(|e| e.to_string())?;
        let q = rotation_between(
            base.local_to_world(front.1 - rear.1),
            poses[front.0].position_m - poses[rear.0].position_m,
        )
        .and_then(|q| q.compose(base.orientation()).normalized())
        .map_err(|e| e.to_string())?;
        poses[body].orientation = q;
        poses[body].position_m =
            poses[rear.0].position_m - frame(poses[body]).local_to_world(rear.1);
    }
    // Mounts describe initialization, not hidden welds. Re-evaluate them after
    // chord fitting, then constrain only actual authored mechanical connections.
    for _ in 0..poses.len() {
        for (i, c) in train.components.iter().enumerate() {
            if let Some(Placement::Mounted {
                parent,
                frame: mount,
            }) = &c.placement
            {
                let p = poses[ids[parent.as_str()]];
                poses[i] = Pose {
                    position_m: point(p, mount.translation_m),
                    orientation: p
                        .orientation
                        .compose(mount.rotation)
                        .normalized()
                        .map_err(|e| e.to_string())?,
                };
            }
        }
    }
    let mut guides = Vec::new();
    for (i, c) in train.components.iter().enumerate() {
        let mut references = c.guide_references_m.clone();
        if references.is_empty() && c.kind.is_running_gear() {
            // Legacy inspection treats the joint pivot as the track reference.
            let local = if c.placement.is_none() {
                train
                    .connections
                    .iter()
                    .find_map(|j| {
                        if j.component_a == c.id {
                            Some(j.anchor_a_m)
                        } else if j.component_b == c.id {
                            Some(j.anchor_b_m)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(Vec3::ZERO)
            } else if let Some(Placement::Track { frame: f, .. }) = &c.placement {
                Frame::new(f.rotation)
                    .map_err(|e| e.to_string())?
                    .world_to_local(-f.translation_m)
            } else {
                continue;
            };
            references.push(local);
        }
        for local in references {
            let s = crate::solver::project_station(track, point(poses[i], local), stations[i])?;
            guides.push(Guide {
                body: i,
                local,
                station: s,
                initial: s,
            });
        }
    }
    let radii: Vec<_> = train
        .components
        .iter()
        .map(|c| (c.dimensions_m.length() * 0.5).max(0.25))
        .collect();
    let mut attitudes = crate::consist::rail_attitudes(train, &stations);
    // Local position-based assembly uses unit translation weights and rotation
    // weights scaled by component extent. These are numerical weights, not mass,
    // springs, forces or a physical timestep. Every iterate remains rigid.
    let mut worst = (String::new(), f64::INFINITY);
    for iteration in 0..2400 {
        if poses.iter().any(|p| !p.position_m.is_finite()) {
            return Err("assembly position overflow; check physical dimensions and anchors".into());
        }
        for j in &train.connections {
            let (a, b) = (ids[j.component_a.as_str()], ids[j.component_b.as_str()]);
            for n in [Vec3::X, Vec3::Y, Vec3::Z] {
                let ra = frame(poses[a]).local_to_world(j.anchor_a_m);
                let rb = frame(poses[b]).local_to_world(j.anchor_b_m);
                let e = (poses[b].position_m + rb - poses[a].position_m - ra).dot(n);
                let ga = ra.cross(n);
                let gb = rb.cross(n);
                let wa = 1. / (radii[a] * radii[a]);
                let wb = 1. / (radii[b] * radii[b]);
                let d = (e / (2. + ga.dot(ga) * wa + gb.dot(gb) * wb)).clamp(-0.2, 0.2);
                poses[a].position_m = poses[a].position_m + n * d;
                poses[b].position_m = poses[b].position_m - n * d;
                rotate(&mut poses[a], ga * (d * wa))?;
                rotate(&mut poses[b], gb * (-d * wb))?;
            }
            if let JointModel::RollLock { frame_a, frame_b } = j.model {
                let fa =
                    Frame::new(poses[a].orientation.compose(frame_a)).map_err(|e| e.to_string())?;
                let fb =
                    Frame::new(poses[b].orientation.compose(frame_b)).map_err(|e| e.to_string())?;
                let (axis, error) = crate::angular::roll_constraint(fa, fb)?;
                let correction = axis * (error / (2. * axis.dot(axis)));
                let pa = point(poses[a], j.anchor_a_m);
                let pb = point(poses[b], j.anchor_b_m);
                rotate_about(&mut poses[a], pa, correction)?;
                rotate_about(&mut poses[b], pb, -correction)?;
            }
            if let JointModel::Fixed { frame_a, frame_b }
            | JointModel::Hinge {
                frame_a, frame_b, ..
            } = j.model
            {
                let fa =
                    Frame::new(poses[a].orientation.compose(frame_a)).map_err(|e| e.to_string())?;
                let fb =
                    Frame::new(poses[b].orientation.compose(frame_b)).map_err(|e| e.to_string())?;
                let relative = fa.orientation().conjugate().compose(fb.orientation());
                let error = if matches!(j.model, JointModel::Fixed { .. }) {
                    fa.local_to_world(relative.rotation_vector().map_err(|e| e.to_string())?)
                } else {
                    rotation_between(fa.lateral(), fb.lateral())
                        .and_then(|q| q.rotation_vector())
                        .map_err(|e| e.to_string())?
                };
                rotate(&mut poses[a], error * 0.5)?;
                rotate(&mut poses[b], error * (-0.5))?;
                if let JointModel::Hinge {
                    limits: Some(l), ..
                } = j.model
                {
                    let theta = (2. * relative.z.atan2(relative.w) + std::f64::consts::PI)
                        .rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI;
                    let e = theta - theta.clamp(l.min_rad, l.max_rad);
                    rotate(&mut poses[a], fa.lateral() * (e * 0.5))?;
                    rotate(&mut poses[b], fa.lateral() * (-e * 0.5))?;
                }
            }
        }
        // Guide stations relax tangentially; one longitudinal point fixes the
        // global translation gauge while all mechanical dimensions stay rigid.
        for (index, g) in guides.iter_mut().enumerate() {
            let i = g.body;
            g.station = crate::solver::project_station(track, point(poses[i], g.local), g.station)?;
            if (g.station - g.initial).abs() > (radii[i] * 8.).max(4.) {
                return Err(format!(
                    "assembly of {} left its bounded station neighborhood",
                    train.components[i].id
                ));
            }
            // Pin one longitudinal reference so whole-consist translation cannot
            // drift while closing the chain. Other guides keep their own stations.
            if index == 0 {
                g.station = g.initial;
            }
            let s = track
                .sample_at_distance(g.station)
                .map_err(|e| e.to_string())?;
            let axes = [s.frame.normal(), s.frame.lateral(), s.frame.tangent()];
            for n in axes.into_iter().take(if index == 0 { 3 } else { 2 }) {
                let r = frame(poses[i]).local_to_world(g.local);
                let e = (poses[i].position_m + r - s.position_m).dot(n);
                let grad = r.cross(n);
                let w = 1. / (radii[i] * radii[i]);
                let d = (e / (1. + grad.dot(grad) * w)).clamp(-0.2, 0.2);
                poses[i].position_m = poses[i].position_m - n * d;
                rotate(&mut poses[i], grad * (-d * w))?;
            }
        }
        for attitude in attitudes.iter_mut() {
            let i = attitude.body;
            let pivot_m = point(poses[i], attitude.local_point_m);
            attitude.station_m =
                crate::solver::project_station(track, pivot_m, attitude.station_m)?;
            let s = track
                .sample_at_distance(attitude.station_m)
                .map_err(|e| e.to_string())?;
            let rail = Frame::new(poses[i].orientation.compose(attitude.local_frame))
                .map_err(|e| e.to_string())?;
            let (axis, error) = crate::angular::roll_constraint(s.frame, rail)?;
            rotate_about(&mut poses[i], pivot_m, axis * (-error / axis.dot(axis)))?;
        }
        if iteration % 8 != 0 {
            continue;
        }
        worst = (String::new(), 0.);
        for j in &train.connections {
            let gap = (point(poses[ids[j.component_a.as_str()]], j.anchor_a_m)
                - point(poses[ids[j.component_b.as_str()]], j.anchor_b_m))
            .length();
            let e = gap;
            if e > worst.1 {
                worst = (format!("connection {}", j.id), e);
            }
            if let JointModel::Fixed { frame_a, frame_b }
            | JointModel::Hinge {
                frame_a, frame_b, ..
            }
            | JointModel::RollLock { frame_a, frame_b } = j.model
            {
                let a = Frame::new(
                    poses[ids[j.component_a.as_str()]]
                        .orientation
                        .compose(frame_a),
                )
                .map_err(|e| e.to_string())?;
                let b = Frame::new(
                    poses[ids[j.component_b.as_str()]]
                        .orientation
                        .compose(frame_b),
                )
                .map_err(|e| e.to_string())?;
                let relative = a.orientation().conjugate().compose(b.orientation());
                let e = match j.model {
                    JointModel::Fixed { .. } => relative
                        .rotation_vector()
                        .map_err(|e| e.to_string())?
                        .length(),
                    JointModel::RollLock { .. } => crate::angular::roll_constraint(a, b)?.1.abs(),
                    JointModel::Hinge { limits, .. } => {
                        let swing = rotation_between(a.lateral(), b.lateral())
                            .and_then(|q| q.rotation_vector())
                            .map_err(|e| e.to_string())?
                            .length();
                        let theta = (2. * relative.z.atan2(relative.w) + std::f64::consts::PI)
                            .rem_euclid(std::f64::consts::TAU)
                            - std::f64::consts::PI;
                        limits.map_or(swing, |l| {
                            swing.max((theta - theta.clamp(l.min_rad, l.max_rad)).abs())
                        })
                    }
                    JointModel::Ball => 0.,
                };
                if e > worst.1 {
                    worst = (format!("angular joint {} (rad)", j.id), e);
                }
            }
        }
        for g in &guides {
            let target = track
                .sample_at_distance(g.station)
                .map_err(|e| e.to_string())?
                .position_m;
            let e = (point(poses[g.body], g.local) - target).length();
            if e > worst.1 {
                worst = (format!("guide on {}", train.components[g.body].id), e);
            }
        }
        for attitude in &attitudes {
            let s = track
                .sample_at_distance(attitude.station_m)
                .map_err(|e| e.to_string())?;
            let rail = Frame::new(
                poses[attitude.body]
                    .orientation
                    .compose(attitude.local_frame),
            )
            .map_err(|e| e.to_string())?;
            let e = crate::angular::roll_constraint(s.frame, rail)?.1.abs();
            if e > worst.1 {
                worst = (
                    format!("body attitude {} (rad)", train.components[attitude.body].id),
                    e,
                );
            }
        }
        if worst.1 <= TOLERANCE {
            break;
        }
    }
    if worst.1 > TOLERANCE {
        let unit = if worst.0.ends_with("(rad)") {
            "rad"
        } else {
            "m"
        };
        return Err(format!(
            "unsupported or overconstrained assembly: {} residual {:.6} {unit} exceeds {TOLERANCE} {unit}; check anchors, guide geometry, station and open endpoints",
            worst.0, worst.1
        ));
    }
    for (i, c) in result.components.iter_mut().enumerate() {
        c.pose = poses[i];
        c.station_m = guides
            .iter()
            .find(|g| g.body == i)
            .map_or(stations[i], |g| g.station);
    }
    result.joints = train
        .connections
        .iter()
        .map(|j| {
            let a = point(poses[ids[j.component_a.as_str()]], j.anchor_a_m);
            let b = point(poses[ids[j.component_b.as_str()]], j.anchor_b_m);
            JointResidual {
                id: j.id.clone(),
                anchor_a_m: a,
                anchor_b_m: b,
                error_m: (a - b).length(),
            }
        })
        .collect();
    Ok(result)
}
