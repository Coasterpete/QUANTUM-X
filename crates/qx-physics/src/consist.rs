//! Experimental consist inspection and initialization from persisted definitions.
//! Track-frame placement is an approximation: connection residuals are explicit,
//! and the resulting poses are only initial conditions for independent dynamics.
use crate::{BallJoint, Body, MassProperties, Pose, Result, TrackGuide, World};
use qx_document::train::{Train, mechanism::Placement};
use qx_geometry::{
    math::{Frame, Vec3, rotation_between},
    solver::GeneratedTrack,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentPose {
    pub id: String,
    pub station_m: f64,
    pub pose: Pose,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JointResidual {
    pub id: String,
    pub anchor_a_m: Vec3,
    pub anchor_b_m: Vec3,
    pub error_m: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ConsistInspection {
    pub components: Vec<ComponentPose>,
    pub joints: Vec<JointResidual>,
}
fn point(p: &Pose, local: Vec3) -> Result<Vec3> {
    let result = p.position_m
        + Frame::new(p.orientation)
            .map_err(|e| e.to_string())?
            .local_to_world(local);
    if !result.is_finite() {
        return Err("component transform overflow".into());
    }
    Ok(result)
}
/// Explicit track/mount placements support arbitrary component graphs. The
/// legacy Schema 5 adapter places bogies from the first attached car's station. Each legacy car
/// is then fit rigidly to up to two of its bogie anchors; extra/unsatisfied
/// connections remain measurable residuals. Unsupported unconnected components
/// are rejected instead of receiving an invented transform.
pub fn inspect(train: &Train, track: &GeneratedTrack, station_m: f64) -> Result<ConsistInspection> {
    train.validate().map_err(|e| e.to_string())?;
    if !station_m.is_finite() {
        return Err("station must be finite".into());
    }
    let mut poses: HashMap<&str, ComponentPose> = HashMap::new();
    for c in &train.components {
        if let Some(Placement::Track { offset_m, frame }) = &c.placement {
            let station = station_m + offset_m;
            let s = track
                .sample_at_distance(station)
                .map_err(|e| e.to_string())?;
            poses.insert(
                &c.id,
                ComponentPose {
                    id: c.id.clone(),
                    station_m: station,
                    pose: Pose {
                        position_m: s.position_m + s.frame.local_to_world(frame.translation_m),
                        orientation: s
                            .frame
                            .orientation()
                            .compose(frame.rotation)
                            .normalized()
                            .map_err(|e| e.to_string())?,
                    },
                },
            );
        }
    }
    for car in &train.cars {
        if train
            .components
            .iter()
            .find(|c| c.id == car.body_id)
            .is_some_and(|c| c.placement.is_some())
        {
            continue;
        }
        let station = station_m - car.offset_m;
        let sample = track
            .sample_at_distance(station)
            .map_err(|e| e.to_string())?;
        poses.insert(
            &car.body_id,
            ComponentPose {
                id: car.body_id.clone(),
                station_m: station,
                pose: Pose {
                    position_m: sample.position_m,
                    orientation: sample.frame.orientation(),
                },
            },
        );
        for bogie in &car.bogie_ids {
            if poses.contains_key(bogie.as_str()) {
                continue;
            }
            if train
                .components
                .iter()
                .find(|c| c.id == *bogie)
                .is_some_and(|c| c.placement.is_some())
            {
                return Err(format!(
                    "legacy car {} references unresolved mount {bogie}; give the car an explicit placement",
                    car.body_id
                ));
            }
            let (body_anchor, bogie_anchor) = train
                .connections
                .iter()
                .find_map(|j| {
                    if j.component_a == car.body_id && j.component_b == *bogie {
                        Some((j.anchor_a_m, j.anchor_b_m))
                    } else if j.component_b == car.body_id && j.component_a == *bogie {
                        Some((j.anchor_b_m, j.anchor_a_m))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| format!("bogie {bogie} needs a joint to car {}", car.body_id))?;
            let bs = station + body_anchor.x - bogie_anchor.x;
            let b = track.sample_at_distance(bs).map_err(|e| e.to_string())?;
            let orientation = b.frame.orientation();
            let position_m = b.position_m - b.frame.local_to_world(bogie_anchor);
            poses.insert(
                bogie,
                ComponentPose {
                    id: bogie.clone(),
                    station_m: bs,
                    pose: Pose {
                        position_m,
                        orientation,
                    },
                },
            );
        }
    }
    for car in &train.cars {
        if train
            .components
            .iter()
            .find(|c| c.id == car.body_id)
            .is_some_and(|c| c.placement.is_some())
        {
            continue;
        }
        let mut anchors = Vec::new();
        for j in &train.connections {
            let pair = if j.component_a == car.body_id && car.bogie_ids.contains(&j.component_b) {
                Some((j.anchor_a_m, &j.component_b, j.anchor_b_m))
            } else if j.component_b == car.body_id && car.bogie_ids.contains(&j.component_a) {
                Some((j.anchor_b_m, &j.component_a, j.anchor_a_m))
            } else {
                None
            };
            if let Some((local, id, other)) = pair {
                anchors.push((local, point(&poses[id.as_str()].pose, other)?));
            }
        }
        let mut body = poses[car.body_id.as_str()].clone();
        if anchors.len() >= 2 {
            let frame = Frame::new(body.pose.orientation).map_err(|e| e.to_string())?;
            let source = frame.local_to_world(anchors[1].0 - anchors[0].0);
            let target = anchors[1].1 - anchors[0].1;
            let rotation = rotation_between(source, target)
                .map_err(|e| format!("cannot fit car {}: {e}", car.body_id))?;
            body.pose.orientation = rotation
                .compose(body.pose.orientation)
                .normalized()
                .map_err(|e| e.to_string())?;
        }
        if !anchors.is_empty() {
            let frame = Frame::new(body.pose.orientation).map_err(|e| e.to_string())?;
            body.pose.position_m = anchors.iter().fold(Vec3::ZERO, |sum, (a, b)| {
                sum + *b - frame.local_to_world(*a)
            }) / anchors.len() as f64;
        }
        poses.insert(&car.body_id, body);
    }
    // Resolve explicit mounts without assuming a car/bogie count, ordering, or passenger side.
    for _ in 0..train.components.len() {
        let mut changed = false;
        for c in &train.components {
            if poses.contains_key(c.id.as_str()) {
                continue;
            }
            if let Some(Placement::Mounted { parent, frame }) = &c.placement
                && let Some(p) = poses.get(parent.as_str()).cloned()
            {
                let basis = Frame::new(p.pose.orientation).map_err(|e| e.to_string())?;
                poses.insert(
                    &c.id,
                    ComponentPose {
                        id: c.id.clone(),
                        station_m: p.station_m,
                        pose: Pose {
                            position_m: p.pose.position_m
                                + basis.local_to_world(frame.translation_m),
                            orientation: p
                                .pose
                                .orientation
                                .compose(frame.rotation)
                                .normalized()
                                .map_err(|e| e.to_string())?,
                        },
                    },
                );
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let components: Vec<_> = train
        .components
        .iter()
        .map(|c| {
            poses
                .get(c.id.as_str())
                .cloned()
                .ok_or_else(|| format!("component {} has no supported placement", c.id))
        })
        .collect::<Result<_>>()?;
    let mut joints = Vec::new();
    for j in &train.connections {
        let a = point(&poses[j.component_a.as_str()].pose, j.anchor_a_m)?;
        let b = point(&poses[j.component_b.as_str()].pose, j.anchor_b_m)?;
        joints.push(JointResidual {
            id: j.id.clone(),
            anchor_a_m: a,
            anchor_b_m: b,
            error_m: (a - b).length(),
        });
    }
    Ok(ConsistInspection { components, joints })
}
/// Builds independent rigid bodies and spherical joints from the same persisted
/// definition. Guide points are bilateral centerline approximations, not contact.
pub fn initialize(
    train: &Train,
    track: &GeneratedTrack,
    station_m: f64,
    gravity_mps2: Vec3,
    iterations: u32,
    substeps: u32,
) -> Result<(World, Vec<TrackGuide>, ConsistInspection)> {
    if train
        .components
        .iter()
        .map(|c| c.guide_references_m.len())
        .sum::<usize>()
        > 4096
    {
        return Err("playback supports at most 4096 track guides".into());
    }
    let inspection = inspect(train, track, station_m)?;
    if inspection.joints.iter().any(|joint| joint.error_m > 0.05) {
        return Err(
            "initial joint residual exceeds 0.05 m; revise the mechanism or station".into(),
        );
    }
    let indices: HashMap<_, _> = train
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    let bodies = train
        .components
        .iter()
        .zip(&inspection.components)
        .map(|(c, p)| {
            Ok(Body {
                mass: MassProperties {
                    mass_kg: c.mass_kg,
                    principal_inertia_kg_m2: c.inertia_kg_m2,
                },
                pose: Pose {
                    position_m: point(&p.pose, c.center_of_mass_m)?,
                    orientation: p.pose.orientation,
                },
                linear_velocity_mps: Vec3::ZERO,
                angular_velocity_radps: Vec3::ZERO,
                force_n: Vec3::ZERO,
                torque_nm: Vec3::ZERO,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let joints = train
        .connections
        .iter()
        .map(|j| BallJoint {
            body_a: indices[j.component_a.as_str()],
            body_b: indices[j.component_b.as_str()],
            anchor_a_m: j.anchor_a_m
                - train.components[indices[j.component_a.as_str()]].center_of_mass_m,
            anchor_b_m: j.anchor_b_m
                - train.components[indices[j.component_b.as_str()]].center_of_mass_m,
        })
        .collect();
    let mut guides = Vec::new();
    for (i, c) in train.components.iter().enumerate() {
        for p in &c.guide_references_m {
            let station = crate::solver::project_station(
                track,
                point(&inspection.components[i].pose, *p)?,
                inspection.components[i].station_m,
            )?;
            let target = track
                .sample_at_distance(station)
                .map_err(|e| e.to_string())?;
            if (point(&inspection.components[i].pose, *p)? - target.position_m).length() > 0.05 {
                return Err(format!("initial guide on {} exceeds 0.05 m residual", c.id));
            }
            guides.push(TrackGuide {
                body: i,
                local_point_m: *p - c.center_of_mass_m,
                station_m: station,
            });
        }
    }
    let angular_joints = train
        .connections
        .iter()
        .filter(|j| !matches!(j.model, qx_document::train::mechanism::JointModel::Ball))
        .map(|j| crate::AngularJoint {
            body_a: indices[j.component_a.as_str()],
            body_b: indices[j.component_b.as_str()],
            model: j.model.clone(),
        })
        .collect();
    let world = World {
        bodies,
        joints,
        angular_joints,
        guides: vec![],
        gravity_mps2,
        iterations,
        substeps,
    };
    world.validate()?;
    for j in &world.angular_joints {
        if j.error(&world.bodies)? > 0.05 {
            return Err(
                "initial angular joint residual exceeds 0.05 rad; revise the mounting frames"
                    .into(),
            );
        }
    }
    Ok((world, guides, inspection))
}

/// Converts simulated mass-center poses back to component origins for the same renderer/inspector.
pub fn snapshot(
    train: &Train,
    world: &World,
    guides: &[TrackGuide],
    initial: &ConsistInspection,
) -> Result<ConsistInspection> {
    if train.components.len() != world.bodies.len()
        || initial.components.len() != world.bodies.len()
    {
        return Err("simulation/definition component mismatch".into());
    }
    let mut result = initial.clone();
    for (i, ((c, body), p)) in train
        .components
        .iter()
        .zip(&world.bodies)
        .zip(&mut result.components)
        .enumerate()
    {
        p.pose = Pose {
            position_m: point(&body.pose, -c.center_of_mass_m)?,
            orientation: body.pose.orientation,
        };
        if let Some(g) = guides.iter().find(|g| g.body == i) {
            p.station_m = g.station_m;
        }
    }
    for (j, residual) in train.connections.iter().zip(&mut result.joints) {
        let a = result
            .components
            .iter()
            .find(|p| p.id == j.component_a)
            .ok_or("missing simulated component A")?;
        let b = result
            .components
            .iter()
            .find(|p| p.id == j.component_b)
            .ok_or("missing simulated component B")?;
        residual.anchor_a_m = point(&a.pose, j.anchor_a_m)?;
        residual.anchor_b_m = point(&b.pose, j.anchor_b_m)?;
        residual.error_m = (residual.anchor_a_m - residual.anchor_b_m).length();
    }
    Ok(result)
}
