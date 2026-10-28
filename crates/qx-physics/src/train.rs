//! In-memory train component definitions. Not a project serialization format.
use crate::{BallJoint, Body, Guide, MassProperties, Pose, Result, World};
use qx_geometry::math::Vec3;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentKind {
    CarBody,
    Bogie,
}

/// An independent rigid component; its pose places its center of mass.
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub dimensions_m: Vec3,
    pub mass: MassProperties,
    pub initial_pose: Pose,
}

/// A ball joint can connect any two components, including a shared bogie.
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub component_a: String,
    pub component_b: String,
    pub anchor_a_m: Vec3,
    pub anchor_b_m: Vec3,
}

/// Explicit idealized guide point, independent of any visual asset.
#[derive(Clone, Debug, PartialEq)]
pub struct GuidePoint {
    pub component: String,
    pub local_point_m: Vec3,
    pub line_origin_m: Vec3,
    pub line_direction: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrainDefinition {
    pub components: Vec<Component>,
    pub connections: Vec<Connection>,
    pub guides: Vec<GuidePoint>,
}
impl TrainDefinition {
    /// Creates a fresh simulation snapshot. It does not alter the definition.
    pub fn instantiate(&self, gravity_mps2: Vec3, iterations: u32, substeps: u32) -> Result<World> {
        if self.components.is_empty()
            || self.components.len() > 1024
            || self.connections.len() > 4096
            || self.guides.len() > 4096
        {
            return Err("invalid train component or connection count".into());
        }
        let mut ids = HashMap::new();
        let mut bodies = Vec::with_capacity(self.components.len());
        for (index, c) in self.components.iter().enumerate() {
            if c.id.is_empty()
                || c.id.len() > 128
                || ids.insert(c.id.as_str(), index).is_some()
                || !c.dimensions_m.is_finite()
                || c.dimensions_m.x <= 0.0
                || c.dimensions_m.y <= 0.0
                || c.dimensions_m.z <= 0.0
            {
                return Err(
                    "component ID must be unique and dimensions finite and positive".into(),
                );
            }
            bodies.push(Body {
                mass: c.mass,
                pose: c.initial_pose,
                linear_velocity_mps: Vec3::ZERO,
                angular_velocity_radps: Vec3::ZERO,
                force_n: Vec3::ZERO,
                torque_nm: Vec3::ZERO,
            });
        }
        let lookup = |id: &str| {
            ids.get(id)
                .copied()
                .ok_or_else(|| format!("unknown component ID: {id}"))
        };
        let mut joints = Vec::with_capacity(self.connections.len());
        for c in &self.connections {
            joints.push(BallJoint {
                body_a: lookup(&c.component_a)?,
                body_b: lookup(&c.component_b)?,
                anchor_a_m: c.anchor_a_m,
                anchor_b_m: c.anchor_b_m,
            });
        }
        let mut guides = Vec::with_capacity(self.guides.len());
        for g in &self.guides {
            guides.push(Guide {
                body: lookup(&g.component)?,
                local_point_m: g.local_point_m,
                line_origin_m: g.line_origin_m,
                line_direction: g.line_direction,
            });
        }
        let world = World {
            angular_joints: vec![],
            bodies,
            joints,
            guides,
            gravity_mps2,
            iterations,
            substeps,
        };
        world.validate()?;
        Ok(world)
    }
}
