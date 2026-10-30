//! Neighbor-aware defaults for a new section, without changing existing inputs.
use crate::{Project, Result, require};
use qx_geometry::{
    authoring::{Domain, Section, SectionKind},
    math::{Frame, Vec3},
    solver::GeneratedTrack,
    transition::{Channel, Transition, Unit},
};

/// Initialize a fresh template at an insertion index using resolved *committed*
/// geometry from the same project. All defaults span 10 metres (or equivalent
/// design time). Force/local/world controls continue the exact incoming spatial
/// curvature and twist when representable. Straight/arc/spline keep their stated
/// construction. No downstream section or authored profile is fitted or moved.
pub fn initialize(
    project: &Project,
    track: &GeneratedTrack,
    index: usize,
    mut section: Section,
) -> Result<Section> {
    require(
        index <= project.sections.len(),
        "insertion index outside document",
    )?;
    require(
        track.sections.len() == project.sections.len()
            && track
                .sections
                .iter()
                .zip(&project.sections)
                .all(|(a, b)| a.id == b.id),
        "insertion needs the current committed geometry",
    )?;
    let (frame, omega, speed) = if index == 0 {
        (
            Frame::new(project.initial.orientation)?,
            Vec3::ZERO,
            project.initial.reference_speed_mps,
        )
    } else {
        let previous = &track.sections[index - 1];
        (
            track.samples[previous.end_sample].frame,
            previous.end_angular_rate_rad_per_m,
            previous.reference_speed_mps,
        )
    };
    section.reference_speed_mps = (speed != project.initial.reference_speed_mps).then_some(speed);
    if section.domain != Domain::Normalized {
        section.domain = if index > 0 && project.sections[index - 1].domain == Domain::Time {
            Domain::Time
        } else {
            Domain::Distance
        };
        section.extent = 10. / section.domain.metres_per_input(speed);
    }
    let scale = if section.domain == Domain::Normalized {
        10.
    } else {
        section.domain.metres_per_input(speed)
    };
    let profile = |unit, value| Channel {
        unit,
        transitions: vec![Transition::nurbs(1, section.extent, value, value)],
    };
    let tangent = frame.tangent();
    let curvature = omega.cross(tangent);
    section.roll_rate = profile(section.domain.rate_unit(), omega.dot(tangent) * scale);
    match &mut section.kind {
        SectionKind::Force {
            normal_g,
            lateral_g,
        } => {
            let acceleration = curvature * (speed * speed) + Vec3::Y * project.initial.gravity_mps2;
            *normal_g = profile(
                Unit::G,
                acceleration.dot(frame.normal()) / project.initial.gravity_mps2,
            );
            *lateral_g = profile(
                Unit::G,
                acceleration.dot(frame.lateral()) / project.initial.gravity_mps2,
            );
        }
        SectionKind::RiderLocal {
            pitch_rate,
            yaw_rate,
        } => {
            *pitch_rate = profile(
                section.domain.rate_unit(),
                curvature.dot(frame.normal()) * scale,
            );
            *yaw_rate = profile(
                section.domain.rate_unit(),
                curvature.dot(frame.lateral()) * scale,
            );
        }
        SectionKind::Geometric {
            pitch_rate,
            yaw_rate,
        } => {
            require(
                (1. - tangent.y * tangent.y).max(0.).sqrt() > 1e-6,
                "World-geometric pitch/yaw is undefined at this vertical start; insert Rider-local geometric instead",
            )?;
            let (up, _) = frame.horizontal_basis()?;
            let yaw = tangent.cross(Vec3::Y);
            *pitch_rate = profile(section.domain.rate_unit(), curvature.dot(up) * scale);
            *yaw_rate = profile(
                section.domain.rate_unit(),
                curvature.dot(yaw) / yaw.dot(yaw) * scale,
            );
        }
        _ => {}
    }
    section.validate()?;
    Ok(section)
}
