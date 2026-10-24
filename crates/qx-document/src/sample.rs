//! Original development fixture, not a finished ride or manufacturer asset.
use crate::{Metadata, Project, SCHEMA_VERSION};
use qx_geometry::{
    authoring::{Domain, InitialConditions, Section, SectionKind},
    transition::{Channel, Function, Transition, Unit},
};
use qx_style::StyleSelection;

fn blend(id: u64, length: f64, start: f64, end: f64) -> Transition {
    Transition {
        id,
        length,
        start,
        end,
        amplitude: 0.0,
        function: Function::QuinticSmoothstep {},
    }
}
pub fn foundation_project() -> Project {
    let lead = Section::straight(1, "Level lead-in", 12.0);
    let mut force = Section::straight(2, "Force-vector rise and bank", 60.0);
    force.roll_rate = Channel {
        unit: Unit::RadiansPerMetre,
        transitions: vec![blend(1, 30.0, 0.0, 0.012), blend(2, 30.0, 0.012, 0.0)],
    };
    force.kind = SectionKind::Force {
        normal_g: Channel {
            unit: Unit::G,
            transitions: vec![blend(1, 30.0, 1.0, 1.7), blend(2, 30.0, 1.7, 1.0)],
        },
        lateral_g: Channel {
            unit: Unit::G,
            transitions: vec![blend(1, 30.0, 0.0, 0.35), blend(2, 30.0, 0.35, 0.0)],
        },
    };
    let mut local = Section::straight(3, "Rider-local recovery", 30.0);
    local.kind = SectionKind::RiderLocal {
        pitch_rate: Channel::constant(Unit::RadiansPerMetre, 30.0, -0.008),
        yaw_rate: Channel::constant(Unit::RadiansPerMetre, 30.0, 0.006),
    };
    let mut geometric = Section::straight(4, "Time-domain world turn", 1.5);
    geometric.domain = Domain::Time;
    geometric.roll_rate = Channel::constant(Unit::RadiansPerSecond, 1.5, -0.12);
    geometric.kind = SectionKind::Geometric {
        pitch_rate: Channel::constant(Unit::RadiansPerSecond, 1.5, -0.08),
        yaw_rate: Channel::constant(Unit::RadiansPerSecond, 1.5, 0.16),
    };
    let mut arc = Section::straight(5, "Construction arc", 20.0);
    arc.kind = SectionKind::Arc {
        radius_m: 80.0,
        plane_angle_rad: std::f64::consts::FRAC_PI_2,
    };
    Project{schema_version:SCHEMA_VERSION,metadata:Metadata{project_id:"qx-foundation-example-1".into(),name:"Core foundation diagnostic".into(),author:"QUANTUM-X contributors".into(),description:"Five authored sections exercising force, world/rider-local geometry, both domains, banking, and construction geometry.".into()},
        initial:InitialConditions::default(),style:StyleSelection{family_id:"hybrid".into(),style_id:"hybrid-100".into()},sections:vec![lead,force,local,geometric,arc]}
}
