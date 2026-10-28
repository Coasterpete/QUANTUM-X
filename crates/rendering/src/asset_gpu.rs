//! Static mesh/texture resources are shared across node and car instances.
use crate::{
    asset::{AssetInstance, Model, Vertex},
    camera::Camera,
};
use glam::{DMat3, DMat4, DVec3};
use qx_geometry::math::Vec3;
use std::sync::Arc;
use wgpu::util::DeviceExt;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    matrix: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Material {
    color: [f32; 4],
    properties: [f32; 4],
    double_sided: [f32; 4],
}
struct Primitive {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    material: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_count: u32,
}
struct Resource {
    source: Arc<Model>,
    primitives: Vec<Primitive>,
}
pub(crate) struct AssetGpu {
    pipeline: wgpu::RenderPipeline,
    camera: wgpu::Buffer,
    bind: wgpu::BindGroup,
    resources: Vec<Resource>,
    material_layout: wgpu::BindGroupLayout,
    pub uploads: usize,
    pub instance_count: usize,
    pub texture_uploads: usize,
    pub upload_ms: f64,
}
impl AssetGpu {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("asset camera"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GLB material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&camera_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rigid GLB instances"),
            source: wgpu::ShaderSource::Wgsl(include_str!("asset.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("GLB basic opaque/mask materials"), layout:Some(&layout),
            vertex:wgpu::VertexState { module:&shader, entry_point:Some("vertex"), compilation_options:Default::default(), buffers:&[
                Some(wgpu::VertexBufferLayout { array_stride:std::mem::size_of::<Vertex>() as u64, step_mode:wgpu::VertexStepMode::Vertex, attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2] }),
                Some(wgpu::VertexBufferLayout { array_stride:std::mem::size_of::<Instance>() as u64, step_mode:wgpu::VertexStepMode::Instance, attributes:&wgpu::vertex_attr_array![3=>Float32x4,4=>Float32x4,5=>Float32x4,6=>Float32x4,7=>Float32x4,8=>Float32x4,9=>Float32x4] })
            ] },
            primitive:wgpu::PrimitiveState { cull_mode:None,..Default::default() },
            depth_stencil:Some(wgpu::DepthStencilState { format:wgpu::TextureFormat::Depth32Float, depth_write_enabled:Some(true), depth_compare:Some(wgpu::CompareFunction::LessEqual), stencil:Default::default(), bias:Default::default() }),
            multisample:Default::default(), fragment:Some(wgpu::FragmentState { module:&shader, entry_point:Some("fragment"), compilation_options:Default::default(), targets:&[Some(wgpu::ColorTargetState { format, blend:None, write_mask:wgpu::ColorWrites::ALL })] }), multiview_mask:None, cache:None
        });
        Self {
            pipeline,
            camera,
            bind,
            resources: vec![],
            material_layout,
            uploads: 0,
            instance_count: 0,
            texture_uploads: 0,
            upload_ms: 0.,
        }
    }
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        assets: &[AssetInstance],
        origin: Vec3,
    ) -> Result<(), String> {
        let start = std::time::Instant::now();
        self.resources
            .retain(|r| assets.iter().any(|a| Arc::ptr_eq(&r.source, &a.model)));
        for a in assets {
            if self
                .resources
                .iter()
                .any(|r| Arc::ptr_eq(&r.source, &a.model))
            {
                continue;
            }
            let mut primitives = vec![];
            let mut textures = std::collections::HashMap::new();
            for p in &a.model.primitives {
                let buffer = |bytes: &[u8], usage| -> Result<wgpu::Buffer, String> {
                    if bytes.len() as u64 > device.limits().max_buffer_size {
                        return Err("GLB exceeds GPU buffer limit".into());
                    }
                    Ok(
                        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("shared GLB mesh"),
                            contents: bytes,
                            usage,
                        }),
                    )
                };
                let vertices = buffer(
                    bytemuck::cast_slice(&p.vertices),
                    wgpu::BufferUsages::VERTEX,
                )?;
                let indices = buffer(bytemuck::cast_slice(&p.indices), wgpu::BufferUsages::INDEX)?;
                let m = &p.material;
                let key = m.texture.as_ref().map_or(std::ptr::null(), Arc::as_ptr);
                if let std::collections::hash_map::Entry::Vacant(entry) = textures.entry(key) {
                    let (width, height, data) =
                        m.texture.as_ref().map_or((1, 1, &[255u8; 4][..]), |t| {
                            (t.width, t.height, &t.rgba[..])
                        });
                    if width.max(height) > device.limits().max_texture_dimension_2d {
                        return Err("GLB texture exceeds this GPU's dimension limit".into());
                    }
                    let size = wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    };
                    let texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("GLB sRGB base color"),
                        size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        texture.as_image_copy(),
                        data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(width * 4),
                            rows_per_image: Some(height),
                        },
                        size,
                    );
                    let wrap = |v| match v {
                        gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
                        gltf::texture::WrappingMode::MirroredRepeat => {
                            wgpu::AddressMode::MirrorRepeat
                        }
                        _ => wgpu::AddressMode::Repeat,
                    };
                    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                        address_mode_u: m
                            .texture
                            .as_ref()
                            .map_or(wgpu::AddressMode::Repeat, |t| wrap(t.wrap_s)),
                        address_mode_v: m
                            .texture
                            .as_ref()
                            .map_or(wgpu::AddressMode::Repeat, |t| wrap(t.wrap_t)),
                        mag_filter: if m.texture.as_ref().is_some_and(|t| t.nearest) {
                            wgpu::FilterMode::Nearest
                        } else {
                            wgpu::FilterMode::Linear
                        },
                        min_filter: wgpu::FilterMode::Linear,
                        ..Default::default()
                    });
                    entry.insert((texture.create_view(&Default::default()), sampler));
                    self.texture_uploads += 1;
                }
                let (texture, sampler) = &textures[&key];
                let uniform = buffer(
                    bytemuck::bytes_of(&Material {
                        color: m.color,
                        properties: [m.metallic, m.roughness, m.alpha_cutoff.unwrap_or(-1.), 0.],
                        double_sided: [if m.double_sided { 1. } else { 0. }, 0., 0., 0.],
                    }),
                    wgpu::BufferUsages::UNIFORM,
                )?;
                let material = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(texture),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: uniform.as_entire_binding(),
                        },
                    ],
                });
                let instances = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("GLB rigid placements"),
                    size: 112,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                primitives.push(Primitive {
                    vertices,
                    indices,
                    count: p.indices.len() as u32,
                    material,
                    instances,
                    instance_count: 0,
                });
            }
            self.resources.push(Resource {
                source: a.model.clone(),
                primitives,
            });
            self.uploads += 1;
        }
        self.instance_count = 0;
        for r in &mut self.resources {
            let mut placements = vec![vec![]; r.primitives.len()];
            for a in assets.iter().filter(|a| Arc::ptr_eq(&a.model, &r.source)) {
                for node in &r.source.nodes {
                    let mut m = DMat4::from_cols_array_2d(&a.transform)
                        * DMat4::from_cols_array_2d(&node.transform);
                    m.w_axis -= DVec3::new(origin.x, origin.y, origin.z).extend(0.);
                    let normal = DMat3::from_mat4(m).inverse().transpose();
                    let sign = m.determinant().signum() as f32;
                    let normal = normal
                        .to_cols_array_2d()
                        .map(|v| [v[0] as f32, v[1] as f32, v[2] as f32, sign]);
                    let matrix = m.as_mat4().to_cols_array_2d();
                    if matrix
                        .iter()
                        .flatten()
                        .chain(normal.iter().flatten())
                        .any(|v| !v.is_finite())
                    {
                        return Err("Asset transform exceeds GPU precision range".into());
                    }
                    placements[node.primitive].push(Instance { matrix, normal });
                }
            }
            for (p, instances) in r.primitives.iter_mut().zip(placements) {
                let bytes = bytemuck::cast_slice(&instances);
                if bytes.len() as u64 > device.limits().max_buffer_size {
                    return Err("Too many GLB instances for GPU".into());
                }
                if bytes.len() as u64 > p.instances.size() {
                    p.instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("GLB placement growth"),
                        contents: bytes,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    });
                } else if !bytes.is_empty() {
                    queue.write_buffer(&p.instances, 0, bytes);
                }
                p.instance_count = instances.len() as u32;
                self.instance_count += instances.len();
            }
        }
        self.upload_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    pub fn camera(&self, queue: &wgpu::Queue, camera: &Camera, origin: Vec3, aspect: f64) {
        let eye = camera.eye(origin);
        let mut data = Vec::from(camera.matrix(origin, aspect).as_flattened());
        data.extend([eye.x as f32, eye.y as f32, eye.z as f32, 1.]);
        queue.write_buffer(&self.camera, 0, bytemuck::cast_slice(&data));
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        for r in &self.resources {
            for p in &r.primitives {
                if p.instance_count == 0 {
                    continue;
                }
                pass.set_bind_group(1, &p.material, &[]);
                pass.set_vertex_buffer(0, p.vertices.slice(..));
                pass.set_vertex_buffer(1, p.instances.slice(..));
                pass.set_index_buffer(p.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..p.count, 0, 0..p.instance_count);
            }
        }
    }
}
