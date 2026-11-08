#[path = "support/articulation.rs"]
mod fixtures;

use fixtures::*;
use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    solver::GeneratedTrack,
};
use qx_physics::{
    TrackSimulation, World,
    assembly_diagnostics::velocity_residuals,
    consist::{initialize, snapshot},
};
use std::sync::Arc;

fn independent_anchor_speed(world: &World) -> f64 {
    world
        .joints
        .iter()
        .map(|j| {
            let velocity = |i: usize, local: Vec3| {
                let b = &world.bodies[i];
                let r = Frame::new(b.pose.orientation)
                    .unwrap()
                    .local_to_world(local);
                b.linear_velocity_mps + b.angular_velocity_radps.cross(r)
            };
            (velocity(j.body_b, j.anchor_b_m) - velocity(j.body_a, j.anchor_a_m)).length()
        })
        .fold(0., f64::max)
}

#[test]
fn rest_initialization_is_consistent_but_playback_tangent_seeds_are_not() {
    let flat = Arc::new(straight());
    let curve = Arc::new(arc(45., 0.25, 500., false));
    let twisting = Arc::new(arc(45., 0.25, 500., true));
    for (name, track, train) in [
        ("straight-2", flat, conventional(2)),
        ("curve-2", curve.clone(), conventional(2)),
        ("curve-8", curve, conventional(8)),
        ("twisting-2", twisting.clone(), conventional(2)),
        (
            "rotating-seats",
            twisting,
            Train::development_consist("m1-seat".into(), 2, DevelopmentArrangement::RotatingSeats)
                .unwrap(),
        ),
    ] {
        let (mut world, guides, attitudes, inspection) =
            initialize(&train, &track, 200., Vec3::new(0., -9.81, 0.), 32, 4).unwrap();
        let rest = world.clone();
        let rest_residuals = velocity_residuals(&world, &track, &guides, &attitudes).unwrap();
        assert_eq!(rest_residuals, Default::default());
        assert_eq!(world, rest);
        let converted = snapshot(&train, &world, &guides, &inspection).unwrap();
        for (a, b) in converted.components.iter().zip(&inspection.components) {
            assert!((a.pose.position_m - b.pose.position_m).length() < 1e-12);
            assert_eq!(a.pose.orientation, b.pose.orientation);
        }
        for (body, component) in world.bodies.iter_mut().zip(&inspection.components) {
            body.linear_velocity_mps = track
                .sample_at_distance(component.station_m)
                .unwrap()
                .frame
                .tangent()
                * 12.;
        }
        let before = world.clone();
        let before_guides = guides.clone();
        let before_attitudes = attitudes.clone();
        let moving = velocity_residuals(&world, &track, &guides, &attitudes).unwrap();
        assert_eq!(
            moving,
            velocity_residuals(&world, &track, &guides, &attitudes).unwrap()
        );
        assert_eq!(world, before);
        assert_eq!(guides, before_guides);
        assert_eq!(attitudes, before_attitudes);
        assert!(
            (moving.max_joint_anchor_speed_mps - independent_anchor_speed(&world)).abs() < 1e-14
        );
        assert_eq!(
            world,
            *TrackSimulation::new(train, track, 200., 12.)
                .unwrap()
                .world()
        );
        println!("{name}: rest={rest_residuals:?} moving_12mps={moving:?}");
        if name == "straight-2" {
            assert!(
                moving.max_joint_anchor_speed_mps < 1e-12
                    && moving.max_guide_transverse_speed_mps < 1e-12
            );
        } else {
            assert!(moving.max_joint_anchor_speed_mps > 0.01);
            assert!(moving.max_guide_transverse_speed_mps > 0.01);
        }
        assert_eq!(moving.max_angular_constraint_speed_radps, 0.);
        if name == "twisting-2" || name == "rotating-seats" {
            assert!(moving.max_rail_twist_speed_radps > 0.001);
        }
    }
}

fn shift(world: &World, dt: f64) -> World {
    let mut shifted = world.clone();
    for b in &mut shifted.bodies {
        b.pose.position_m = b.pose.position_m + b.linear_velocity_mps * dt;
        let w = b.angular_velocity_radps;
        if w.length() > 0. {
            b.pose.orientation = Quaternion::axis_angle(w, w.length() * dt)
                .unwrap()
                .compose(b.pose.orientation)
                .normalized()
                .unwrap();
        }
    }
    shifted
}

fn rail_twist(world: &World, track: &GeneratedTrack, a: &qx_physics::TrackAttitude) -> f64 {
    let b = &world.bodies[a.body];
    let point = b.world_point(a.local_point_m).unwrap();
    // Independent converged closest-station measurement for finite differences.
    let mut s = a.station_m;
    for _ in 0..64 {
        let sample = track.sample_at_distance(s).unwrap();
        let ds = (point - sample.position_m).dot(sample.frame.tangent());
        s += ds;
        if ds.abs() < 1e-13 {
            break;
        }
    }
    let target = track.sample_at_distance(s).unwrap().frame.orientation();
    let q = target
        .conjugate()
        .compose(b.pose.orientation.compose(a.local_frame));
    2. * q.x.atan2(q.w)
}

#[test]
fn rail_velocity_jacobian_matches_independent_finite_differences() {
    // Constant spatial twist on a straight centerline supplies an independently
    // exact rail derivative, without the production finite-difference stencil's
    // small mismatch with Hermite-position/slerp-frame interpolation on curves.
    let mut track = straight();
    for s in &mut track.samples {
        s.frame = Frame::new(Quaternion::axis_angle(Vec3::X, 0.25 + 0.002 * s.distance_m).unwrap())
            .unwrap();
    }
    let (mut world, guides, attitudes, _) =
        initialize(&conventional(2), &track, 200., Vec3::ZERO, 32, 4).unwrap();
    for (i, b) in world.bodies.iter_mut().enumerate() {
        b.linear_velocity_mps = Vec3::new(2., -1., 0.7);
        b.angular_velocity_radps = Vec3::new(0.1, 0.3, -0.2) * (i + 1) as f64;
    }
    let d = velocity_residuals(&world, &track, &guides, &attitudes).unwrap();
    let h = 1e-5;
    let plus = shift(&world, h);
    let minus = shift(&world, -h);
    let anchor_derivative = world
        .joints
        .iter()
        .map(|j| {
            let gap = |w: &World| {
                w.bodies[j.body_b].world_point(j.anchor_b_m).unwrap()
                    - w.bodies[j.body_a].world_point(j.anchor_a_m).unwrap()
            };
            ((gap(&plus) - gap(&minus)) / (2. * h)).length()
        })
        .fold(0., f64::max);
    assert!((d.max_joint_anchor_speed_mps - anchor_derivative).abs() < 1e-8);
    let numerical = attitudes
        .iter()
        .map(|a| ((rail_twist(&plus, &track, a) - rail_twist(&minus, &track, a)) / (2. * h)).abs())
        .fold(0., f64::max);
    println!(
        "rail Jacobian: measured_radps={} finite_difference_radps={numerical}",
        d.max_rail_twist_speed_radps
    );
    assert!((d.max_rail_twist_speed_radps - numerical).abs() < 1e-8);

    let mut invalid = world;
    invalid.bodies[0].linear_velocity_mps.x = f64::NAN;
    assert!(velocity_residuals(&invalid, &track, &guides, &attitudes).is_err());
    let mut bad_guide = guides;
    bad_guide[0].body = usize::MAX;
    assert!(velocity_residuals(&plus, &track, &bad_guide, &attitudes).is_err());
}

#[test]
fn angular_velocity_checks_respect_free_hinge_and_active_limit_directions() {
    use qx_document::train::mechanism::JointModel;
    let track = straight();
    let train = Train::development_consist(
        "velocity-hinge".into(),
        1,
        DevelopmentArrangement::RotatingSeats,
    )
    .unwrap();
    let (mut world, guides, attitudes, _) =
        initialize(&train, &track, 200., Vec3::ZERO, 32, 4).unwrap();
    let hinge = world
        .angular_joints
        .iter()
        .find(|j| matches!(j.model, JointModel::Hinge { .. }))
        .unwrap()
        .clone();
    let JointModel::Hinge {
        limits: Some(limits),
        ..
    } = hinge.model
    else {
        panic!("fixture hinge limits");
    };
    world.bodies[hinge.body_b].angular_velocity_radps = Vec3::Z * 2.;
    assert_eq!(
        velocity_residuals(&world, &track, &guides, &attitudes)
            .unwrap()
            .max_angular_constraint_speed_radps,
        0.
    );
    world.bodies[hinge.body_b].angular_velocity_radps = Vec3::X * 1.5;
    assert_eq!(
        velocity_residuals(&world, &track, &guides, &attitudes)
            .unwrap()
            .max_angular_constraint_speed_radps,
        1.5
    );
    // Slightly outside the upper bound avoids an ambiguous floating-point tie.
    world.bodies[hinge.body_b].pose.orientation =
        Quaternion::axis_angle(Vec3::Z, limits.max_rad + 1e-8).unwrap();
    world.bodies[hinge.body_b].angular_velocity_radps = Vec3::Z * 3.;
    assert_eq!(
        velocity_residuals(&world, &track, &guides, &attitudes)
            .unwrap()
            .max_angular_constraint_speed_radps,
        3.
    );
    world.bodies[hinge.body_b].angular_velocity_radps = -Vec3::Z * 3.;
    assert_eq!(
        velocity_residuals(&world, &track, &guides, &attitudes)
            .unwrap()
            .max_angular_constraint_speed_radps,
        0.
    );
    // Finite inputs whose relative anchor velocity overflows must be an error.
    world.bodies[hinge.body_a].linear_velocity_mps = Vec3::X * 1e308;
    world.bodies[hinge.body_b].linear_velocity_mps = -Vec3::X * 1e308;
    assert!(velocity_residuals(&world, &track, &guides, &attitudes).is_err());
}
