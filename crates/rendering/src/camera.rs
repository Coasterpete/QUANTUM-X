use glam::{DMat4, DVec3};
use qx_geometry::math::Vec3;
pub(crate) fn dvec(v: Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}
#[derive(Clone)]
pub(crate) struct Camera {
    pub target: DVec3,
    pub distance: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub orthographic: bool,
}
pub(crate) struct Projection {
    matrix: DMat4,
    origin: DVec3,
    viewport: egui::Rect,
}
impl Projection {
    pub fn project(&self, point: Vec3) -> Option<egui::Pos2> {
        let clip = self.matrix * (dvec(point) - self.origin).extend(1.);
        if clip.w <= 0. || !clip.is_finite() {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if !(0. ..=1.).contains(&ndc.z) {
            return None;
        }
        Some(egui::pos2(
            self.viewport.left() + (ndc.x as f32 + 1.) * self.viewport.width() * 0.5,
            self.viewport.top() + (1. - ndc.y as f32) * self.viewport.height() * 0.5,
        ))
    }
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
        let aspect = if aspect.is_finite() && aspect > 0. {
            aspect
        } else {
            1.
        };
        let backward = self.direction();
        let right = self.up().cross(backward).normalize();
        let up = backward.cross(right);
        let tan_y = 0.5_f64.tan();
        let mut distance: f64 = 0.01;
        // Fit all eight bounds corners in the current camera basis. Thin/short
        // tracks no longer inherit the wasted space of a bounding sphere.
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let p = dvec(Vec3::new(x, y, z)) - self.target;
                    let depth = p.dot(backward);
                    let planar =
                        (p.dot(right).abs() / (tan_y * aspect)).max(p.dot(up).abs() / tan_y) * 1.15;
                    distance = distance.max(if self.orthographic {
                        planar.max(depth + 0.01)
                    } else {
                        depth + planar
                    });
                }
            }
        }
        self.distance = distance.clamp(0.01, 1e10);
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
        self.matrix_f64(origin, aspect).as_mat4().to_cols_array_2d()
    }
    #[cfg(test)]
    pub(crate) fn project(
        &self,
        point: Vec3,
        origin: Vec3,
        viewport: egui::Rect,
    ) -> Option<egui::Pos2> {
        self.projection(origin, viewport).project(point)
    }
    pub(crate) fn projection(&self, origin: Vec3, viewport: egui::Rect) -> Projection {
        Projection {
            matrix: self.matrix_f64(
                origin,
                viewport.width() as f64 / viewport.height().max(1.) as f64,
            ),
            origin: dvec(origin),
            viewport,
        }
    }
    fn matrix_f64(&self, origin: Vec3, aspect: f64) -> DMat4 {
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
        projection * view
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
    #[test]
    fn fit_bounds_stays_visible_at_extreme_aspects_lengths_and_large_origins() {
        for length in [0.1, 10., 1000.] {
            for aspect in [0.25, 1., 4.] {
                for ortho in [false, true] {
                    let origin = Vec3::new(1e8, 1e8, -1e8);
                    let max = origin + Vec3::new(length, length * 0.2, length * 0.1);
                    let viewport = egui::Rect::from_min_size(
                        egui::pos2(100., 90.),
                        egui::vec2(300. * aspect, 300.),
                    );
                    let mut camera = Camera {
                        orthographic: ortho,
                        ..Default::default()
                    };
                    camera.fit(origin, max, aspect as f64);
                    for x in [origin.x, max.x] {
                        for y in [origin.y, max.y] {
                            for z in [origin.z, max.z] {
                                let p = camera
                                    .project(Vec3::new(x, y, z), origin, viewport)
                                    .unwrap();
                                assert!(
                                    viewport
                                        .shrink(viewport.width().min(viewport.height()) * 0.05)
                                        .contains(p),
                                    "{p:?} in {viewport:?}, length {length}"
                                );
                            }
                        }
                    }
                    let a = camera.project(origin, origin, viewport).unwrap();
                    let b = camera.project(max, origin, viewport).unwrap();
                    assert!(a.distance(b) > viewport.width().min(viewport.height()) * 0.25);
                    camera.orbit(0.2, 0.1);
                    camera.pan(length * 0.1, length * 0.2);
                    camera.zoom(-0.1);
                    let orientation = (camera.yaw, camera.pitch, camera.orthographic);
                    camera.fit(origin, max, aspect as f64);
                    assert_eq!((camera.yaw, camera.pitch, camera.orthographic), orientation);
                }
            }
        }
    }
}
