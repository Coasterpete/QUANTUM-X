//! Synthetic development fixture, not a coaster layout or physical rail contact.
use qx_geometry::math::{Quaternion, Vec3};
use qx_physics::{
    MassProperties, Pose,
    train::{Component, ComponentKind, Connection, GuidePoint, TrainDefinition},
};
use std::error::Error;

pub fn run() -> Result<(), Box<dyn Error>> {
    let angle: f64 = 0.3;
    let tangent = Vec3::new(angle.cos(), -angle.sin(), 0.);
    let rotation = Quaternion::axis_angle(Vec3::Z, -angle)?;
    let mut components = Vec::new();
    for (name, kind, x, mass, dimensions) in [
        (
            "car-a",
            ComponentKind::CarBody,
            -1.,
            400.,
            Vec3::new(2., 1., 1.),
        ),
        (
            "car-b",
            ComponentKind::CarBody,
            1.,
            500.,
            Vec3::new(2., 1., 1.),
        ),
        (
            "bogie-a",
            ComponentKind::Bogie,
            -1.,
            50.,
            Vec3::new(0.5, 0.4, 0.5),
        ),
        (
            "bogie-b",
            ComponentKind::Bogie,
            1.,
            50.,
            Vec3::new(0.5, 0.4, 0.5),
        ),
    ] {
        components.push(Component {
            id: name.into(),
            kind,
            dimensions_m: dimensions,
            mass: MassProperties::uniform_box(mass, dimensions)?,
            initial_pose: Pose {
                position_m: tangent * x,
                orientation: rotation,
            },
        });
    }
    let connections = [
        ("car-a", "car-b", Vec3::X, -Vec3::X),
        ("car-a", "bogie-a", Vec3::ZERO, Vec3::ZERO),
        ("car-b", "bogie-b", Vec3::ZERO, Vec3::ZERO),
    ]
    .into_iter()
    .map(|(a, b, aa, bb)| Connection {
        component_a: a.into(),
        component_b: b.into(),
        anchor_a_m: aa,
        anchor_b_m: bb,
    })
    .collect();
    let guides = ["bogie-a", "bogie-b"]
        .into_iter()
        .map(|name| GuidePoint {
            component: name.into(),
            local_point_m: Vec3::ZERO,
            line_origin_m: Vec3::ZERO,
            line_direction: tangent,
        })
        .collect();
    let definition = TrainDefinition {
        components,
        connections,
        guides,
    };
    let mut world = definition.instantiate(Vec3::new(0., -9.81, 0.), 32, 2)?;
    let mut max_joint: f64 = 0.;
    let mut max_guide: f64 = 0.;
    for _ in 0..240 {
        let d = world.step()?;
        max_joint = max_joint.max(d.max_joint_error_m);
        max_guide = max_guide.max(d.max_guide_error_m);
    }
    println!(
        "Development physics diagnostic: 2 cars, 2 bogies, spherical joints, ideal straight-line guides\n1 s at 240 Hz; no wheel/rail contact, angular limits, or authored track\nSpeed {:.9} m/s (analytic {:.9}); maximum joint error {:.3e} m; maximum guide error {:.3e} m",
        world.bodies[0].linear_velocity_mps.dot(tangent),
        9.81 * angle.sin(),
        max_joint,
        max_guide
    );
    Ok(())
}
