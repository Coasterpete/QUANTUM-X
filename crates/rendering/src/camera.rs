use glam::{DMat4, DVec3};
use qx_geometry::math::Vec3;
pub(crate) fn dvec(v: Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}
pub(crate) struct Camera {
    pub target: DVec3,
    pub distance: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub orthographic: bool,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            target: DVec3::ZERO,
            distance: 100.,
            yaw: 0.8,
            pitch: 0.5,
            orthographic: false,
        }
    }
}
impl Camera {
    pub fn fit(&mut self, min: Vec3, max: Vec3, aspect: f64) {
        self.target = (dvec(min) + dvec(max)) * 0.5;
        let radius = (dvec(max) - dvec(min)).length().max(2.) * 0.5;
        let half_angle = (0.5_f64.tan() * aspect.min(1.)).atan();
        self.distance = (radius / half_angle.sin() * 1.15).clamp(0.01, 1e10);
    }
    fn direction(&self) -> DVec3 {
        DVec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }
    fn up(&self) -> DVec3 {
        if self.pitch.cos().abs() < 1e-5 {
            DVec3::Z
        } else {
            DVec3::Y
        }
    }
    pub fn orbit(&mut self, x: f64, y: f64) {
        self.yaw += x;
        self.pitch = (self.pitch + y).clamp(-1.569, 1.569);
    }
    pub fn pan(&mut self, x: f64, y: f64) {
        let backward = self.direction();
        let right = self.up().cross(backward).normalize();
        let up = backward.cross(right);
        self.target += right * x + up * y;
    }
    pub fn zoom(&mut self, delta: f64) {
        self.distance = (self.distance * (delta * 0.12).exp()).clamp(0.01, 1e10);
    }
    pub fn matrix(&self, origin: Vec3, aspect: f64) -> [[f32; 4]; 4] {
        let target = self.target - dvec(origin);
        let eye = target + self.direction() * self.distance;
        let view = DMat4::look_at_rh(eye, target, self.up());
        let near = (self.distance * 1e-5).max(1e-5);
        let far = self.distance * 100. + 100.;
        let projection = if self.orthographic {
            let h = self.distance * 0.5_f64.tan();
            DMat4::orthographic_rh(-h * aspect, h * aspect, -h, h, near, far)
        } else {
            DMat4::perspective_rh(1., aspect, near, far)
        };
        (projection * view).as_mat4().to_cols_array_2d()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_origin_rebasing_and_orthographic_views_are_finite() {
        let origin = Vec3::new(1e8, 1e8, 1e8);
        let mut c = Camera::default();
        c.fit(origin, origin + Vec3::new(1., 2., 3.), 0.3);
        for pitch in [0., std::f64::consts::FRAC_PI_2] {
            c.pitch = pitch;
            for ortho in [false, true] {
                c.orthographic = ortho;
                assert!(
                    c.matrix(origin, 0.3)
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite())
                );
            }
        }
    }
}
