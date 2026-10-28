use crate::Result;
use qx_geometry::math::{Frame, Quaternion, Vec3};

/// Principal moments about the center of mass in body coordinates, kg m².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProperties {
    pub mass_kg: f64,
    pub principal_inertia_kg_m2: Vec3,
}
impl MassProperties {
    pub fn validate(self) -> Result<()> {
        let i = self.principal_inertia_kg_m2;
        if !self.mass_kg.is_finite()
            || self.mass_kg <= 0.0
            || !i.is_finite()
            || i.x <= 0.0
            || i.y <= 0.0
            || i.z <= 0.0
            || i.x + i.y < i.z
            || i.x + i.z < i.y
            || i.y + i.z < i.x
        {
            return Err(
                "mass and physically realizable principal moments must be finite and positive"
                    .into(),
            );
        }
        Ok(())
    }
    /// Uniform box centered on its mass center; dimensions are X/Y/Z in metres.
    pub fn uniform_box(mass_kg: f64, dimensions_m: Vec3) -> Result<Self> {
        if !dimensions_m.is_finite()
            || dimensions_m.x <= 0.0
            || dimensions_m.y <= 0.0
            || dimensions_m.z <= 0.0
        {
            return Err("box dimensions must be finite and positive".into());
        }
        let d = dimensions_m;
        let result = Self {
            mass_kg,
            principal_inertia_kg_m2: Vec3::new(
                mass_kg * (d.y * d.y + d.z * d.z) / 12.0,
                mass_kg * (d.x * d.x + d.z * d.z) / 12.0,
                mass_kg * (d.x * d.x + d.y * d.y) / 12.0,
            ),
        };
        result.validate()?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position_m: Vec3,
    pub orientation: Quaternion,
}

/// Independent rigid body. Velocities, forces and torques are in world coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub mass: MassProperties,
    pub pose: Pose,
    pub linear_velocity_mps: Vec3,
    pub angular_velocity_radps: Vec3,
    pub force_n: Vec3,
    pub torque_nm: Vec3,
}
impl Body {
    pub fn validate(&self) -> Result<()> {
        self.mass.validate()?;
        let q = self.pose.orientation;
        let norm = q.w.hypot(q.x).hypot(q.y).hypot(q.z);
        if !self.pose.position_m.is_finite()
            || !self.linear_velocity_mps.is_finite()
            || !self.angular_velocity_radps.is_finite()
            || !self.force_n.is_finite()
            || !self.torque_nm.is_finite()
            || !norm.is_finite()
            || (norm - 1.0).abs() > 1e-8
        {
            return Err("body state must be finite with a unit orientation".into());
        }
        Ok(())
    }
    pub(crate) fn frame(&self) -> Frame {
        Frame::new(self.pose.orientation).expect("validated orientation")
    }
    pub fn world_point(&self, local: Vec3) -> Result<Vec3> {
        self.validate()?;
        if !local.is_finite() {
            return Err("local point must be finite".into());
        }
        let point = self.pose.position_m + self.frame().local_to_world(local);
        if !point.is_finite() {
            return Err("world point overflow".into());
        }
        Ok(point)
    }
    pub(crate) fn inv_inertia(&self, v: Vec3) -> Vec3 {
        let f = self.frame();
        let x = f.world_to_local(v);
        let i = self.mass.principal_inertia_kg_m2;
        f.local_to_world(Vec3::new(x.x / i.x, x.y / i.y, x.z / i.z))
    }
    fn inertia(&self, v: Vec3) -> Vec3 {
        let f = self.frame();
        let x = f.world_to_local(v);
        let i = self.mass.principal_inertia_kg_m2;
        f.local_to_world(Vec3::new(x.x * i.x, x.y * i.y, x.z * i.z))
    }
    pub(crate) fn integrate_forces(&mut self, gravity: Vec3, dt: f64) {
        self.linear_velocity_mps =
            self.linear_velocity_mps + (gravity + self.force_n / self.mass.mass_kg) * dt;
        let w = self.angular_velocity_radps;
        self.angular_velocity_radps =
            w + self.inv_inertia(self.torque_nm - w.cross(self.inertia(w))) * dt;
    }
    pub(crate) fn advance(&mut self, dt: f64) -> Result<()> {
        self.pose.position_m = self.pose.position_m + self.linear_velocity_mps * dt;
        let w = self.angular_velocity_radps;
        let speed = w.length();
        if speed > 1e-14 {
            let dq = Quaternion::axis_angle(w, speed * dt).map_err(|e| e.to_string())?;
            self.pose.orientation = dq
                .compose(self.pose.orientation)
                .normalized()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub(crate) fn point_velocity(&self, r: Vec3) -> Vec3 {
        self.linear_velocity_mps + self.angular_velocity_radps.cross(r)
    }
    pub(crate) fn effective_mass(&self, r: Vec3, n: Vec3) -> f64 {
        1.0 / self.mass.mass_kg + n.dot(self.inv_inertia(r.cross(n)).cross(r))
    }
    pub(crate) fn impulse(&mut self, r: Vec3, impulse: Vec3) {
        self.linear_velocity_mps = self.linear_velocity_mps + impulse / self.mass.mass_kg;
        self.angular_velocity_radps =
            self.angular_velocity_radps + self.inv_inertia(r.cross(impulse));
    }
}
