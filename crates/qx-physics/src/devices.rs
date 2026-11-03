//! Ideal distributed longitudinal actuators at body mass centers, sampled at
//! 240 Hz. No speed assignment, gravity compensation, or invented wheel contact.
use crate::{FIXED_STEP_SECONDS, Result, World};
use qx_document::operations::{DeviceKind, Operations};
use qx_geometry::{math::Vec3, solver::GeneratedTrack};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceTelemetry {
    pub id: u64,
    pub occupied_bodies: usize,
    pub commanded_acceleration_mps2: f64,
    pub requested_force_n: f64,
    pub applied_force_n: f64,
    pub force_limited: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrainTelemetry {
    pub elapsed_s: f64,
    pub speed_mps: f64,
    pub distance_traveled_m: f64,
    pub acceleration_mps2: Vec3,
    pub specific_force_mps2: Vec3,
    /// Mass-weighted body-local specific force (T,N,L), N/kg. This is not
    /// a seat accelerometer or a passenger comfort certification.
    pub local_specific_force_mps2: Vec3,
    pub longitudinal_acceleration_mps2: f64,
    pub applied_device_force_n: f64,
    pub resistance_force_n: f64,
    pub devices: Vec<DeviceTelemetry>,
}
pub(crate) fn forces(
    world: &mut World,
    track: &GeneratedTrack,
    stations: &[f64],
    operations: &Operations,
    circuit: bool,
) -> Result<(Vec<DeviceTelemetry>, f64)> {
    if stations.len() != world.bodies.len() {
        return Err("device/body station mismatch".into());
    }
    let mass: f64 = world.bodies.iter().map(|b| b.mass.mass_kg).sum();
    let mut total = vec![Vec3::ZERO; world.bodies.len()];
    let mut passive = vec![Vec3::ZERO; world.bodies.len()];
    let mut telemetry = vec![];
    for d in &operations.devices {
        let mut t = DeviceTelemetry {
            id: d.id,
            ..Default::default()
        };
        let mut commands = vec![];
        if d.enabled {
            for (i, (body, station)) in world.bodies.iter().zip(stations).enumerate() {
                if let Some(local) = d.local_distance(*station, track.length_m(), circuit) {
                    let tangent = track
                        .sample_at_distance(*station)
                        .map_err(|e| e.to_string())?
                        .frame
                        .tangent();
                    let v = body.linear_velocity_mps.dot(tangent);
                    let magnitude = d.command(local);
                    let command = match d.kind {
                        DeviceKind::Launch => {
                            if d.reverse_launch {
                                -magnitude
                            } else {
                                magnitude
                            }
                        }
                        DeviceKind::Brake => {
                            -v.signum() * magnitude.min(v.abs() / FIXED_STEP_SECONDS)
                        }
                    };
                    let force = body.mass.mass_kg * command;
                    commands.push((i, tangent * force));
                    t.occupied_bodies += 1;
                    t.commanded_acceleration_mps2 += command;
                    t.requested_force_n += force.abs();
                }
            }
        }
        let scale = if t.requested_force_n > 0. {
            (d.maximum_force_n / t.requested_force_n).min(1.)
        } else {
            0.
        };
        t.applied_force_n = t.requested_force_n * scale;
        t.force_limited = scale < 1. && t.requested_force_n > 0.;
        if t.occupied_bodies > 0 {
            t.commanded_acceleration_mps2 /= t.occupied_bodies as f64;
        }
        for (i, f) in commands {
            if d.kind == DeviceKind::Brake {
                passive[i] = passive[i] + f * scale;
            } else {
                total[i] = total[i] + f * scale;
            }
        }
        telemetry.push(t);
    }
    let mut resistance = 0.;
    let mut passive_scale = 1.0_f64;
    for (i, body) in world.bodies.iter().enumerate() {
        let tangent = track
            .sample_at_distance(stations[i])
            .map_err(|e| e.to_string())?
            .frame
            .tangent();
        let v = body.linear_velocity_mps.dot(tangent);
        let p = &operations.physical;
        let drag = (p.rolling_resistance_coefficient * body.mass.mass_kg * p.gravity_mps2
            + 0.5 * p.air_density_kg_m3 * p.drag_area_m2 * (body.mass.mass_kg / mass) * v * v)
            .min(body.mass.mass_kg * v.abs() / FIXED_STEP_SECONDS);
        resistance += drag;
        passive[i] = passive[i] - tangent * (v.signum() * drag);
        // Overlapping passive devices together must not drive through rest.
        let magnitude = passive[i].length();
        if magnitude > 0. {
            passive_scale =
                passive_scale.min(body.mass.mass_kg * v.abs() / FIXED_STEP_SECONDS / magnitude);
        }
    }
    for (i, body) in world.bodies.iter_mut().enumerate() {
        body.force_n = total[i] + passive[i] * passive_scale;
        body.torque_nm = Vec3::ZERO;
    }
    for (d, t) in operations.devices.iter().zip(&mut telemetry) {
        if d.kind == DeviceKind::Brake {
            t.applied_force_n *= passive_scale;
        }
    }
    Ok((telemetry, resistance * passive_scale))
}

pub(crate) fn velocity(world: &World) -> Vec3 {
    let mass: f64 = world.bodies.iter().map(|b| b.mass.mass_kg).sum();
    world.bodies.iter().fold(Vec3::ZERO, |v, b| {
        v + b.linear_velocity_mps * (b.mass.mass_kg / mass)
    })
}
pub(crate) fn mean_speed(world: &World) -> f64 {
    let mass: f64 = world.bodies.iter().map(|b| b.mass.mass_kg).sum();
    world
        .bodies
        .iter()
        .map(|b| b.linear_velocity_mps.length() * b.mass.mass_kg / mass)
        .sum()
}
