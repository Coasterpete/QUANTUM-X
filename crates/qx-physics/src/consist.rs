//! Experimental consist inspection and initialization from persisted definitions.
//! Bounded rigid assembly produces initial conditions for independent dynamics.
use crate::{BallJoint, Body, MassProperties, Pose, Result, TrackGuide, World};
use qx_document::train::{Train, mechanism::Placement};
use qx_geometry::{
    math::{Frame, Vec3},
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
/// One presentation pose for both mesh instances and attached cameras. Linear
/// origin/station interpolation and shortest-arc unit-quaternion interpolation;
/// it is visual interpolation, not a second dynamics solve or extrapolation.
pub fn interpolate(
    previous: &ConsistInspection,
    current: &ConsistInspection,
    alpha: f64,
) -> Result<ConsistInspection> {
    if !alpha.is_finite()
        || !(0. ..=1.).contains(&alpha)
        || previous.components.len() != current.components.len()
        || previous.joints.len() != current.joints.len()
    {
        return Err("incompatible presentation snapshots or interpolation fraction".into());
    }
    let mut result = current.clone();
    for ((a, b), p) in previous
        .components
        .iter()
        .zip(&current.components)
        .zip(&mut result.components)
    {
        if a.id != b.id {
            return Err("presentation component identity changed".into());
        }
        p.pose.position_m = a.pose.position_m * (1. - alpha) + b.pose.position_m * alpha;
        p.pose.orientation = a
            .pose
            .orientation
            .slerp(b.pose.orientation, alpha)
            .map_err(|e| e.to_string())?;
        p.station_m = a.station_m * (1. - alpha) + b.station_m * alpha;
    }
    for ((a, b), j) in previous
        .joints
        .iter()
        .zip(&current.joints)
        .zip(&mut result.joints)
    {
        if a.id != b.id {
            return Err("presentation joint identity changed".into());
        }
        j.anchor_a_m = a.anchor_a_m * (1. - alpha) + b.anchor_a_m * alpha;
        j.anchor_b_m = a.anchor_b_m * (1. - alpha) + b.anchor_b_m * alpha;
        j.error_m = (j.anchor_a_m - j.anchor_b_m).length();
    }
    Ok(result)
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
/// Assemble arbitrary component IDs and physical connections on an open track.
/// Track offsets/mounts seed the bounded local solve; rigid anchors and explicit
/// guide references determine closure. Incompatible graphs return an error rather
/// than changing dimensions or displaying a disconnected train.
pub fn inspect(train: &Train, track: &GeneratedTrack, station_m: f64) -> Result<ConsistInspection> {
    let seed = seed(train, track, station_m)?;
    crate::assembly::assemble(train, track, seed)
}

/// Opt-in developer observations of the identical assembly solve. The supplied
/// report is reset for each call and retained on errors, including seed failures.
/// Nothing is written to the train, track, playback state or persisted schema.
pub fn inspect_with_diagnostics(
    train: &Train,
    track: &GeneratedTrack,
    station_m: f64,
    diagnostics: &mut crate::assembly_diagnostics::AssemblyDiagnostics,
) -> Result<ConsistInspection> {
    use crate::assembly_diagnostics::{AssemblyPhase, AssemblyTermination};
    *diagnostics = Default::default();
    let result = seed(train, track, station_m).and_then(|seed| {
        crate::assembly::assemble_with_diagnostics(train, track, seed, diagnostics)
    });
    match &result {
        Ok(_) => {
            diagnostics.phase = AssemblyPhase::Complete;
            diagnostics.termination = AssemblyTermination::Converged;
        }
        Err(e) => {
            if diagnostics.termination != AssemblyTermination::SweepLimit {
                diagnostics.termination = AssemblyTermination::Aborted;
            }
            diagnostics.error = Some(e.clone());
        }
    }
    result
}

/// Unclosed placement for explicitly labeled development diagnostics only.
/// Never use this result to initialize physics or attach a ride camera.
pub fn diagnostic_seed(
    train: &Train,
    track: &GeneratedTrack,
    station_m: f64,
) -> Result<ConsistInspection> {
    seed(train, track, station_m)
}

fn seed(train: &Train, track: &GeneratedTrack, station_m: f64) -> Result<ConsistInspection> {
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
) -> Result<(
    World,
    Vec<TrackGuide>,
    Vec<crate::TrackAttitude>,
    ConsistInspection,
)> {
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
    let stations: Vec<_> = inspection.components.iter().map(|p| p.station_m).collect();
    let mut attitudes = rail_attitudes(train, &stations);
    for a in &mut attitudes {
        a.local_point_m = a.local_point_m - train.components[a.body].center_of_mass_m;
    }
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
    Ok((world, guides, attitudes, inspection))
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

/// Shared by initial assembly (component origins) and dynamics (mass centers).
/// Rail guidance belongs to running gear, including suspended configurations;
/// passenger bodies receive reactions only through their authored joints.
pub(crate) fn rail_attitudes(train: &Train, stations: &[f64]) -> Vec<crate::TrackAttitude> {
    train
        .components
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            if !c.kind.is_running_gear() || c.guide_references_m.is_empty() {
                return None;
            }
            let local_point_m = c
                .guide_references_m
                .iter()
                .copied()
                .fold(Vec3::ZERO, |a, b| a + b)
                / c.guide_references_m.len() as f64;
            let local_frame = match &c.placement {
                Some(Placement::Track { frame, .. }) => frame.rotation.conjugate(),
                _ => qx_geometry::math::Quaternion::IDENTITY,
            };
            Some(crate::TrackAttitude {
                body: i,
                station_m: stations[i],
                local_frame,
                local_point_m,
            })
        })
        .collect()
}
