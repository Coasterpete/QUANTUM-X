//! Shared bindings and uploads for procedural and imported mesh pipelines.
use crate::asset::{Material, Texture};
use std::{collections::HashMap, sync::Arc};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    color: [f32; 4],
    properties: [f32; 4],
    normal_ao: [f32; 4],
    uv: [f32; 4],
    metallic_channel: [f32; 4],
    roughness_channel: [f32; 4],
    occlusion_channel: [f32; 4],
    emissive: [f32; 4],
}
pub(crate) struct MaterialGpu {
    pub layout: wgpu::BindGroupLayout,
    // Arc ownership prevents allocator address reuse from aliasing a stale key.
    textures: HashMap<(usize, bool), (Arc<Texture>, wgpu::TextureView, wgpu::Sampler)>,
    fallbacks: [Arc<Texture>; 2],
    mips: crate::texture_mips::MipGenerator,
    pub uploads: usize,
    environment: Option<crate::environment::Environment>,
}
impl MaterialGpu {
    pub fn new(device: &wgpu::Device) -> Self {
        // Generate once during renderer initialization, before interactive frames.
        crate::environment::prepare();
        let mut entries = vec![];
        for i in 0..6 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(wgpu::BindGroupLayoutEntry {
                binding: i * 2 + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 12,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        for (binding, dimension) in [
            (13, wgpu::TextureViewDimension::Cube),
            (15, wgpu::TextureViewDimension::D2),
        ] {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: dimension,
                    multisampled: false,
                },
                count: None,
            });
        }
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 14,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let pixel = |rgba| {
            Arc::new(Texture {
                width: 1,
                height: 1,
                rgba: Vec::from(rgba),
                wrap_s: gltf::texture::WrappingMode::Repeat,
                wrap_t: gltf::texture::WrappingMode::Repeat,
                nearest: false,
            })
        };
        Self {
            layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shared track PBR materials"),
                entries: &entries,
            }),
            textures: HashMap::new(),
            fallbacks: [pixel([255; 4]), pixel([128, 128, 255, 255])],
            mips: crate::texture_mips::MipGenerator::new(device),
            uploads: 0,
            environment: None,
        }
    }
    /// Call only when old bind groups are discarded; bounds cache lifetime to a
    /// scene's resources, while sharing maps across slots and color-space uses.
    pub fn clear(&mut self) {
        self.textures.clear();
    }
    pub fn retain<'a>(&mut self, materials: impl Iterator<Item = &'a Material>) {
        let mut live = std::collections::HashSet::new();
        for material in materials {
            for texture in material.textures().into_iter().flatten() {
                live.insert(Arc::as_ptr(texture) as usize);
            }
        }
        for texture in &self.fallbacks {
            live.insert(Arc::as_ptr(texture) as usize);
        }
        self.textures.retain(|(id, _), _| live.contains(id));
    }
    pub fn bind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material: &Material,
    ) -> Result<wgpu::BindGroup, String> {
        if [
            &material.metallic_map,
            &material.roughness_map,
            &material.occlusion,
        ]
        .into_iter()
        .flatten()
        .any(|m| m.channel > 3)
        {
            return Err("PBR scalar map channel must be R/G/B/A".into());
        }
        let environment = self
            .environment
            .get_or_insert_with(|| crate::environment::Environment::new(device, queue));
        let maps = [
            material.texture.as_ref(),
            material.normal.as_ref(),
            material.metallic_map.as_ref().map(|m| &m.texture),
            material.roughness_map.as_ref().map(|m| &m.texture),
            material.occlusion.as_ref().map(|m| &m.texture),
            material.emissive_map.as_ref(),
        ];
        let mut keys = vec![];
        for (i, map) in maps.into_iter().enumerate() {
            let texture = map.unwrap_or(&self.fallbacks[usize::from(i == 1)]);
            let srgb = i == 0 || i == 5;
            let key = (Arc::as_ptr(texture) as usize, srgb);
            keys.push(key);
            if self.textures.contains_key(&key) {
                continue;
            }
            if texture.width == 0
                || texture.height == 0
                || texture.width.max(texture.height) > device.limits().max_texture_dimension_2d
                || texture.rgba.len() as u64 != texture.width as u64 * texture.height as u64 * 4
            {
                return Err("Invalid PBR texture size or GPU dimension limit exceeded".into());
            }
            let size = wgpu::Extent3d {
                width: texture.width,
                height: texture.height,
                depth_or_array_layers: 1,
            };
            let levels = if texture.nearest {
                1
            } else {
                texture.width.max(texture.height).ilog2() + 1
            };
            let gpu = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(if srgb {
                    "PBR sRGB color"
                } else {
                    "PBR linear data"
                }),
                size,
                mip_level_count: levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: texture_format(srgb),
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            queue.write_texture(
                gpu.as_image_copy(),
                &texture.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texture.width * 4),
                    rows_per_image: Some(texture.height),
                },
                size,
            );
            self.mips.generate(device, queue, &gpu, levels, srgb);
            let wrap = |v| match v {
                gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
                _ => wgpu::AddressMode::Repeat,
            };
            let filter = if texture.nearest {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            };
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wrap(texture.wrap_s),
                address_mode_v: wrap(texture.wrap_t),
                mag_filter: filter,
                min_filter: filter,
                mipmap_filter: if texture.nearest {
                    wgpu::MipmapFilterMode::Nearest
                } else {
                    wgpu::MipmapFilterMode::Linear
                },
                anisotropy_clamp: if texture.nearest { 1 } else { 4 },
                ..Default::default()
            });
            self.textures.insert(
                key,
                (
                    texture.clone(),
                    gpu.create_view(&Default::default()),
                    sampler,
                ),
            );
            if map.is_some() {
                self.uploads += 1;
            }
        }
        let channel = |map: &Option<crate::material_library::ScalarTexture>| {
            let mut c = [0.; 4];
            c[map.as_ref().map_or(0, |m| m.channel)] = 1.;
            c
        };
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PBR parameters"),
            usage: wgpu::BufferUsages::UNIFORM,
            contents: bytemuck::bytes_of(&Uniform {
                color: material.color,
                properties: [
                    material.metallic,
                    material.roughness,
                    material.alpha_cutoff.unwrap_or(-1.),
                    f32::from(material.double_sided),
                ],
                normal_ao: [
                    if material.normal.is_some() {
                        material.normal_scale
                    } else {
                        0.
                    },
                    material.normal_y,
                    material.occlusion_strength,
                    f32::from(material.roughness_map.as_ref().is_some_and(|m| m.invert)),
                ],
                uv: material.uv_transform,
                metallic_channel: channel(&material.metallic_map),
                roughness_channel: channel(&material.roughness_map),
                occlusion_channel: channel(&material.occlusion),
                emissive: [
                    material.emissive[0],
                    material.emissive[1],
                    material.emissive[2],
                    0.,
                ],
            }),
        });
        let mut entries = vec![];
        for (i, key) in keys.iter().enumerate() {
            let (_, view, sampler) = &self.textures[key];
            entries.push(wgpu::BindGroupEntry {
                binding: i as u32 * 2,
                resource: wgpu::BindingResource::TextureView(view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: i as u32 * 2 + 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: 12,
            resource: uniform.as_entire_binding(),
        });
        entries.extend([
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::TextureView(&environment.sky),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: wgpu::BindingResource::Sampler(&environment.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 15,
                resource: wgpu::BindingResource::TextureView(&environment.brdf),
            },
        ]);
        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PBR material"),
            layout: &self.layout,
            entries: &entries,
        }))
    }
}
pub(crate) fn texture_format(srgb: bool) -> wgpu::TextureFormat {
    if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    }
}
pub(crate) fn shader(source: &str) -> String {
    format!("{}\n{source}", include_str!("pbr.wgsl"))
}
