//! Persisted, renderer-independent train definitions. Coordinates use metres and
//! +X forward, +Y up, +Z lateral. Pivots are component-local transforms.
use crate::{Result, require};
use qx_geometry::math::{Quaternion, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    /// UTF-8 project-relative path, using forward slashes; absolute and parent paths are forbidden.
    pub path: String,
    pub translation_m: Vec3,
    pub rotation: Quaternion,
    pub scale: Vec3,
    /// Model-space pivot: local = translation + rotation * scale * (model - pivot).
    /// Absent in early Schema 5 files, where the pivot is the model origin.
    #[serde(default)]
    pub pivot_m: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    CarBody,
    Bogie,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub dimensions_m: Vec3,
    pub mass_kg: f64,
    pub inertia_kg_m2: Vec3,
    pub asset: Option<Asset>,
    /// References in this component's local frame. They do not imply wheel contact.
    pub wheel_references_m: Vec<Vec3>,
    pub guide_references_m: Vec<Vec3>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Car {
    pub body_id: String,
    /// Signed distance behind the first car's center, in metres.
    pub offset_m: f64,
    /// References may be shared with another car.
    pub bogie_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub id: String,
    pub component_a: String,
    pub component_b: String,
    pub anchor_a_m: Vec3,
    pub anchor_b_m: Vec3,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Train {
    pub id: String,
    pub name: String,
    pub components: Vec<Component>,
    pub cars: Vec<Car>,
    pub connections: Vec<Connection>,
}
fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 128
}
fn positive(v: Vec3) -> bool {
    v.is_finite() && v.x > 0. && v.y > 0. && v.z > 0.
}
impl Asset {
    pub fn validate(&self) -> Result<()> {
        let path = self.path.as_str();
        require(
            !path.is_empty()
                && path.len() <= 1024
                && !path.contains('\\')
                && !path.contains(':')
                && path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".."),
            "asset path must be project-relative with forward slashes and no parent traversal",
        )?;
        let q = self.rotation;
        let norm = q.w.hypot(q.x).hypot(q.y).hypot(q.z);
        require(
            self.translation_m.is_finite()
                && self.pivot_m.is_finite()
                && positive(self.scale)
                && norm.is_finite()
                && (norm - 1.).abs() < 1e-8,
            "invalid asset transform",
        )
    }
}
impl Train {
    pub fn validate(&self) -> Result<()> {
        require(
            valid_id(&self.id) && !self.name.trim().is_empty() && self.name.len() <= 1024,
            "invalid train ID or name",
        )?;
        require(
            !self.cars.is_empty()
                && self.cars.len() <= 256
                && self.components.len() <= 1024
                && self.connections.len() <= 4096,
            "invalid train size",
        )?;
        let mut ids = HashSet::new();
        for c in &self.components {
            require(
                valid_id(&c.id) && ids.insert(c.id.as_str()),
                "invalid or duplicate component ID",
            )?;
            let i = c.inertia_kg_m2;
            require(
                positive(c.dimensions_m)
                    && c.mass_kg.is_finite()
                    && c.mass_kg > 0.
                    && positive(i)
                    && i.x + i.y >= i.z
                    && i.x + i.z >= i.y
                    && i.y + i.z >= i.x,
                "invalid component dimensions, mass, or principal inertia",
            )?;
            require(
                c.wheel_references_m.len() <= 1024
                    && c.guide_references_m.len() <= 1024
                    && c.wheel_references_m
                        .iter()
                        .chain(&c.guide_references_m)
                        .all(|p| p.is_finite()),
                "invalid component references",
            )?;
            if let Some(a) = &c.asset {
                a.validate()?;
            }
        }
        let mut bodies = HashSet::new();
        let mut previous = -1.;
        for car in &self.cars {
            require(
                car.offset_m.is_finite()
                    && car.offset_m >= 0.
                    && car.offset_m > previous
                    && (previous >= 0. || car.offset_m == 0.),
                "car offsets must start at zero and increase",
            )?;
            previous = car.offset_m;
            require(
                bodies.insert(car.body_id.as_str())
                    && self
                        .components
                        .iter()
                        .any(|c| c.id == car.body_id && c.kind == ComponentKind::CarBody),
                "car body must reference a unique car-body component",
            )?;
            let mut bogies = HashSet::new();
            for id in &car.bogie_ids {
                require(
                    bogies.insert(id)
                        && self
                            .components
                            .iter()
                            .any(|c| c.id == *id && c.kind == ComponentKind::Bogie),
                    "invalid or duplicate bogie reference",
                )?;
            }
        }
        let mut joints = HashSet::new();
        for joint in &self.connections {
            require(
                valid_id(&joint.id)
                    && joints.insert(joint.id.as_str())
                    && joint.component_a != joint.component_b
                    && ids.contains(joint.component_a.as_str())
                    && ids.contains(joint.component_b.as_str())
                    && joint.anchor_a_m.is_finite()
                    && joint.anchor_b_m.is_finite(),
                "invalid joint ID, components, or anchors",
            )?;
        }
        Ok(())
    }
}
impl Train {
    /// Explicit development fixture: two distinct bodies, four bogies, ball joints.
    pub fn development_fixture(id: String) -> Self {
        let component = |id: String, kind, dimensions_m: Vec3, mass_kg: f64| {
            let d = dimensions_m;
            Component {
                id,
                kind,
                dimensions_m,
                mass_kg,
                inertia_kg_m2: Vec3::new(
                    mass_kg * (d.y * d.y + d.z * d.z) / 12.,
                    mass_kg * (d.x * d.x + d.z * d.z) / 12.,
                    mass_kg * (d.x * d.x + d.y * d.y) / 12.,
                ),
                asset: None,
                wheel_references_m: if kind == ComponentKind::Bogie {
                    vec![Vec3::new(0., 0., -d.z * 0.5), Vec3::new(0., 0., d.z * 0.5)]
                } else {
                    vec![]
                },
                guide_references_m: if kind == ComponentKind::Bogie {
                    vec![Vec3::ZERO]
                } else {
                    vec![]
                },
            }
        };
        let mut components = Vec::new();
        let mut cars = Vec::new();
        let mut connections = Vec::new();
        for n in 0..2 {
            let body = format!("car-{}", n + 1);
            components.push(component(
                body.clone(),
                ComponentKind::CarBody,
                Vec3::new(3., 1., 1.5),
                600.,
            ));
            let mut bogies = Vec::new();
            for (k, x) in [1., -1.].into_iter().enumerate() {
                let bogie = format!("bogie-{}-{}", n + 1, k + 1);
                components.push(component(
                    bogie.clone(),
                    ComponentKind::Bogie,
                    Vec3::new(0.8, 0.4, 1.),
                    100.,
                ));
                connections.push(Connection {
                    id: format!("joint-{}-{}", n + 1, k + 1),
                    component_a: body.clone(),
                    component_b: bogie.clone(),
                    anchor_a_m: Vec3::new(x, 0., 0.),
                    anchor_b_m: Vec3::ZERO,
                });
                bogies.push(bogie);
            }
            cars.push(Car {
                body_id: body,
                offset_m: n as f64 * 4.,
                bogie_ids: bogies,
            });
        }
        connections.push(Connection {
            id: "coupler-1".into(),
            component_a: "car-1".into(),
            component_b: "car-2".into(),
            anchor_a_m: Vec3::new(-2., 0., 0.),
            anchor_b_m: Vec3::new(2., 0., 0.),
        });
        Self {
            id,
            name: "Development two-car fixture".into(),
            components,
            cars,
            connections,
        }
    }
}
