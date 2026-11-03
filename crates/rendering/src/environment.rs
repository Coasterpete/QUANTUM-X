//! Small deterministic procedural sky IBL. No photographs or scene captures.
//! GGX-prefiltered radiance and correlated-Smith split-sum BRDF integration.
use glam::Vec3;
use std::sync::OnceLock;

const SIZE: u32 = 64;
pub(crate) const LEVELS: u32 = 7;
const SAMPLES: u32 = 128;
struct Data {
    faces: Vec<Vec<u8>>,
    brdf: Vec<u8>,
}
pub(crate) struct Environment {
    pub sky: wgpu::TextureView,
    pub brdf: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}
fn sky(direction: Vec3) -> Vec3 {
    if direction.y >= 0. {
        Vec3::new(0.95, 1.1, 1.4).lerp(Vec3::new(0.12, 0.28, 0.65), direction.y.sqrt())
    } else {
        Vec3::splat(0.16).lerp(Vec3::new(0.045, 0.05, 0.06), (-direction.y).sqrt())
    }
}
fn xi(index: u32) -> (f32, f32) {
    (
        index as f32 / SAMPLES as f32,
        index.reverse_bits() as f32 / 4294967296.,
    )
}
fn half_vector(index: u32, rough: f32) -> Vec3 {
    let (u, v) = xi(index);
    let a2 = rough.powi(4);
    let z = ((1. - v) / (1. + (a2 - 1.) * v)).sqrt();
    let radius = (1. - z * z).max(0.).sqrt();
    let phi = std::f32::consts::TAU * u;
    Vec3::new(radius * phi.cos(), radius * phi.sin(), z)
}
fn prefilter(n: Vec3, rough: f32) -> Vec3 {
    if rough == 0. {
        return sky(n);
    }
    let axis = if n.z.abs() < 0.99 { Vec3::Z } else { Vec3::X };
    let t = axis.cross(n).normalize();
    let b = n.cross(t);
    let mut sum = Vec3::ZERO;
    let mut weight = 0.;
    for i in 0..SAMPLES {
        let local = half_vector(i, rough);
        let h = t * local.x + b * local.y + n * local.z;
        let l = 2. * n.dot(h) * h - n;
        let nl = n.dot(l).max(0.);
        sum += sky(l) * nl;
        weight += nl;
    }
    sum / weight.max(1e-6)
}
fn direction(face: u32, u: f32, v: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1., -v, -u),
        1 => Vec3::new(-1., -v, u),
        2 => Vec3::new(u, 1., v),
        3 => Vec3::new(u, -1., -v),
        4 => Vec3::new(u, -v, 1.),
        _ => Vec3::new(-u, -v, -1.),
    }
    .normalize()
}
// All generated values are finite, positive normal binary16 values or zero.
fn half(value: f32) -> u16 {
    if value < 0.000061035156 {
        return 0;
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 255) as i32 - 127 + 15;
    let rounded = (bits & 0x7fffff) + 0x1000;
    ((exponent as u32 * 1024) + (rounded >> 13)) as u16
}
fn pixel(bytes: &mut Vec<u8>, values: [f32; 4]) {
    for value in values {
        bytes.extend_from_slice(&half(value).to_le_bytes());
    }
}
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let mut faces = vec![];
        for level in 0..LEVELS {
            let size = SIZE >> level;
            for face in 0..6 {
                let mut bytes = Vec::with_capacity((size * size * 8) as usize);
                for y in 0..size {
                    for x in 0..size {
                        let n = direction(
                            face,
                            2. * (x as f32 + 0.5) / size as f32 - 1.,
                            2. * (y as f32 + 0.5) / size as f32 - 1.,
                        );
                        let c = prefilter(n, level as f32 / (LEVELS - 1) as f32);
                        pixel(&mut bytes, [c.x, c.y, c.z, 1.]);
                    }
                }
                faces.push(bytes);
            }
        }
        let mut brdf = vec![];
        for y in 0..32 {
            for x in 0..32 {
                let rough = (y as f32 + 0.5) / 32.;
                let nv = (x as f32 + 0.5) / 32.;
                let v = Vec3::new((1. - nv * nv).sqrt(), 0., nv);
                let a2 = rough.powi(4);
                let mut sum = [0.; 2];
                for i in 0..SAMPLES {
                    let h = half_vector(i, rough);
                    let vh = v.dot(h).max(0.);
                    let l = 2. * vh * h - v;
                    let nl = l.z.max(0.);
                    if nl > 0. {
                        let visibility = 0.5
                            / (nl * (nv * nv * (1. - a2) + a2).sqrt()
                                + nv * (nl * nl * (1. - a2) + a2).sqrt())
                            .max(1e-6);
                        let weight = 4. * visibility * nl * vh / h.z.max(1e-6);
                        let fc = (1. - vh).powi(5);
                        sum[0] += (1. - fc) * weight;
                        sum[1] += fc * weight;
                    }
                }
                pixel(
                    &mut brdf,
                    [sum[0] / SAMPLES as f32, sum[1] / SAMPLES as f32, 0., 1.],
                );
            }
        }
        Data { faces, brdf }
    })
}
pub(crate) fn prepare() {
    let _ = data();
}
impl Environment {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let data = data();
        let descriptor = |label, size, layers, levels| wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: layers,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let sky =
            device.create_texture(&descriptor("procedural GGX sky radiance", SIZE, 6, LEVELS));
        for level in 0..LEVELS {
            for face in 0..6 {
                let size = SIZE >> level;
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &sky,
                        mip_level: level,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: face,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &data.faces[(level * 6 + face) as usize],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(size * 8),
                        rows_per_image: Some(size),
                    },
                    wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        let brdf = device.create_texture(&descriptor("integrated correlated GGX BRDF", 32, 1, 1));
        queue.write_texture(
            brdf.as_image_copy(),
            &data.brdf,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32 * 8),
                rows_per_image: Some(32),
            },
            wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            },
        );
        Self {
            sky: sky.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::Cube),
                ..Default::default()
            }),
            brdf: brdf.create_view(&Default::default()),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reflection_filter_is_finite_and_softens_horizon() {
        let a = Vec3::new(1., 0.03, 0.).normalize();
        let b = Vec3::new(1., -0.03, 0.).normalize();
        let sharp = (prefilter(a, 0.) - prefilter(b, 0.)).length();
        let rough = (prefilter(a, 0.8) - prefilter(b, 0.8)).length();
        assert!(sharp > 0.8 && rough < sharp * 0.4, "{sharp} {rough}");
        for face in 0..6 {
            for rough in [0., 0.05, 0.5, 1.] {
                let c = prefilter(direction(face, 0., 0.), rough);
                assert!(c.is_finite() && c.min_element() >= 0.);
            }
        }
        assert_eq!(half(1.), 0x3c00);
        assert_eq!(half(0.5), 0x3800);
        assert_eq!(data().faces.len(), 42);
    }
}
