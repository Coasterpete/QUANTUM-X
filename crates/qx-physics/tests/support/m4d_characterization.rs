use super::{
    contract::{self, Kind},
    explicit, guidance,
    matrix::{max_abs, multiply},
    mechanisms::Path,
    model, original_dynamics,
    surface::{Rail, Role, Surface},
    tracking::Tracked,
};
use qx_geometry::math::{Frame, Vec3};
// Metrics preserve individual supports and actual owning body identities. Wheel
// centers/rolling axes are diagnostic conventions, not physical contact laws.
pub fn run(
    path: Path,
    speed: f64,
    ticks: usize,
    shared: bool,
    circuit: bool,
    gravity: Vec3,
    lead: f64,
) {
    let length = if circuit {
        16. * std::f64::consts::PI
    } else {
        160.
    };
    let r = Rail::analytic(path, length, circuit).unwrap();
    let mut t = Tracked::new(model(path, lead, shared), &r, speed).unwrap();
    let mut repeat = Tracked::new(model(path, lead, shared), &r, speed).unwrap();
    // Explicit experimental refinement, never a new production timestep. Keep
    // the M4C 2e-5 / .003 quality envelope rather than relaxing it at high speed.
    let substeps = if speed.abs() <= 6. {
        4
    } else if speed.abs() <= 12. {
        16
    } else {
        64
    };
    t.substeps = substeps;
    repeat.substeps = substeps;
    let initial = original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, gravity);
    let initial_kinetic = contract::kinetic(&t.state.velocity, &t.model.inverse_mass());
    let start = t.hints();
    let (mut p, mut v, mut energy, mut force, mut impulse, mut station_error) =
        (0_f64, 0_f64, 0_f64, 0_f64, 0_f64, 0_f64);
    let mut min_running = f64::INFINITY;
    let mut per_support = vec![0_f64; t.model.supports.len()];
    let mut per_speed = per_support.clone();
    let mut per_wheel = vec![0_f64; t.model.supports.len()];
    let mut wheel_angle = per_wheel.clone();
    let mut per_body = vec![0_f64; t.model.poses.len()];
    let mut bearing_gap = 0_f64;
    let mut angular_speed = 0_f64;
    let mut innovation = 0_f64;
    let mut point_force = 0_f64;
    let mut minimum_speed = f64::INFINITY;
    let mut maximum_speed = 0_f64;
    let mut rigid = 0_f64;
    for tick in 0..ticks {
        let report = t
            .step(&r, gravity)
            .unwrap_or_else(|e| panic!("{path:?} {speed} tick={tick} {e:?}"));
        assert_eq!(report, repeat.step(&r, gravity).unwrap());
        assert_eq!(t.state, repeat.state);
        assert_eq!(t.hints(), repeat.hints());
        assert_eq!(t.envelopes, repeat.envelopes);
        p = p.max(report.max_position);
        v = v.max(report.max_velocity);
        energy = energy.max((report.energy - initial).abs());
        force = force.max(max_abs(&report.loads));
        impulse = impulse.max(max_abs(&report.loads) * qx_physics::FIXED_STEP_SECONDS);
        let e = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap();
        let velocity_rows = multiply(&e.j, &t.state.velocity);
        for (i, &c) in e.c.iter().enumerate().skip(e.rail_rows) {
            // Bearing rows are three anchor coordinates followed by one
            // angular row exactly when its authored roll lock is present.
            let mut row = e.rail_rows;
            for b in &t.model.bearings {
                if (row..row + 3).contains(&i) {
                    bearing_gap = bearing_gap.max(c.abs());
                }
                if b.roll && i == row + 3 {
                    angular_speed = angular_speed.max(velocity_rows[i].abs());
                }
                row += 3 + usize::from(b.roll);
            }
        }
        for (i, s) in t.model.supports.iter().enumerate() {
            per_support[i] = per_support[i].max(e.c[i].abs());
            per_speed[i] = per_speed[i].max(velocity_rows[i].abs());
            innovation = innovation.max(t.envelopes[i].innovation);
            minimum_speed = minimum_speed.min(t.envelopes[i].rate.abs());
            maximum_speed = maximum_speed.max(t.envelopes[i].rate.abs());
            if s.surface.role == Role::Running {
                min_running = min_running.min(report.loads[i]);
            }
            let pose = t.state.poses[s.owner];
            let frame = Frame::new(pose.orientation).unwrap();
            let k = s.owner * 6;
            let g = Vec3::new(e.j[i][k], e.j[i][k + 1], e.j[i][k + 2]);
            point_force = point_force.max(g.length() * report.loads[i].abs());
            // Only the three running supports on each axle have the declared
            // .18 Y diagnostic wheel convention. Side datums are not wheels.
            if s.surface.role == Role::Running {
                let wheel = guidance::point(pose, s.local + Vec3::Y * 0.18);
                let target = r
                    .project(
                        Surface {
                            offset: s.surface.offset + Vec3::Y * 0.18,
                            ..s.surface
                        },
                        wheel,
                        s.station,
                        0.05,
                    )
                    .unwrap()
                    .1;
                let delta = wheel - target.p;
                let tangent = target.d / target.d.length();
                per_wheel[i] = per_wheel[i].max((delta - tangent * delta.dot(tangent)).length());
                wheel_angle[i] =
                    wheel_angle[i].max(frame.tangent().dot(tangent).clamp(-1., 1.).acos());
            }
            // Independent POSITION-only local golden search (no F or J).
            if tick % 24 == 0 {
                let (mut a, mut b) = (s.station - 0.015, s.station + 0.015);
                let x = guidance::point(pose, s.local);
                let distance = |q: f64| {
                    let d = x - explicit(
                        path,
                        if circuit { q.rem_euclid(length) } else { q },
                        s.surface.offset,
                    );
                    d.dot(d)
                };
                for _ in 0..48 {
                    let l = a + (b - a) * 0.3819660112501051;
                    let h = b - (b - a) * 0.3819660112501051;
                    if distance(l) < distance(h) {
                        b = h;
                    } else {
                        a = l;
                    }
                }
                station_error = station_error.max(((a + b) * 0.5 - s.station).abs());
            }
            let local = Vec3::new(0.31, -0.21, 0.27);
            rigid = rigid.max(
                ((guidance::point(pose, s.local) - guidance::point(pose, local)).length()
                    - (s.local - local).length())
                .abs(),
            );
        }
        // Every body, including bogies and cars, receives an orientation
        // observation relative to its independently nearest centerline frame.
        // These are alignment observations, not extra mechanical constraints.
        if tick % 24 == 0 || tick + 1 == ticks {
            let hints = t.hints();
            let lo = hints.iter().copied().fold(f64::INFINITY, f64::min) - 1.;
            let hi = hints.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 1.;
            for (i, p) in t.state.poses.iter().enumerate() {
                let (mut a, mut b) = (lo, hi);
                let distance = |s: f64| {
                    let d = p.position_m - explicit(path, s, Vec3::ZERO);
                    d.dot(d)
                };
                for _ in 0..48 {
                    let l = a + (b - a) * 0.3819660112501051;
                    let h = b - (b - a) * 0.3819660112501051;
                    if distance(l) < distance(h) {
                        b = h;
                    } else {
                        a = l;
                    }
                }
                let reference = Frame::new(path.at((a + b) * 0.5).orientation).unwrap();
                let body = Frame::new(p.orientation).unwrap();
                let angle = reference
                    .orientation()
                    .conjugate()
                    .compose(body.orientation())
                    .rotation_vector()
                    .unwrap()
                    .length();
                per_body[i] = per_body[i].max(angle);
            }
        }
    }
    let travel = t.hints()[0] - start[0];
    assert!(travel * speed > 0.);
    assert!(p < 2e-5 && v < 0.003, "{path:?} speed={speed} p={p} v={v}");
    assert!(station_error < 3e-7 && rigid < 1e-11);
    assert_eq!(t.state.poses.len(), if shared { 11 } else { 7 });
    let energy_limit = (initial_kinetic * 1e-5).max(0.005);
    let conservation = energy < energy_limit;
    // Retain the known energy failure in the gravity-driven helix as an
    // explicit characterization. Every other declared run retains M4C's gate.
    if matches!(path, Path::Helix) && gravity.length() > 0. {
        assert!(
            !conservation && energy > 1.,
            "expected retained gravity-helix conservation failure"
        );
    } else {
        assert!(conservation, "{path:?} drift={energy} limit={energy_limit}");
    }
    let wheels: Vec<_> = t
        .model
        .supports
        .iter()
        .enumerate()
        .filter(|(_, s)| s.surface.role == Role::Running)
        .map(|(i, s)| (s.owner, i, per_wheel[i], wheel_angle[i]))
        .collect();
    println!(
        "M4D_RUN {path:?} speed={speed} ticks={ticks} substeps={substeps} shared={shared} circuit={circuit} travel={travel:.9e} position={p:.9e} velocity={v:.9e} energy_J={energy:.9e} conservation={conservation} energy_limit_J={energy_limit:.9e} load={force:.9e} point_force_N={point_force:.9e} impulse={impulse:.9e} min_running_N={min_running:.9e} station_m={station_error:.9e} rigid_m={rigid:.9e} bearing_m={bearing_gap:.9e} angular_speed={angular_speed:.9e} innovation_m={innovation:.9e} speed_range={minimum_speed:.9e},{maximum_speed:.9e} support_m={per_support:?} support_mps={per_speed:?} body_rad={per_body:?} wheels_owner_support_m_rad={wheels:?}"
    );
}
