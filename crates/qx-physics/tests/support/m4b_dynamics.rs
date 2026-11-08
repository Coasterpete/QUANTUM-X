//! Standalone rigid-body impulse experiment. Never installs a production World.
use super::{
    contract::{Kind, Model, correction, kinetic},
    matrix::{max_abs, multiply},
    surface::{Failure, Rail, Result},
};
use qx_geometry::math::Vec3;
use qx_physics::{FIXED_STEP_SECONDS, Pose};

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub poses: Vec<Pose>,
    pub velocity: Vec<f64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub loads: Vec<f64>,
    pub max_position: f64,
    pub max_velocity: f64,
    pub energy: f64,
    pub linear_balance: f64,
}
pub fn energy(m: &Model, p: &[Pose], v: &[f64], gravity: Vec3) -> f64 {
    kinetic(v, &m.inverse_mass())
        - p.iter()
            .map(|p| 100. * gravity.dot(p.position_m))
            .sum::<f64>()
}
pub fn initial(m: &Model, r: &Rail, speed: f64) -> Result<(State, f64)> {
    let (p, _) = m.initialize(r, Kind::B1)?;
    let e = m.evaluate(r, &p, Kind::B1)?;
    // Project a physically declared +lead-station motion onto the actual
    // nullspace, then normalize by the front carrier tangent speed.
    let surface = m.supports[0].surface;
    let t = r.jet(surface, m.axles[0].1)?.frame.tangent();
    let mut v = vec![0.; 6 * p.len()];
    for body in v.as_chunks_mut::<6>().0 {
        body[0] = t.x;
        body[1] = t.y;
        body[2] = t.z;
    }
    let residual = multiply(&e.j, &v);
    let (dv, impulse) = correction(
        &e.j,
        &m.inverse_mass(),
        &residual.iter().map(|x| -x).collect::<Vec<_>>(),
    )?;
    for (v, d) in v.iter_mut().zip(dv) {
        *v += d;
    }
    let k = m.axles[0].0 * 6;
    let along = Vec3::new(v[k], v[k + 1], v[k + 2]).dot(t);
    if along.abs() < 1e-5 {
        return Err(Failure::Singular);
    }
    for v in &mut v {
        *v *= speed / along;
    }
    if max_abs(&multiply(&e.j, &v)) > 1e-9 {
        return Err(Failure::Residual);
    }
    // This is a discarded construction velocity, NOT an applied seating impulse.
    Ok((
        State {
            poses: p,
            velocity: v,
        },
        max_abs(&impulse) * speed.abs(),
    ))
}
pub fn instantaneous(
    m: &Model,
    r: &Rail,
    state: &State,
    gravity: Vec3,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let e = m.evaluate(r, &state.poses, Kind::B1)?;
    let gamma = m.curvature(r, &state.poses, &state.velocity, Kind::B1, 1e-5)?;
    let mut free = vec![0.; state.velocity.len()];
    for v in free.as_chunks_mut::<6>().0 {
        v[0] = gravity.x;
        v[1] = gravity.y;
        v[2] = gravity.z;
    }
    let jf = multiply(&e.j, &free);
    let (a, loads) = correction(
        &e.j,
        &m.inverse_mass(),
        &jf.iter()
            .zip(&gamma)
            .map(|(j, g)| -j - g)
            .collect::<Vec<_>>(),
    )?;
    let a: Vec<_> = a.iter().zip(free).map(|(a, f)| a + f).collect();
    let residual: Vec<_> = multiply(&e.j, &a)
        .iter()
        .zip(gamma)
        .map(|(a, g)| a + g)
        .collect();
    if max_abs(&residual) > 1e-7 {
        return Err(Failure::Residual);
    }
    Ok((a, loads))
}
pub fn step(m: &Model, r: &Rail, state: &mut State, gravity: Vec3) -> Result<Step> {
    // Entire outer step, including any target loss or residual gate, rolls back.
    let mut next = state.clone();
    if next.velocity.len() != 6 * next.poses.len()
        || next.velocity.iter().any(|x| !x.is_finite())
        || !gravity.is_finite()
    {
        return Err(Failure::Invalid);
    }
    let dt = FIXED_STEP_SECONDS / 4.;
    let mut accumulated = vec![];
    let inv = m.inverse_mass();
    let mut max_position = 0_f64;
    let mut max_velocity = 0_f64;
    let before = momentum(&state.velocity);
    let mut external = Vec3::ZERO;
    for _ in 0..4 {
        let e = m.evaluate(r, &next.poses, Kind::B1)?;
        let gamma = m.curvature(r, &next.poses, &next.velocity, Kind::B1, 1e-5)?;
        for v in next.velocity.as_chunks_mut::<6>().0 {
            v[0] += gravity.x * dt;
            v[1] += gravity.y * dt;
            v[2] += gravity.z * dt;
        }
        if accumulated.is_empty() {
            accumulated = vec![0.; e.c.len()];
        }
        let mut impulse = vec![0.; e.c.len()];
        // Exactly 32 sequential impulse sweeps, four substeps, 1/240 outer dt.
        // Isotropic 20 kg m² inertia: gyro torque is exactly zero for this fixture.
        for _ in 0..32 {
            for (i, row) in e.j.iter().enumerate() {
                let speed: f64 = row.iter().zip(&next.velocity).map(|(a, b)| a * b).sum();
                let mass: f64 = row.iter().zip(&inv).map(|(a, m)| a * a * m).sum();
                if mass < 1e-12 {
                    return Err(Failure::Singular);
                }
                let lambda = -(speed + dt * gamma[i] + 0.2 * e.c[i] / dt) / mass;
                impulse[i] += lambda;
                for ((v, j), im) in next.velocity.iter_mut().zip(row).zip(&inv) {
                    *v += im * j * lambda;
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
        let displacement: Vec<_> = next.velocity.iter().map(|v| v * dt).collect();
        next.poses = m.shifted(&next.poses, &displacement);
        let final_e = m.evaluate(r, &next.poses, Kind::B1)?;
        max_position = max_position.max(max_abs(&final_e.c));
        max_velocity = max_velocity.max(max_abs(&multiply(&final_e.j, &next.velocity)));
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
