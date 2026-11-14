//! On-demand recorded-state export only; no app/rendering/playback integration.
use super::*;
use qx_geometry::math::Frame;
use std::time::Instant;

#[test]
#[ignore = "development-only actual-pose trace export; inspect with tools/render-source-guidance-m4f.py"]
fn export_actual_shared_bogie_poses() {
    let (_, initial, sections) = fixtures::fixtures().remove(2);
    let r = rail(&initial, &sections);
    let lead = 8.025;
    let labels = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        true,
    );
    let mut t = tracking::Tracked::new(model(&r, lead, true), &r, 6.).unwrap();
    let roles: Vec<_> = (0..t.state.poses.len())
        .map(|i| {
            if labels.cars.contains(&i) {
                "Car"
            } else if labels.bogies.contains(&i) {
                "Bogie"
            } else {
                "Carrier"
            }
        })
        .collect();
    let track: Vec<_> = (0..=240)
        .map(|i| {
            let s = r.length() * i as f64 / 240.;
            [-0.6, 0., 0.6].map(|z| {
                array(
                    r.jet(
                        Surface {
                            rail: "development-datum",
                            face: "running",
                            offset: Vec3::new(0., 0., z),
                            role: Role::Running,
                        },
                        s,
                    )
                    .unwrap()
                    .p,
                )
            })
        })
        .collect();
    let running: Vec<_> = t
        .model
        .supports
        .iter()
        .map(|s| s.surface.role == Role::Running)
        .collect();
    let edges: Vec<_> = t.model.bearings.iter().map(|b| [b.a, b.b]).collect();
    println!(
        "M4F_SCENE {{\"title\":\"R8 / .04 twist source - shared bogie\",\"dt\":{},\"roles\":{roles:?},\"running\":{running:?},\"edges\":{edges:?},\"track\":{track:?}}}",
        qx_physics::FIXED_STEP_SECONDS
    );
    let gravity = Vec3::new(0., -9.81, 0.);
    let initial_energy =
        original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, gravity);
    let mut solver_ms = 0_f64;
    let mut diagnostics_ms = 0_f64;
    let mut combined_ms = 0_f64;
    for tick in 0..=24 {
        let mut current_solver_ms = 0.;
        if tick > 0 {
            let clock = Instant::now();
            let report = t.step(&r, gravity).unwrap();
            current_solver_ms = clock.elapsed().as_secs_f64() * 1000.;
            solver_ms = solver_ms.max(current_solver_ms);
            assert!(report.max_position < 2e-5 && report.max_velocity < 0.003);
        }
        let clock = Instant::now();
        let time = tick as f64 * qx_physics::FIXED_STEP_SECONDS;
        let evaluation = t.model.evaluate(&r, &t.state.poses, Kind::B1).unwrap();
        let energy =
            original_dynamics::energy(&t.model, &t.state.poses, &t.state.velocity, gravity)
                - initial_energy;
        let mut bodies = Vec::new();
        for (i, p) in t.state.poses.iter().enumerate() {
            let center = Surface {
                rail: "center-measurement",
                face: "development-normal",
                offset: Vec3::ZERO,
                role: Role::Running,
            };
            let reference = r
                .project(
                    center,
                    p.position_m,
                    labels.poses[i].position_m.x + 6. * time,
                    1.0,
                )
                .unwrap()
                .1
                .frame;
            let angle = reference
                .orientation()
                .conjugate()
                .compose(p.orientation)
                .rotation_vector()
                .unwrap()
                .length()
                .to_degrees();
            let q = p.orientation;
            bodies.push([
                p.position_m.x,
                p.position_m.y,
                p.position_m.z,
                q.w,
                q.x,
                q.y,
                q.z,
                angle,
            ]);
        }
        let mut supports = Vec::new();
        for s in &t.model.supports {
            let p = t.state.poses[s.owner];
            let x = guidance::point(p, s.local);
            let (station, j) = r.project(s.surface, x, s.station, s.half_window).unwrap();
            let tangent = j.d / j.d.length();
            let angle = Frame::new(p.orientation)
                .unwrap()
                .tangent()
                .dot(tangent)
                .clamp(-1., 1.)
                .acos()
                .to_degrees();
            supports.push([
                s.owner as f64,
                x.x,
                x.y,
                x.z,
                j.p.x,
                j.p.y,
                j.p.z,
                j.n.x,
                j.n.y,
                j.n.z,
                station,
                (x - j.p).dot(j.n),
                angle,
            ]);
        }
        let anchors: Vec<_> = t
            .model
            .bearings
            .iter()
            .map(|b| {
                let a = guidance::point(t.state.poses[b.a], b.local_a);
                let c = guidance::point(t.state.poses[b.b], b.local_b);
                [a.x, a.y, a.z, c.x, c.y, c.z]
            })
            .collect();
        let bearing_gap = anchors
            .iter()
            .map(|a| (Vec3::new(a[0], a[1], a[2]) - Vec3::new(a[3], a[4], a[5])).length())
            .fold(0., f64::max);
        let position = max_abs(&evaluation.c);
        let velocity = max_abs(&multiply(&evaluation.j, &t.state.velocity));
        println!(
            "M4F_FRAME {{\"tick\":{tick},\"time\":{time},\"bodies\":{bodies:?},\"supports\":{supports:?},\"anchors\":{anchors:?},\"max_position\":{position},\"max_velocity\":{velocity},\"energy_drift\":{energy},\"bearing_gap\":{bearing_gap}}}"
        );
        let current_diagnostics_ms = clock.elapsed().as_secs_f64() * 1000.;
        diagnostics_ms = diagnostics_ms.max(current_diagnostics_ms);
        combined_ms = combined_ms.max(current_solver_ms + current_diagnostics_ms);
    }
    println!(
        "M4F_VIEW_COST {{\"max_solver_ms\":{solver_ms},\"max_diagnostics_ms\":{diagnostics_ms},\"max_combined_ms\":{combined_ms}}}"
    );
}
