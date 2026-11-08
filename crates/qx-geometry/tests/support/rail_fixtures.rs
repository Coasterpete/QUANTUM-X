//! Independently specified analytic development geometry, not coaster designs.
use qx_geometry::{
    math::{Frame, Quaternion, Vec3},
    solver::{CenterlineSample, GeneratedTrack},
};

pub fn sampled(length: f64, step: f64, at: impl Fn(f64) -> (Vec3, Quaternion)) -> GeneratedTrack {
    let n = (length / step).ceil() as usize;
    let samples = (0..=n)
        .map(|i| {
            let s = length * i as f64 / n as f64;
            let (p, q) = at(s);
            CenterlineSample {
                distance_m: s,
                design_time_s: 0.,
                position_m: p,
                frame: Frame::new(q).unwrap(),
            }
        })
        .collect();
    GeneratedTrack {
        samples,
        graphs: vec![],
        sections: vec![],
        diagnostics: vec![],
        connections: vec![],
        numerical: vec![],
        smooth_ranges: vec![],
    }
}

pub fn helix(s: f64, radius: f64, rise: f64, bank: f64) -> (Vec3, Quaternion) {
    let theta = s * (1. - rise * rise).sqrt() / radius;
    (
        Vec3::new(radius * theta.sin(), rise * s, radius * (1. - theta.cos())),
        Quaternion::axis_angle(Vec3::Y, -theta)
            .unwrap()
            .compose(Quaternion::axis_angle(Vec3::Z, rise.asin()).unwrap())
            .compose(Quaternion::axis_angle(Vec3::X, bank).unwrap()),
    )
}
pub fn helix_locus(s: f64, radius: f64, rise: f64, bank: f64, offset: Vec3) -> Vec3 {
    let a = (1. - rise * rise).sqrt();
    let theta = s * a / radius;
    let t = Vec3::new(a * theta.cos(), rise, a * theta.sin());
    let n = Vec3::new(-rise * theta.cos(), a, -rise * theta.sin());
    let l = Vec3::new(-theta.sin(), 0., theta.cos());
    Vec3::new(radius * theta.sin(), rise * s, radius * (1. - theta.cos()))
        + t * offset.x
        + (n * bank.cos() + l * bank.sin()) * offset.y
        + (-n * bank.sin() + l * bank.cos()) * offset.z
}
