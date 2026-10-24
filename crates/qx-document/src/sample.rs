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
        initial:InitialConditions::default(),style:StyleSelection{family_id:"hybrid".into(),style_id:"hybrid-100".into()},sections:vec![lead,force,local,geometric,arc],heartline:Default::default()}
}

fn controls(values: &[[f64; 3]]) -> Vec<qx_geometry::curve::ControlPoint> {
    values
        .iter()
        .enumerate()
        .map(|(i, p)| qx_geometry::curve::ControlPoint {
            id: i as u64 + 1,
            position_m: qx_geometry::math::Vec3::new(p[0], p[1], p[2]),
        })
        .collect()
}
/// Independent circle: track R=20 m, inward normal offset 2 m gives rider R=18 m.
/// This is an analytic development fixture, not a proposed coaster layout.
pub fn heartline_reference_project() -> Project {
    let mut p = foundation_project();
    p.metadata.project_id = "qx-heartline-circle-v3".into();
    p.metadata.name = "Heartline analytic circle (development fixture)".into();
    p.metadata.description = "Vertical radius-20 m reference circle with a +2 m rider-normal offset. Expected rider radius 18 m. Design-speed estimates only.".into();
    let mut section = Section::straight(1, "Analytic vertical circle", 20. * std::f64::consts::TAU);
    section.kind = SectionKind::Arc {
        radius_m: 20.,
        plane_angle_rad: 0.,
    };
    p.sections = vec![section];
    p.heartline.rider_offset = qx_geometry::heartline::OffsetProfile::Constant {
        offset_m: qx_geometry::math::Vec3::Y * 2.,
    };
    p.heartline.train_configuration_id = Some("nominal-reference-point-only".into());
    p
}
pub fn reference_arc(radius: f64) -> qx_geometry::curve::Curve {
    qx_geometry::curve::Curve::Nurbs {
        degree: 2,
        points: controls(&[[0.0, 0.0, 0.0], [radius, 0.0, 0.0], [radius, radius, 0.0]]),
        knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        weights: vec![1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0],
    }
}
pub fn spline_comparison_project() -> Project {
    use qx_geometry::curve::{Curve, Parameterization, clamped_knots};
    let mut p = foundation_project();
    p.metadata.project_id = "qx-spline-comparison-v2".into();
    p.metadata.name = "Four spline families".into();
    p.metadata.description =
        "Original development geometry; all sections use explicit local-zero/+X joins.".into();
    let curves = [
        Curve::Bezier {
            points: controls(&[
                [0., 0., 0.],
                [6., 0., 0.],
                [12., 4., 0.],
                [18., 4., 0.],
                [24., 4., 0.],
                [30., -2., 5.],
                [36., -2., 5.],
            ]),
        },
        Curve::BSpline {
            degree: 3,
            points: controls(&[
                [0., 0., 0.],
                [6., 0., 0.],
                [12., 3., 0.],
                [20., 7., 4.],
                [28., 0., 8.],
                [36., 0., 8.],
            ]),
            knots: clamped_knots(6, 3).expect("valid fixture"),
        },
        reference_arc(20.),
        Curve::CatmullRom {
            points: controls(&[
                [0., 0., 0.],
                [8., 0., 0.],
                [17., 3., 2.],
                [27., -2., 5.],
                [40., 0., 7.],
            ]),
            parameterization: Parameterization::Centripetal,
        },
    ];
    p.sections = curves
        .into_iter()
        .enumerate()
        .map(|(i, c)| Section::spline(100 + i as u64, format!("{} comparison", c.method()), c))
        .collect();
    p.sections[1].roll_rate = Channel {
        unit: Unit::RadiansPerNormalized,
        transitions: vec![blend(1, 0.5, 0.0, 0.8), blend(2, 0.5, 0.8, 0.0)],
    };
    p
}
pub fn mixed_authoring_project() -> Project {
    let mut p = foundation_project();
    p.metadata.project_id = "qx-mixed-authoring-v2".into();
    p.metadata.name = "Graph and spline mixed authoring".into();
    p.metadata.description="Force-vector and geometric graph sections interleaved with four explicit spline representations.".into();
    let mut splines = spline_comparison_project().sections.into_iter();
    let old = p.sections.clone();
    p.sections = vec![
        old[0].clone(),
        old[1].clone(),
        splines.next().unwrap(),
        old[3].clone(),
        splines.next().unwrap(),
        splines.next().unwrap(),
        splines.next().unwrap(),
        old[2].clone(),
        old[4].clone(),
    ];
    let mut force = old[1].clone();
    force.id = qx_geometry::authoring::SectionId(200);
    force.name = "Final force-vector section".into();
    p.sections.push(force);
    p
}
pub fn numerical_reference_project() -> Project {
    use qx_geometry::curve::{Curve, Parameterization};
    let mut p = spline_comparison_project();
    p.metadata.project_id = "qx-numerical-references-v2".into();
    p.metadata.name = "Analytic lines and rational quarter circle".into();
    p.metadata.description="Lengths: Bezier 30m, nonuniform B-spline 20m, radius-20m NURBS quarter circle 10*pi m, Catmull-Rom 30m. Total 80+10*pi m.".into();
    p.sections = vec![
        Section::spline(
            1,
            "Bezier line 30m",
            Curve::Bezier {
                points: controls(&[[0., 0., 0.], [2., 0., 0.], [21., 0., 0.], [30., 0., 0.]]),
            },
        ),
        Section::spline(
            2,
            "Nonuniform B-spline line 20m",
            Curve::BSpline {
                degree: 2,
                points: controls(&[[0., 0., 0.], [2., 0., 0.], [13., 0., 0.], [20., 0., 0.]]),
                knots: vec![0., 0., 0., 0.3, 1., 1., 1.],
            },
        ),
        Section::spline(3, "NURBS quarter circle R=20m", reference_arc(20.)),
        Section::spline(
            4,
            "Centripetal line 30m",
            Curve::CatmullRom {
                points: controls(&[[0., 0., 0.], [2., 0., 0.], [18., 0., 0.], [30., 0., 0.]]),
                parameterization: Parameterization::Centripetal,
            },
        ),
    ];
    p
}
