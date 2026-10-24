use qx_geometry::{authoring::SectionId, math::Vec3, solver::GeneratedTrack};
#[derive(Clone, Debug)]
pub struct SectionLabel {
    pub id: SectionId,
    pub name: String,
    pub method: String,
}
pub struct InspectionScene {
    pub name: String,
    pub track: GeneratedTrack,
    pub labels: Vec<SectionLabel>,
}
impl InspectionScene {
    pub(crate) fn bounds(&self, section: Option<usize>) -> (Vec3, Vec3) {
        let samples = if let Some(i) = section {
            let s = &self.track.sections[i];
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
        (min, max)
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}
pub(crate) struct DisplayOptions {
    pub grid: bool,
    pub grid_spacing: f64,
    pub frames: bool,
    pub curvature: bool,
    pub selected: Option<usize>,
}
impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            grid: true,
            grid_spacing: 10.,
            frames: false,
            curvature: false,
            selected: None,
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
        for i in -30..=30 {
            let offset = i as f64 * step;
            line(
                Vec3::new(cx + offset, 0., cz - 30. * step),
                Vec3::new(cx + offset, 0., cz + 30. * step),
                [0.035, 0.045, 0.055],
            );
            line(
                Vec3::new(cx - 30. * step, 0., cz + offset),
                Vec3::new(cx + 30. * step, 0., cz + offset),
                [0.035, 0.045, 0.055],
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
        let color = if display.selected == Some(i) {
            [1., 1., 1.]
        } else if display.selected.is_some() {
            [0.22, 0.3, 0.36]
        } else {
            colors[i % colors.len()]
        };
        for pair in scene.track.samples[s.start_sample..=s.end_sample].windows(2) {
            line(pair[0].position_m, pair[1].position_m, color);
        }
        for index in [s.start_sample, s.end_sample] {
            let sample = &scene.track.samples[index];
            for axis in [sample.frame.normal(), sample.frame.lateral()] {
                line(
                    sample.position_m - axis * scale,
                    sample.position_m + axis * scale,
                    [1., 1., 1.],
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
