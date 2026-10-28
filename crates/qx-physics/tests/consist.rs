use qx_document::train::Train;
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::{GenerationOptions, generate},
};
use qx_physics::consist::{initialize, inspect};
#[test]
fn persisted_consist_poses_and_dynamics_are_distinct_and_deterministic() {
    let mut section = Section::straight(1, "arc", 100.);
    section.kind = SectionKind::Arc {
        radius_m: 80.,
        plane_angle_rad: 0.,
    };
    let track = generate(
        &InitialConditions {
            orientation: Quaternion::axis_angle(Vec3::Z, -0.5)
                .unwrap()
                .compose(Quaternion::axis_angle(Vec3::X, 2.8).unwrap()),
            ..Default::default()
        },
        &[section],
        GenerationOptions::default(),
    )
    .unwrap();
    let train = Train::development_fixture("test".into());
    let pose = inspect(&train, &track, 30.).unwrap();
    assert_eq!(pose, inspect(&train, &track, 30.).unwrap());
    assert_eq!(pose.components.len(), 6);
    assert_eq!(pose.joints.len(), 5);
    let a = &pose
        .components
        .iter()
        .find(|p| p.id == "car-1")
        .unwrap()
        .pose;
    let b = &pose
        .components
        .iter()
        .find(|p| p.id == "car-2")
        .unwrap()
        .pose;
    let articulation = a
        .orientation
        .conjugate()
        .compose(b.orientation)
        .rotation_vector()
        .unwrap()
        .length();
    let up = qx_geometry::math::Frame::new(a.orientation)
        .unwrap()
        .normal();
    let max_residual = pose.joints.iter().map(|j| j.error_m).fold(0., f64::max);
    println!(
        "Inverted curved-track inspection: car relative rotation {:.6} deg; up dot world up {:.6}; maximum joint residual {:.9} m",
        articulation.to_degrees(),
        up.dot(Vec3::Y),
        max_residual
    );
    assert!(articulation > 0.01);
    assert!(up.dot(Vec3::Y) < 0.);
    assert!(pose.joints.iter().all(|j| j.error_m < 0.03));
    let (mut world, mut guides, initial) =
        initialize(&train, &track, 30., Vec3::new(0., -9.81, 0.), 32, 2).unwrap();
    let (mut replay, mut replay_guides) = (world.clone(), guides.clone());
    let mut max_joint_force: f64 = 0.;
    let mut max_guide_force: f64 = 0.;
    for _ in 0..120 {
        let d = world.step_on_track(&track, &mut guides).unwrap();
        assert_eq!(d, replay.step_on_track(&track, &mut replay_guides).unwrap());
        assert_eq!(d.joint_force_on_b_n.len(), 5);
        max_joint_force = max_joint_force.max(
            d.joint_force_on_b_n
                .iter()
                .map(|v| v.length())
                .fold(0., f64::max),
        );
        max_guide_force = max_guide_force.max(
            d.guide_force_n
                .iter()
                .map(|v| v.length())
                .fold(0., f64::max),
        );
        assert_eq!(d.guide_force_n.len(), 4);
        assert!(d.max_joint_error_m < 0.1 && d.max_guide_error_m < 0.1);
    }
    assert!(max_joint_force > 1. && max_guide_force > 1.);
    assert_eq!(world, replay);
    assert_ne!(world.bodies[0].pose, initial.components[0].pose);
    assert_eq!(pose, inspect(&train, &track, 30.).unwrap());
}
