use qx_document::train::{DevelopmentArrangement, Train};
use qx_geometry::{
    authoring::{InitialConditions, Section, SectionKind},
    math::{Quaternion, Vec3},
    solver::{GeneratedTrack, GenerationOptions, generate},
    transition::{Channel, Function, Transition, Unit},
};

pub fn conventional(count: usize) -> Train {
    Train::development_consist(
        "m1-diagnostic".into(),
        count,
        DevelopmentArrangement::Conventional,
    )
    .unwrap()
}

pub fn straight() -> GeneratedTrack {
    generate(
        &InitialConditions::default(),
        &[Section::straight(1, "M1 straight fixture", 600.)],
        GenerationOptions::default(),
    )
    .unwrap()
}

pub fn arc(radius: f64, bank: f64, length: f64, changing_bank: bool) -> GeneratedTrack {
    let mut section = Section::straight(1, "M1 circular fixture", length);
    section.kind = SectionKind::Arc {
        radius_m: radius,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    if changing_bank {
        section.roll_rate = Channel {
            unit: Unit::RadiansPerMetre,
            transitions: vec![Transition {
                id: 1,
                length,
                start: 0.,
                end: 0.004,
                amplitude: 0.,
                function: Function::Linear {},
            }],
        };
    }
    generate(
        &InitialConditions {
            orientation: Quaternion::axis_angle(Vec3::X, bank).unwrap(),
            ..Default::default()
        },
        &[section],
        GenerationOptions::default(),
    )
    .unwrap()
}
