//! Persisted, renderer-independent train definitions. Coordinates use metres and
//! +X forward, +Y up, +Z lateral. Pivots are component-local transforms.
use crate::{Result, require};
use qx_geometry::math::{Quaternion, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
mod fixtures;
pub mod mechanism;
pub use fixtures::DevelopmentArrangement;
use mechanism::{JointModel, Placement};

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
    PassengerAssembly,
    RunningGear,
    Auxiliary,
}
impl ComponentKind {
    pub fn is_running_gear(self) -> bool {
        matches!(self, Self::Bogie | Self::RunningGear)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub dimensions_m: Vec3,
    pub mass_kg: f64,
    pub inertia_kg_m2: Vec3,
    /// Inertia is about this mass center, with principal axes parallel to the component axes.
    #[serde(default)]
    pub center_of_mass_m: Vec3,
    /// Initial assembly only; never a prescribed pose during dynamics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
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
    #[serde(default)]
    pub model: JointModel,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Train {
    pub id: String,
    pub name: String,
    pub components: Vec<Component>,
    pub cars: Vec<Car>,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub compatibility: qx_style::compatibility::VehicleCompatibility,
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
    /// Inspector grouping only; physical relationships always come from connections.
    pub fn car_contains_component(&self, car: &Car, id: &str) -> bool {
        let mut current = id;
        for _ in 0..=self.components.len() {
            if car.body_id == current || car.bogie_ids.iter().any(|g| g == current) {
                return true;
            }
            let Some(Component {
                placement: Some(Placement::Mounted { parent, .. }),
                ..
            }) = self.components.iter().find(|c| c.id == current)
            else {
                return false;
            };
            current = parent;
        }
        false
    }
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
        self.compatibility
            .validate()
            .map_err(crate::DocumentError)?;
        for c in &self.components {
            require(
                valid_id(&c.id) && ids.insert(c.id.as_str()),
                "invalid or duplicate component ID",
            )?;
            let i = c.inertia_kg_m2;
            require(c.center_of_mass_m.is_finite(), "nonfinite mass center")?;
            if let Some(p) = &c.placement {
                p.validate()?;
            }
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
                        && self.components.iter().any(|c| {
                            c.id == *id
                                && matches!(
                                    c.kind,
                                    ComponentKind::Bogie | ComponentKind::RunningGear
                                )
                        }),
                    "invalid or duplicate bogie reference",
                )?;
            }
        }
        let mut joints = HashSet::new();
        // Mounts form an initialization graph, independent of the physical joint graph.
        // Walking iteratively bounds stack use even for malicious long chains.
        let by_id: std::collections::HashMap<_, _> =
            self.components.iter().map(|c| (c.id.as_str(), c)).collect();
        for c in &self.components {
            let mut visited = HashSet::new();
            let mut current = c;
            while let Some(Placement::Mounted { parent, .. }) = &current.placement {
                require(
                    visited.insert(current.id.as_str()),
                    "cyclic component mounts",
                )?;
                current = by_id.get(parent.as_str()).copied().ok_or_else(|| {
                    crate::DocumentError(format!("unknown mount parent {parent}"))
                })?;
            }
        }
        for joint in &self.connections {
            joint.model.validate()?;
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
                center_of_mass_m: Vec3::ZERO,
                placement: None,
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
                    model: JointModel::Ball,
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
            model: JointModel::Ball,
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
            compatibility: Default::default(),
        }
    }
}
