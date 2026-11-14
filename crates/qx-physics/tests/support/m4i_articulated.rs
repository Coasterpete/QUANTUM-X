//! Articulated controls: reconstruct energy, without applying a single-body
//! trajectory/reaction formula to a coupled train with different masses/inertia.
use super::*;
use analytic::Case;
use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::math::Frame;
use qx_physics::{Body, TrackSimulation};
use std::sync::Arc;

fn bodies(s: &original_dynamics::State) -> Vec<Body> {
    s.poses
        .iter()
        .zip(s.velocity.as_chunks::<6>().0)
        .map(|(p, v)| Body {
            mass: qx_physics::MassProperties {
                mass_kg: 100.,
                principal_inertia_kg_m2: Vec3::new(20., 20., 20.),
            },
            pose: *p,
            linear_velocity_mps: Vec3::new(v[0], v[1], v[2]),
            angular_velocity_radps: Vec3::new(v[3], v[4], v[5]),
            force_n: Vec3::ZERO,
            torque_nm: Vec3::ZERO,
        })
        .collect()
}
fn emit(case: Case, engine: &str, tick: usize, bodies: &[Body], outside: bool) {
    let mut energy = 0.;
    let raw: Vec<_> = bodies
        .iter()
        .map(|b| {
            let p = b.pose.position_m;
            let q = b.pose.orientation;
            let v = b.linear_velocity_mps;
            let w = b.angular_velocity_radps;
            let local = Frame::new(q).unwrap().world_to_local(w);
            let i = b.mass.principal_inertia_kg_m2;
            let m = b.mass.mass_kg;
            energy += 0.5 * m * v.dot(v)
                + 0.5
                    * (i.x * local.x * local.x + i.y * local.y * local.y + i.z * local.z * local.z)
                + m * 9.81 * p.y;
            [
                p.x, p.y, p.z, q.w, q.x, q.y, q.z, v.x, v.y, v.z, w.x, w.y, w.z, m, i.x, i.y, i.z,
            ]
        })
        .collect();
    println!(
        "M4I_ARTICULATED {{\"case\":\"{}\",\"engine\":\"{engine}\",\"tick\":{tick},\"energy\":{energy},\"outside_quality_gate\":{outside},\"bodies\":{raw:?}}}",
        case.name()
    );
}
#[test]
#[ignore = "native Release four-second original production and B1 articulated energy controls"]
fn articulated_energy_and_initial_velocity_controls() {
    for case in [Case::Incline, Case::Circle, Case::Banked] {
        let (i, s) = references::authored(case);
        let track =
            Arc::new(qx_geometry::solver::generate(&i, &s, GenerationOptions::default()).unwrap());
        let r = rail(&i, &s);
        r.reuse.set(true);
        let train = Train::development_consist(
            "M4I articulated control".into(),
            1,
            DevelopmentArrangement::Conventional,
        )
        .unwrap();
        let (_, _, attitudes, _) =
            qx_physics::consist::initialize(&train, &track, 15., G, 32, 4).unwrap();
        let mut p = TrackSimulation::new(train, track.clone(), 15., case.speed()).unwrap();
        let initial = p.world().clone();
        let residual = qx_physics::assembly_diagnostics::velocity_residuals(
            p.world(),
            &track,
            p.guides(),
            &attitudes,
        )
        .unwrap();
        println!(
            "M4I_PRODUCTION_START {{\"case\":\"{}\",\"bodies\":{},\"anchor_speed\":{},\"guide_speed\":{},\"angular_speed\":{},\"twist_speed\":{}}}",
            case.name(),
            initial.bodies.len(),
            residual.max_joint_anchor_speed_mps,
            residual.max_guide_transverse_speed_mps,
            residual.max_angular_constraint_speed_radps,
            residual.max_rail_twist_speed_radps
        );
        emit(case, "production", 0, &p.world().bodies, false);
        for tick in 1..=960 {
            p.step().unwrap();
            emit(case, "production", tick, &p.world().bodies, false);
        }
        p.reset();
        assert_eq!(p.world(), &initial);
        p.step().unwrap();
        let first = p.world().clone();
        p.reset();
        p.step().unwrap();
        assert_eq!(p.world(), &first);
        let mut b =
            measured_tracking::Tracked::new(model(&r, 15., false), &r, case.speed()).unwrap();
        emit(case, "b1", 0, &bodies(&b.state), false);
        for tick in 1..=960 {
            r.begin();
            let report = b.step(&r, G).unwrap();
            // This long reference intentionally exports ungated observations.
            // Flag every original quality-gate violation; do not qualify it.
            emit(
                case,
                "b1",
                tick,
                &bodies(&b.state),
                !residuals::acceptable(&report),
            );
        }
    }
}
