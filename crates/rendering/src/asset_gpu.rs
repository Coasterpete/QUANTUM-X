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
    materials: crate::material_gpu::MaterialGpu,
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
        let materials = crate::material_gpu::MaterialGpu::new(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&camera_layout), Some(&materials.layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rigid GLB instances"),
            source: wgpu::ShaderSource::Wgsl(
                crate::material_gpu::shader(include_str!("asset.wgsl")).into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:Some("GLB basic opaque/mask materials"), layout:Some(&layout),
            vertex:wgpu::VertexState { module:&shader, entry_point:Some("vertex"), compilation_options:Default::default(), buffers:&[
                Some(wgpu::VertexBufferLayout { array_stride:std::mem::size_of::<Vertex>() as u64, step_mode:wgpu::VertexStepMode::Vertex, attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,10=>Float32x4] }),
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
            materials,
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
        self.materials.retain(
            assets
                .iter()
                .flat_map(|a| a.model.primitives.iter().map(|p| &p.material)),
        );
        for a in assets {
            if self
                .resources
                .iter()
                .any(|r| Arc::ptr_eq(&r.source, &a.model))
            {
                continue;
            }
            let mut primitives = vec![];

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
                let material = self.materials.bind(device, queue, &p.material)?;
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
        self.texture_uploads = self.materials.uploads;
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
