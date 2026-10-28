use qx_geometry::math::{Quaternion, Vec3};
use qx_physics::{BallJoint, Body, FIXED_STEP_SECONDS, Guide, MassProperties, Pose, World};

fn body(mass: f64, position: Vec3) -> Body {
    Body {
        mass: MassProperties::uniform_box(mass, Vec3::new(2., 1., 1.)).unwrap(),
        pose: Pose {
            position_m: position,
            orientation: Quaternion::IDENTITY,
        },
        linear_velocity_mps: Vec3::ZERO,
        angular_velocity_radps: Vec3::ZERO,
        force_n: Vec3::ZERO,
        torque_nm: Vec3::ZERO,
    }
}
fn world(bodies: Vec<Body>) -> World {
    World {
        bodies,
        joints: vec![],
        guides: vec![],
        gravity_mps2: Vec3::new(0., -9.81, 0.),
        iterations: 24,
        substeps: 2,
    }
}
fn near(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() < tol,
        "{a} differs from {b} by more than {tol}"
    );
}

#[test]
fn box_inertia_and_invalid_mass() {
    let mass = MassProperties::uniform_box(12., Vec3::new(2., 3., 4.)).unwrap();
    assert_eq!(mass.principal_inertia_kg_m2, Vec3::new(25., 20., 13.));
    assert!(MassProperties::uniform_box(-1., Vec3::X).is_err());
    assert!(
        MassProperties {
            mass_kg: 1.,
            principal_inertia_kg_m2: Vec3::new(1., 1., 3.)
        }
        .validate()
        .is_err()
    );
}

#[test]
fn free_fall_and_torque() {
    let mut w = world(vec![body(2., Vec3::ZERO)]);
    w.bodies[0].torque_nm = Vec3::new(1., 0., 0.);
    for _ in 0..240 {
        w.step().unwrap();
    }
    near(w.bodies[0].linear_velocity_mps.y, -9.81, 1e-11);
    near(w.bodies[0].pose.position_m.y, -4.905, 0.011);
    near(w.bodies[0].angular_velocity_radps.x, 3., 1e-11);
}

#[test]
fn ball_joint_transmits_impulse_and_preserves_momentum() {
    let mut w = world(vec![
        body(2., Vec3::new(-1., 0., 0.)),
        body(3., Vec3::new(1., 0., 0.)),
    ]);
    w.gravity_mps2 = Vec3::ZERO;
    w.joints.push(BallJoint {
        body_a: 0,
        body_b: 1,
        anchor_a_m: Vec3::X,
        anchor_b_m: -Vec3::X,
    });
    w.bodies[0].linear_velocity_mps = Vec3::X;
    let initial_momentum = 2.;
    let mut max_error: f64 = 0.;
    let mut reaction = 0.;
    for _ in 0..240 {
        let d = w.step().unwrap();
        max_error = max_error.max(d.max_joint_error_m);
        reaction += d.joint_force_on_b_n[0].x.abs() * FIXED_STEP_SECONDS;
    }
    near(
        2. * w.bodies[0].linear_velocity_mps.x + 3. * w.bodies[1].linear_velocity_mps.x,
        initial_momentum,
        1e-10,
    );
    near(w.bodies[1].linear_velocity_mps.x, 0.4, 1e-10);
    assert!(reaction > 1.19 && max_error < 1e-8);
}

#[test]
fn connected_bodies_on_inclined_ideal_guides_accelerate_and_are_deterministic() {
    let angle: f64 = 0.3;
    let t = Vec3::new(angle.cos(), -angle.sin(), 0.);
    let mut w = world(vec![body(2., -t), body(3., t)]);
    let q = Quaternion::axis_angle(Vec3::Z, -angle).unwrap();
    for b in &mut w.bodies {
        b.pose.orientation = q;
    }
    w.joints.push(BallJoint {
        body_a: 0,
        body_b: 1,
        anchor_a_m: Vec3::X,
        anchor_b_m: -Vec3::X,
    });
    for index in 0..2 {
        w.guides.push(Guide {
            body: index,
            local_point_m: Vec3::ZERO,
            line_origin_m: Vec3::ZERO,
            line_direction: t,
        });
    }
    let mut replay = w.clone();
    for _ in 0..240 {
        let d = w.step().unwrap();
        assert!(d.max_joint_error_m < 1e-8 && d.max_guide_error_m < 1e-8);
        assert_eq!(d, replay.step().unwrap());
    }
    assert_eq!(w, replay);
    near(
        w.bodies[0].linear_velocity_mps.dot(t),
        9.81 * angle.sin(),
        1e-10,
    );
}

#[test]
fn invalid_step_is_transactional() {
    let mut w = world(vec![body(1., Vec3::ZERO)]);
    w.joints.push(BallJoint {
        body_a: 0,
        body_b: 1,
        anchor_a_m: Vec3::ZERO,
        anchor_b_m: Vec3::ZERO,
    });
    let before = w.clone();
    assert!(w.step().is_err());
    assert_eq!(w, before);
    w.joints.clear();
    w.guides.push(Guide {
        body: 0,
        local_point_m: Vec3::ZERO,
        line_origin_m: Vec3::ZERO,
        line_direction: Vec3::ZERO,
    });
    assert!(w.step().is_err());
}

#[test]
fn numerical_overflow_rolls_back_step() {
    let mut w = world(vec![body(1., Vec3::ZERO)]);
    w.bodies[0].force_n = Vec3::new(f64::MAX, 0., 0.);
    w.bodies[0].mass.mass_kg = 1e-300;
    let before = w.clone();
    assert!(w.step().is_err());
    assert_eq!(w, before);
}

#[test]
fn free_fall_substeps_converge() {
    let mut a = world(vec![body(1., Vec3::ZERO)]);
    a.substeps = 1;
    let mut b = a.clone();
    b.substeps = 8;
    for _ in 0..240 {
        a.step().unwrap();
        b.step().unwrap();
    }
    near(
        a.bodies[0].linear_velocity_mps.y,
        b.bodies[0].linear_velocity_mps.y,
        1e-10,
    );
    assert!(
        (b.bodies[0].pose.position_m.y + 4.905).abs()
            < (a.bodies[0].pose.position_m.y + 4.905).abs()
    );
}

#[test]
fn gravity_energy_error_is_bounded_and_decreases_with_substeps() {
    fn error(substeps: u32) -> f64 {
        let mut w = world(vec![body(2., Vec3::new(0., 10., 0.))]);
        w.substeps = substeps;
        for _ in 0..240 {
            w.step().unwrap();
        }
        let b = &w.bodies[0];
        let energy = 0.5 * b.mass.mass_kg * b.linear_velocity_mps.dot(b.linear_velocity_mps)
            + b.mass.mass_kg * 9.81 * b.pose.position_m.y;
        (energy - 2. * 9.81 * 10.).abs()
    }
    assert!(error(1) < 0.41);
    assert!(error(8) < error(1) / 7.9);
}

#[test]
fn off_center_connection_generates_rotation_and_stays_connected() {
    let mut w = world(vec![
        body(2., Vec3::new(-1., 0., 0.)),
        body(3., Vec3::new(1., 0., 0.)),
    ]);
    w.gravity_mps2 = Vec3::ZERO;
    w.joints.push(BallJoint {
        body_a: 0,
        body_b: 1,
        anchor_a_m: Vec3::X,
        anchor_b_m: -Vec3::X,
    });
    w.bodies[0].linear_velocity_mps = Vec3::Y;
    let mut max_error: f64 = 0.;
    for _ in 0..240 {
        max_error = max_error.max(w.step().unwrap().max_joint_error_m);
    }
    assert!(w.bodies[1].angular_velocity_radps.length() > 0.01);
    assert!(max_error < 0.001, "anchor error {max_error}");
    let momentum = w.bodies[0].linear_velocity_mps * 2. + w.bodies[1].linear_velocity_mps * 3.;
    near(momentum.y, 2., 1e-10);
}

#[test]
fn definitions_resolve_shared_components_and_reject_bad_references() {
    use qx_physics::train::{Component, ComponentKind, Connection, TrainDefinition};
    let base = body(2., Vec3::ZERO);
    let component = |id: &str, kind| Component {
        id: id.into(),
        kind,
        dimensions_m: Vec3::new(2., 1., 1.),
        mass: base.mass,
        initial_pose: base.pose,
    };
    let connect = |a: &str, b: &str| Connection {
        component_a: a.into(),
        component_b: b.into(),
        anchor_a_m: Vec3::ZERO,
        anchor_b_m: Vec3::ZERO,
    };
    let mut train = TrainDefinition {
        components: vec![
            component("a", ComponentKind::CarBody),
            component("b", ComponentKind::CarBody),
            component("shared", ComponentKind::Bogie),
        ],
        connections: vec![connect("a", "shared"), connect("b", "shared")],
        guides: vec![],
    };
    let mut w = train.instantiate(Vec3::ZERO, 16, 2).unwrap();
    assert_eq!(w.bodies.len(), 3);
    assert_eq!(w.joints.len(), 2);
    w.step().unwrap();
    train.connections[0].component_b = "missing".into();
    assert!(train.instantiate(Vec3::ZERO, 16, 2).is_err());
    train.components[1].id = "a".into();
    assert!(train.instantiate(Vec3::ZERO, 16, 2).is_err());
}

#[test]
fn paused_step_and_reset_are_deterministic() {
    let initial = world(vec![body(1., Vec3::ZERO)]);
    let mut simulation = qx_physics::Simulation::new(initial.clone()).unwrap();
    assert!(simulation.tick().unwrap().is_none());
    simulation.step().unwrap();
    let after = simulation.world().clone();
    assert_eq!(simulation.ticks(), 1);
    simulation.reset();
    assert_eq!(simulation.world(), &initial);
    assert!(simulation.paused());
    simulation.set_paused(false);
    simulation.tick().unwrap().unwrap();
    assert_eq!(simulation.world(), &after);
}
