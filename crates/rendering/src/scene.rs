use qx_geometry::{authoring::SectionId, math::Vec3, solver::GeneratedTrack};
#[derive(Clone, Debug)]
pub struct SectionLabel {
    pub id: SectionId,
    pub name: String,
    pub method: String,
}
#[derive(Clone)]
pub struct InspectionScene {
    pub mesh: Option<std::sync::Arc<qx_geometry::construction::TrackMesh>>,
    pub mesh_ms: f64,
    pub solver_ms: f64,
    pub name: String,
    pub track: GeneratedTrack,
    pub labels: Vec<SectionLabel>,
    pub reference: Option<qx_geometry::heartline::ReferenceTrack>,
    pub authored_reference: qx_geometry::heartline::AuthoredReference,
}
impl InspectionScene {
    pub(crate) fn bounds(&self, section: Option<usize>) -> (Vec3, Vec3) {
        self.visible_bounds(section, true)
    }
    pub(crate) fn visible_bounds(
        &self,
        section: Option<usize>,
        reference_visible: bool,
    ) -> (Vec3, Vec3) {
        self.filtered_bounds(section, reference_visible, None)
    }
    pub(crate) fn display_bounds(
        &self,
        section: Option<usize>,
        display: &DisplayOptions,
    ) -> (Vec3, Vec3) {
        self.filtered_bounds(section, display.reference, Some(display))
    }
    fn filtered_bounds(
        &self,
        section: Option<usize>,
        reference_visible: bool,
        display: Option<&DisplayOptions>,
    ) -> (Vec3, Vec3) {
        let samples = if let Some(s) = section.and_then(|i| self.track.sections.get(i)) {
            &self.track.samples[s.start_sample..=s.end_sample]
        } else {
            &self.track.samples[..]
        };
        let mut min = samples[0].position_m;
        let mut max = min;
        for s in samples {
            let p = s.position_m;
            min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
            max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
        }
        if reference_visible && let Some(reference) = &self.reference {
            let selected_id = section
                .and_then(|i| self.track.sections.get(i))
                .map(|s| s.id);
            for point in &reference.points {
                if selected_id.is_some_and(|id| id != point.section_id) {
                    continue;
                }
                let p = point.position_m;
                min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
            }
        }
        if let Some(mesh) = &self.mesh {
            let (start, end) = section
                .and_then(|i| self.track.sections.get(i))
                .map_or((0., self.track.length_m()), |s| {
                    (s.start_distance_m, s.end_distance_m)
                });
            for c in &mesh.components {
                if !c.visible
                    || display.is_some_and(|d| {
                        d.mode == DisplayMode::Technical
                            || !match c.role {
                                qx_style::construction::Role::Running => d.running,
                                qx_style::construction::Role::Structure => d.structure,
                                qx_style::construction::Role::Hardware => d.hardware,
                            }
                    })
                {
                    continue;
                }
                if section.is_none() {
                    min = Vec3::new(
                        min.x.min(c.bounds.min.x),
                        min.y.min(c.bounds.min.y),
                        min.z.min(c.bounds.min.z),
                    );
                    max = Vec3::new(
                        max.x.max(c.bounds.max.x),
                        max.y.max(c.bounds.max.y),
                        max.z.max(c.bounds.max.z),
                    );
                } else if c.role == qx_style::construction::Role::Hardware {
                    for p in c
                        .placements
                        .iter()
                        .filter(|p| p.source_station_m >= start && p.source_station_m <= end)
                    {
                        for v in &c.mesh.vertices {
                            let p = p.position_m + p.frame.local_to_world(v.position_m);
                            min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                            max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
                        }
                    }
                } else {
                    for v in c
                        .mesh
                        .vertices
                        .iter()
                        .filter(|v| v.source_station_m >= start && v.source_station_m <= end)
                    {
                        let p = v.position_m;
                        min = Vec3::new(min.x.min(p.x), min.y.min(p.y), min.z.min(p.z));
                        max = Vec3::new(max.x.max(p.x), max.y.max(p.y), max.z.max(p.z));
                    }
                }
            }
        }
        (min, max)
    }
    /// Hit-test projected centerline segments, retaining stable IDs and station.
    /// Hover/picking is inspection only; this never moves authored geometry.
    pub(crate) fn pick(
        &self,
        camera: &crate::camera::Camera,
        origin: Vec3,
        viewport: egui::Rect,
        pointer: egui::Pos2,
    ) -> Option<(SectionId, f64)> {
        if !viewport.contains(pointer) {
            return None;
        }
        let mut nearest = 8_f32.powi(2);
        let mut hit = None;
        let projection = camera.projection(origin, viewport);
        for section in &self.track.sections {
            for pair in self.track.samples[section.start_sample..=section.end_sample].windows(2) {
                let (Some(a), Some(b)) = (
                    projection.project(pair[0].position_m),
                    projection.project(pair[1].position_m),
                ) else {
                    continue;
                };
                let delta = b - a;
                let fraction =
                    ((pointer - a).dot(delta) / delta.length_sq().max(1e-12)).clamp(0., 1.);
                let distance = (a + delta * fraction).distance_sq(pointer);
                if distance < nearest {
                    nearest = distance;
                    hit = Some((
                        section.id,
                        pair[0].distance_m
                            + (pair[1].distance_m - pair[0].distance_m) * fraction as f64,
                    ));
                }
            }
        }
        hit
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}
const GRID_HALF_LINES: i32 = 30;
/// Grid lines are the leading batch and never write the geometry depth buffer.
pub(crate) fn grid_vertex_count(display: &DisplayOptions) -> u32 {
    if display.grid {
        (2 * GRID_HALF_LINES as u32 + 1) * 4
    } else {
        0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Rendered,
    RenderedCenterline,
    Technical,
    Wireframe,
}
impl DisplayMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rendered => "Rendered Track",
            Self::RenderedCenterline => "Rendered + Centerline",
            Self::Technical => "Technical Lines",
            Self::Wireframe => "Wireframe / Mesh Inspection",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayOptions {
    pub mode: DisplayMode,
    pub running: bool,
    pub structure: bool,
    pub hardware: bool,
    pub boundaries: bool,
    pub highlights: bool,
    pub grid: bool,
    pub grid_spacing: f64,
    pub frames: bool,
    pub curvature: bool,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub inspection_distance_m: Option<f64>,
    pub centerline: bool,
    pub reference: bool,
    pub offsets: bool,
    pub points: bool,
    pub loads: bool,
}
impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            mode: DisplayMode::RenderedCenterline,
            running: true,
            structure: true,
            hardware: true,
            boundaries: true,
            highlights: true,
            grid: true,
            grid_spacing: 10.,
            frames: false,
            curvature: false,
            selected: None,
            hovered: None,
            inspection_distance_m: None,
            centerline: true,
            reference: false,
            offsets: false,
            points: false,
            loads: false,
        }
    }
}
pub(crate) fn vertices(
    scene: &InspectionScene,
    origin: Vec3,
    display: &DisplayOptions,
) -> Vec<Vertex> {
    let mut v = vec![];
    let mut line = |a: Vec3, b: Vec3, color: [f32; 3]| {
        for p in [a, b] {
            let p = p - origin;
            v.push(Vertex {
                position: [p.x as f32, p.y as f32, p.z as f32],
                color,
            });
        }
    };
    let (min, max) = scene.bounds(None);
    let scale = ((max - min).length() * 0.012).clamp(0.1, 10.);
    if display.grid {
        let step = display.grid_spacing;
        let cx = (origin.x / step).round() * step;
        let cz = (origin.z / step).round() * step;
        for i in -GRID_HALF_LINES..=GRID_HALF_LINES {
            let offset = i as f64 * step;
            line(
                Vec3::new(cx + offset, 0., cz - 30. * step),
                Vec3::new(cx + offset, 0., cz + 30. * step),
                if display.mode == DisplayMode::Technical {
                    [0.035, 0.045, 0.055]
                } else {
                    [0.025, 0.033, 0.044]
                },
            );
            line(
                Vec3::new(cx - 30. * step, 0., cz + offset),
                Vec3::new(cx + 30. * step, 0., cz + offset),
                if display.mode == DisplayMode::Technical {
                    [0.035, 0.045, 0.055]
                } else {
                    [0.025, 0.033, 0.044]
                },
            );
        }
    }
    for (axis, color) in [
        (Vec3::X, [0.9, 0.25, 0.2]),
        (Vec3::Y, [0.25, 0.9, 0.35]),
        (Vec3::Z, [0.25, 0.5, 1.]),
    ] {
        line(Vec3::ZERO, axis * scale * 10., color);
    }
    let colors = [
        [0.35, 0.8, 0.95],
        [0.95, 0.65, 0.3],
        [0.6, 0.85, 0.4],
        [0.8, 0.6, 0.9],
    ];
    for (i, s) in scene.track.sections.iter().enumerate() {
        let color = if display.selected == Some(i) && display.highlights {
            [1., 1., 1.]
        } else if display.hovered == Some(i) {
            [0.95, 0.8, 0.4]
        } else if display.selected.is_some() {
            [0.4, 0.58, 0.68]
        } else {
            colors[i % colors.len()]
        };
        if display.centerline && display.mode != DisplayMode::Rendered {
            for pair in scene.track.samples[s.start_sample..=s.end_sample].windows(2) {
                line(pair[0].position_m, pair[1].position_m, color);
            }
        }
        for index in [s.start_sample, s.end_sample]
            .into_iter()
            .filter(|_| display.boundaries)
        {
            let sample = &scene.track.samples[index];
            let marker_scale = if display.selected == Some(i) && display.highlights {
                ((s.end_distance_m - s.start_distance_m) * 0.01).clamp(0.001, 3.)
            } else {
                scale
            };
            for axis in [sample.frame.normal(), sample.frame.lateral()] {
                line(
                    sample.position_m - axis * marker_scale,
                    sample.position_m + axis * marker_scale,
                    [1., 1., 1.],
                );
            }
            if display.selected == Some(i) && display.highlights {
                let end = sample.position_m + sample.frame.tangent() * marker_scale * 3.;
                line(sample.position_m, end, [0.9, 0.65, 0.3]);
                for side in [-1., 1.] {
                    line(
                        end,
                        end - sample.frame.tangent() * marker_scale
                            + sample.frame.normal() * marker_scale * 0.5 * side,
                        [0.9, 0.65, 0.3],
                    );
                }
                line(
                    sample.position_m,
                    sample.position_m + sample.frame.normal() * marker_scale * 2.,
                    [0.35, 0.85, 0.6],
                );
            }
        }
    }
    if let Some(distance) = display.inspection_distance_m
        && let Ok(sample) = scene.track.sample_at_distance(distance)
    {
        for axis in [
            sample.frame.normal(),
            sample.frame.lateral(),
            sample.frame.tangent(),
        ] {
            line(
                sample.position_m - axis * scale * 2.,
                sample.position_m + axis * scale * 2.,
                [1., 0.8, 0.25],
            );
        }
    }
    if let Some(reference) = &scene.reference {
        if display.reference {
            for pair in reference.points.windows(2) {
                if pair[1].source_distance_m > pair[0].source_distance_m {
                    line(pair[0].position_m, pair[1].position_m, [0.95, 0.78, 0.35]);
                }
            }
        }
        let stride = (reference.points.len() / 100).max(1);
        for point in reference.points.iter().step_by(stride) {
            if display.offsets
                && let Ok(source) = scene.track.sample_at_distance(point.source_distance_m)
            {
                line(source.position_m, point.position_m, [0.65, 0.65, 0.65]);
            }
            if display.points {
                for axis in [point.frame.normal(), point.frame.lateral()] {
                    line(
                        point.position_m - axis * scale * 0.25,
                        point.position_m + axis * scale * 0.25,
                        [1., 0.9, 0.5],
                    );
                }
            }
            if display.loads {
                let load =
                    point.frame.normal() * point.normal_g + point.frame.lateral() * point.lateral_g;
                line(
                    point.position_m,
                    point.position_m + load * scale,
                    [0.9, 0.5, 0.4],
                );
            }
        }
    }
    if display.frames {
        let stride = (scene.track.samples.len() / 180).max(1);
        for sample in scene.track.samples.iter().step_by(stride) {
            for (axis, color) in [
                (sample.frame.tangent(), [0.9, 0.25, 0.2]),
                (sample.frame.normal(), [0.25, 0.9, 0.35]),
                (sample.frame.lateral(), [0.25, 0.5, 1.]),
            ] {
                line(
                    sample.position_m,
                    sample.position_m + axis * scale * 2.,
                    color,
                );
            }
        }
    }
    if display.curvature {
        let stride = (scene.track.graphs.len() / 250).max(1);
        for (i, g) in scene.track.graphs.iter().enumerate().step_by(stride) {
            if let Some(n) = g.frenet_normal {
                let p = (scene.track.samples[i].position_m + scene.track.samples[i + 1].position_m)
                    * 0.5;
                line(
                    p,
                    p + n * (g.curvature_per_m * scale * 50.).min(scale * 5.),
                    [1., 0.5, 0.25],
                );
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use qx_geometry::{
        authoring::{InitialConditions, Section},
        solver::{GenerationOptions, generate},
    };
    fn scene() -> InspectionScene {
        InspectionScene {
            mesh: None,
            mesh_ms: 0.,
            solver_ms: 0.,
            name: "picking fixture".into(),
            track: generate(
                &InitialConditions::default(),
                &[
                    Section::straight(1, "Short", 10.),
                    Section::straight(2, "Long", 100.),
                ],
                GenerationOptions::default(),
            )
            .unwrap(),
            labels: vec![],
            reference: None,
            authored_reference: qx_geometry::heartline::AuthoredReference::TrackCenterline,
        }
    }
    #[test]
    fn bounds_and_projected_picking_resolve_the_same_section_and_station() {
        let s = scene();
        assert_eq!(
            s.visible_bounds(Some(0), false),
            (Vec3::ZERO, Vec3::X * 10.)
        );
        assert!((s.visible_bounds(None, false).1.x - 110.).abs() < 1e-9);
        let viewport = egui::Rect::from_min_size(egui::pos2(250., 100.), egui::vec2(800., 400.));
        let mut camera = crate::camera::Camera::default();
        let (min, max) = s.visible_bounds(None, false);
        camera.fit(min, max, 2.);
        for station in [5., 40., 100.] {
            let pos = camera
                .project(Vec3::X * station, Vec3::ZERO, viewport)
                .unwrap();
            let (id, picked) = s.pick(&camera, Vec3::ZERO, viewport, pos).unwrap();
            assert_eq!(id, SectionId(if station < 10. { 1 } else { 2 }));
            assert!((picked - station).abs() < 1e-3, "{picked} versus {station}");
        }
        assert!(
            s.pick(&camera, Vec3::ZERO, viewport, egui::Pos2::ZERO)
                .is_none()
        );
        // Fitting a short selection remains independent of the long neighbor.
        camera.fit(Vec3::ZERO, Vec3::X * 10., 2.);
        let a = camera.project(Vec3::ZERO, Vec3::ZERO, viewport).unwrap();
        let b = camera.project(Vec3::X * 10., Vec3::ZERO, viewport).unwrap();
        assert!(a.distance(b) > viewport.width() * 0.4);
    }
    #[test]
    fn hidden_rider_reference_does_not_expand_track_framing() {
        let mut s = scene();
        s.reference = Some(
            qx_geometry::heartline::generate_reference(
                &s.track,
                &qx_geometry::heartline::OffsetProfile::Constant {
                    offset_m: Vec3::Y * 100.,
                },
                s.authored_reference,
                9.80665,
            )
            .unwrap(),
        );
        assert_eq!(s.visible_bounds(None, false).1.y, 0.);
        assert!((s.visible_bounds(None, true).1.y - 100.).abs() < 1e-9);
        assert!((s.visible_bounds(Some(0), true).1.x - 10.).abs() < 1e-9);
        // Every selection/inspection/endpoint overlay is derived line data.
        let before = s.track.clone();
        assert!(
            vertices(
                &s,
                Vec3::ZERO,
                &DisplayOptions {
                    selected: Some(0),
                    inspection_distance_m: Some(5.),
                    ..Default::default()
                }
            )
            .len()
                > 100
        );
        assert_eq!(s.track, before);
    }
}
