//! M4D TEST-ONLY internal-step refinement. Same equations and 32 sweeps,
//! stabilization, mass/inertia and acceptance gates. No production integration.
use super::{
    contract::{Kind, Model},
    matrix::{max_abs, multiply},
    original_dynamics::{State, Step, energy},
    surface::{Failure, Rail, Result},
};
use qx_geometry::math::Vec3;
use qx_physics::FIXED_STEP_SECONDS;
use std::time::{Duration, Instant};
#[derive(Debug, Default)]
pub struct Profile {
    pub evaluate: Duration,
    pub curvature: Duration,
    pub impulses: Duration,
    pub advance_and_validate: Duration,
}
pub fn step(m: &Model, r: &Rail, s: &mut State, g: Vec3, substeps: usize) -> Result<Step> {
    if ![4, 16, 64].contains(&substeps) {
        return Err(Failure::Invalid);
    }
    run(m, r, s, g, true, None, substeps)
}
pub fn profiled(
    m: &Model,
    r: &Rail,
    s: &mut State,
    g: Vec3,
    sparse: bool,
) -> Result<(Step, Profile)> {
    let mut p = Profile::default();
    let report = run(m, r, s, g, sparse, Some(&mut p), 4)?;
    Ok((report, p))
}
fn run(
    m: &Model,
    r: &Rail,
    state: &mut State,
    gravity: Vec3,
    sparse: bool,
    mut profile: Option<&mut Profile>,
    substeps: usize,
) -> Result<Step> {
    // Entire outer step, including any target loss or residual gate, rolls back.
    let mut next = state.clone();
    if next.velocity.len() != 6 * next.poses.len()
        || next.velocity.iter().any(|x| !x.is_finite())
        || !gravity.is_finite()
    {
        return Err(Failure::Invalid);
    }
    let dt = FIXED_STEP_SECONDS / substeps as f64;
    let mut accumulated = vec![];
    let inv = m.inverse_mass();
    let mut max_position = 0_f64;
    let mut max_velocity = 0_f64;
    let before = momentum(&state.velocity);
    let mut external = Vec3::ZERO;
    for _ in 0..substeps {
        let clock = profile.as_ref().map(|_| Instant::now());
        let e = m.evaluate(r, &next.poses, Kind::B1)?;
        if let Some(p) = profile.as_mut() {
            p.evaluate += clock.unwrap().elapsed();
        }
        let clock = profile.as_ref().map(|_| Instant::now());
        let gamma = m.curvature(r, &next.poses, &next.velocity, Kind::B1, 1e-5)?;
        if let Some(p) = profile.as_mut() {
            p.curvature += clock.unwrap().elapsed();
        }
        let clock = profile.as_ref().map(|_| Instant::now());
        for v in next.velocity.as_chunks_mut::<6>().0 {
            v[0] += gravity.x * dt;
            v[1] += gravity.y * dt;
            v[2] += gravity.z * dt;
        }
        if accumulated.is_empty() {
            accumulated = vec![0.; e.c.len()];
        }
        let mut impulse = vec![0.; e.c.len()];
        // Exactly 32 sweeps per internal step; outer dt stays 1/240 s.
        // Isotropic 20 kg m² inertia: gyro torque is exactly zero for this fixture.
        // Cache invariant row mass and nonzero columns once per substep.
        let rows: Vec<_> = if sparse {
            e.j.iter()
                .map(|row| {
                    let entries: Vec<_> = row
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(_, j)| *j != 0.)
                        .collect();
                    let mass: f64 = entries.iter().map(|&(k, j)| j * j * inv[k]).sum();
                    (entries, mass)
                })
                .collect()
        } else {
            vec![]
        };
        for _ in 0..32 {
            for (i, row) in e.j.iter().enumerate() {
                let (speed, mass) = if sparse {
                    (
                        rows[i]
                            .0
                            .iter()
                            .map(|&(k, j)| j * next.velocity[k])
                            .sum::<f64>(),
                        rows[i].1,
                    )
                } else {
                    (
                        row.iter()
                            .zip(&next.velocity)
                            .map(|(a, b)| a * b)
                            .sum::<f64>(),
                        row.iter().zip(&inv).map(|(a, m)| a * a * m).sum::<f64>(),
                    )
                };
                if mass < 1e-12 {
                    return Err(Failure::Singular);
                }
                let lambda = -(speed + dt * gamma[i] + 0.2 * e.c[i] / dt) / mass;
                impulse[i] += lambda;
                if sparse {
                    for &(k, j) in &rows[i].0 {
                        next.velocity[k] += inv[k] * j * lambda;
                    }
                } else {
                    for ((v, j), im) in next.velocity.iter_mut().zip(row).zip(&inv) {
                        *v += im * j * lambda;
                    }
                }
            }
        }
        for (sum, i) in accumulated.iter_mut().zip(&impulse) {
            *sum += i;
        }
        for (row, lambda) in e.j[..e.rail_rows].iter().zip(&impulse) {
            for body in row.as_chunks::<6>().0 {
                external = external + Vec3::new(body[0], body[1], body[2]) * *lambda;
            }
        }
        if let Some(p) = profile.as_mut() {
            p.impulses += clock.unwrap().elapsed();
        }
        let clock = profile.as_ref().map(|_| Instant::now());
        let displacement: Vec<_> = next.velocity.iter().map(|v| v * dt).collect();
        next.poses = m.shifted(&next.poses, &displacement);
        let final_e = m.evaluate(r, &next.poses, Kind::B1)?;
        max_position = max_position.max(max_abs(&final_e.c));
        max_velocity = max_velocity.max(max_abs(&multiply(&final_e.j, &next.velocity)));
        if let Some(p) = profile.as_mut() {
            p.advance_and_validate += clock.unwrap().elapsed();
        }
        if max_position > 5e-4 || max_velocity > 0.1 || max_abs(&next.velocity) > 100. {
            return Err(Failure::Residual);
        }
    }
    let balance = (momentum(&next.velocity)
        - before
        - external
        - gravity * (100. * next.poses.len() as f64 * FIXED_STEP_SECONDS))
        .length()
        / FIXED_STEP_SECONDS;
    let report = Step {
        loads: accumulated.iter().map(|l| l / FIXED_STEP_SECONDS).collect(),
        max_position,
        max_velocity,
        energy: energy(m, &next.poses, &next.velocity, gravity),
        linear_balance: balance,
    };
    if !report.energy.is_finite() || balance > 1e-6 {
        return Err(Failure::Residual);
    }
    *state = next;
    Ok(report)
}
fn momentum(v: &[f64]) -> Vec3 {
    v.as_chunks::<6>()
        .0
        .iter()
        .fold(Vec3::ZERO, |a, b| a + Vec3::new(b[0], b[1], b[2]) * 100.)
}
