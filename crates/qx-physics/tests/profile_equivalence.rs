//! Existing rigid-body solver, unchanged, driven by equivalent authored profiles.
use qx_document::{
    editing,
    train::{DevelopmentArrangement, Train},
};
use qx_geometry::{
    authoring::{ChannelKey, InitialConditions, Section, SectionKind},
    solver::{GenerationOptions, generate},
    transition::{Channel, Function, Transition, Unit},
};
use qx_physics::{FIXED_STEP_SECONDS, TrackSimulation};
use std::sync::Arc;

#[test]
fn exact_profile_splits_preserve_force_banking_geometry_and_fixed_step_rigid_body_dynamics() {
    let profile = |start, end| Channel {
        unit: Unit::G,
        transitions: vec![Transition {
            id: 1,
            length: 100.,
            start,
            end,
            amplitude: 0.01,
            function: Function::Hermite {
                start_slope: 0.,
                end_slope: 0.,
            },
        }],
    };
    let mut s = Section::straight(1, "Synthetic force/bank compatibility", 100.);
    s.kind = SectionKind::Force {
        normal_g: profile(1., 1.08),
        lateral_g: profile(0., 0.03),
    };
    s.roll_rate = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![Transition {
            id: 1,
            length: 100.,
            start: 0.,
            end: 0.002,
            amplitude: 0.0002,
            function: Function::CubicSmoothstep {},
        }],
    };
    let initial = InitialConditions::default();
    let options = GenerationOptions::default();
    let original = Arc::new(generate(&initial, &[s.clone()], options).unwrap());
    for key in [ChannelKey::Normal, ChannelKey::Lateral, ChannelKey::Roll] {
        s = editing::split_transition(&s, key, 1, 0.3577).unwrap();
    }
    let split = Arc::new(generate(&initial, &[s], options).unwrap());
    assert_eq!(original.samples.len(), split.samples.len());
    for (a, b) in original.samples.iter().zip(&split.samples) {
        assert!((a.position_m - b.position_m).length() < 2e-11);
        assert!((a.frame.normal() - b.frame.normal()).length() < 2e-12);
    }
    let train = Train::development_consist(
        "unchanged-bodies".into(),
        1,
        DevelopmentArrangement::Conventional,
    )
    .unwrap();
    let mut a = TrackSimulation::new(train.clone(), original, 20., 2.).unwrap();
    let mut b = TrackSimulation::new(train, split, 20., 2.).unwrap();
    let mut max_position: f64 = 0.;
    let mut max_velocity: f64 = 0.;
    assert_eq!(FIXED_STEP_SECONDS, 1. / 240.);
    for _ in 0..240 {
        let x = a.step().unwrap();
        let y = b.step().unwrap();
        assert!((x.max_guide_error_m - y.max_guide_error_m).abs() < 1e-8);
        for (x, y) in a.world().bodies.iter().zip(&b.world().bodies) {
            max_position = max_position.max((x.pose.position_m - y.pose.position_m).length());
            max_velocity =
                max_velocity.max((x.linear_velocity_mps - y.linear_velocity_mps).length());
            assert!((x.angular_velocity_radps - y.angular_velocity_radps).length() < 1e-7);
            assert_eq!(x.mass, y.mass);
        }
    }
    assert!(
        max_position < 1e-8 && max_velocity < 1e-7,
        "position={max_position:e}; velocity={max_velocity:e}"
    );
    println!(
        "240 rigid-body steps: max position delta {max_position:.16e} m; max velocity delta {max_velocity:.16e} m/s"
    );
}
