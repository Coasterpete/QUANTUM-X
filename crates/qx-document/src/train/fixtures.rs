//! Opt-in numerical development arrangements, never production coaster designs.
use super::{Car, Component, ComponentKind, Connection, Train, mechanism::*};
use crate::{Result, require};
use qx_geometry::math::{Quaternion, Vec3};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DevelopmentArrangement {
    #[default]
    Conventional,
    BelowTrack,
    RotatingSeats,
}
impl Train {
    /// Variable-length diagnostic consist. No production factory or manufacturer fidelity is implied.
    pub fn development_consist(
        id: String,
        count: usize,
        arrangement: DevelopmentArrangement,
    ) -> Result<Self> {
        require(
            (1..=256).contains(&count),
            "development car count must be 1..=256",
        )?;
        let mut train = Self {
            id,
            name: format!("Development {arrangement:?} ({count} cars)"),
            components: vec![],
            cars: vec![],
            connections: vec![],
            compatibility: Default::default(),
        };
        let height = if arrangement == DevelopmentArrangement::Conventional {
            0.8
        } else {
            -1.4
        };
        for n in 0..count {
            let body_id = format!("car-{}", n + 1);
            let offset = n as f64 * 4.;
            train.components.push(component(
                body_id.clone(),
                ComponentKind::CarBody,
                Vec3::new(3., 0.7, 1.4),
                600.,
                Placement::Track {
                    offset_m: -offset,
                    frame: LocalFrame {
                        translation_m: Vec3::new(0., height, 0.),
                        ..Default::default()
                    },
                },
            ));
            let mut gear = vec![];
            for (k, x) in [1.2, -1.2].into_iter().enumerate() {
                let id = format!("gear-{}-{}", n + 1, k + 1);
                let mut c = component(
                    id.clone(),
                    ComponentKind::RunningGear,
                    Vec3::new(0.6, 0.3, 0.9),
                    100.,
                    Placement::Track {
                        offset_m: -offset + x,
                        frame: LocalFrame::default(),
                    },
                );
                // Two longitudinal references on one centerline, with no assumption about rail count.
                c.guide_references_m = vec![Vec3::new(-0.2, 0., 0.), Vec3::new(0.2, 0., 0.)];
                train.components.push(c);
                train.connections.push(Connection {
                    id: format!("suspension-{}-{}", n + 1, k + 1),
                    component_a: body_id.clone(),
                    component_b: id.clone(),
                    anchor_a_m: Vec3::new(x, -height, 0.),
                    anchor_b_m: Vec3::ZERO,
                    model: JointModel::Ball,
                });
                gear.push(id);
            }
            if arrangement == DevelopmentArrangement::RotatingSeats {
                let id = format!("seat-{}", n + 1);
                let mount = Vec3::new(0., -0.7, 0.);
                let mut seat = component(
                    id.clone(),
                    ComponentKind::PassengerAssembly,
                    Vec3::new(0.8, 1.2, 1.),
                    120.,
                    Placement::Mounted {
                        parent: body_id.clone(),
                        frame: LocalFrame {
                            translation_m: mount,
                            rotation: Quaternion::axis_angle(Vec3::Z, 0.35)
                                .expect("constant rotation"),
                        },
                    },
                );
                seat.center_of_mass_m = Vec3::new(0., -0.4, 0.);
                train.components.push(seat);
                train.connections.push(Connection {
                    id: format!("seat-hinge-{}", n + 1),
                    component_a: body_id.clone(),
                    component_b: id,
                    anchor_a_m: mount,
                    anchor_b_m: Vec3::ZERO,
                    model: JointModel::Hinge {
                        frame_a: Quaternion::IDENTITY,
                        frame_b: Quaternion::IDENTITY,
                        limits: Some(AngleLimits {
                            min_rad: -1.2,
                            max_rad: 1.2,
                        }),
                        drive: RotationDrive::Free,
                    },
                });
            }
            train.cars.push(Car {
                body_id: body_id.clone(),
                offset_m: offset,
                bogie_ids: gear,
            });
            if n > 0 {
                train.connections.push(Connection {
                    id: format!("coupler-{n}"),
                    component_a: format!("car-{n}"),
                    component_b: body_id,
                    anchor_a_m: Vec3::new(-2., 0., 0.),
                    anchor_b_m: Vec3::new(2., 0., 0.),
                    model: JointModel::Ball,
                });
            }
        }
        train.validate()?;
        Ok(train)
    }
}
fn component(
    id: String,
    kind: ComponentKind,
    d: Vec3,
    mass: f64,
    placement: Placement,
) -> Component {
    Component {
        id,
        kind,
        dimensions_m: d,
        mass_kg: mass,
        inertia_kg_m2: Vec3::new(
            mass * (d.y * d.y + d.z * d.z) / 12.,
            mass * (d.x * d.x + d.z * d.z) / 12.,
            mass * (d.x * d.x + d.y * d.y) / 12.,
        ),
        center_of_mass_m: Vec3::ZERO,
        placement: Some(placement),
        asset: None,
        wheel_references_m: vec![],
        guide_references_m: vec![],
    }
}
