//! Solid and portable edge rendering; shared meshes and hardware instancing.
use crate::{
    camera::Camera,
    scene::{DisplayMode, DisplayOptions, InspectionScene},
};
use qx_geometry::{construction::TrackMesh, math::Vec3};
use qx_style::construction::Role;
use std::{ops::Range, sync::Arc, time::Instant};
use wgpu::util::DeviceExt;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    station: f32,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    t: [f32; 3],
    n: [f32; 3],
    l: [f32; 3],
    position: [f32; 3],
    color: [f32; 3],
    properties: [f32; 3],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    matrix: [[f32; 4]; 4],
    selection: [f32; 4],
    eye: [f32; 4],
}
struct Batch {
    indices: Range<u32>,
    edges: Range<u32>,
    instances: Range<u32>,
    role: Role,
    visible: bool,
}
pub(crate) struct MeshGpu {
    solid: wgpu::RenderPipeline,
    wire: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    edges: wgpu::Buffer,
    instances: wgpu::Buffer,
    batches: Vec<Batch>,
    source: Option<Arc<TrackMesh>>,
    origin: Vec3,
    pub upload_ms: f64,
    pub upload_bytes: usize,
    pub uploads: u64,
}
fn pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    layout: &wgpu::PipelineLayout,
    wire: bool,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("procedural materials"),
        source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
        label:Some(if wire{"portable indexed mesh edges"}else{"indexed procedural solids"}),layout:Some(layout),
        vertex:wgpu::VertexState{module:&shader,entry_point:Some("vertex"),compilation_options:Default::default(),buffers:&[
            Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Vertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32]}),
            Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Instance>() as u64,step_mode:wgpu::VertexStepMode::Instance,attributes:&wgpu::vertex_attr_array![4=>Float32x3,5=>Float32x3,6=>Float32x3,7=>Float32x3,8=>Float32x3,9=>Float32x3]})]},
        primitive:wgpu::PrimitiveState{topology:if wire{wgpu::PrimitiveTopology::LineList}else{wgpu::PrimitiveTopology::TriangleList},cull_mode:if wire{None}else{Some(wgpu::Face::Back)},front_face:wgpu::FrontFace::Ccw,..Default::default()},
        depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),
        multisample:Default::default(),fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some(if wire{"wire"}else{"fragment"}),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format,blend:None,write_mask:wgpu::ColorWrites::ALL})]}),multiview_mask:None,cache:None })
}
impl MeshGpu {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh camera and selection"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let empty = |usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("reusable mesh buffer"),
                size: 256,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            solid: pipeline(device, format, &layout, false),
            wire: pipeline(device, format, &layout, true),
            bind,
            uniform,
            vertices: empty(wgpu::BufferUsages::VERTEX),
            indices: empty(wgpu::BufferUsages::INDEX),
            edges: empty(wgpu::BufferUsages::INDEX),
            instances: empty(wgpu::BufferUsages::VERTEX),
            batches: vec![],
            source: None,
            origin: Vec3::ZERO,
            upload_ms: 0.,
            upload_bytes: 0,
            uploads: 0,
        }
    }
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &InspectionScene,
        origin: Vec3,
    ) -> Result<(), String> {
        let Some(mesh) = &scene.mesh else {
            self.batches.clear();
            self.source = None;
            return Ok(());
        };
        if self.source.as_ref().is_some_and(|m| Arc::ptr_eq(m, mesh)) && self.origin == origin {
            return Ok(());
        }
        let start = Instant::now();
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut edges = Vec::new();
        let mut instances = Vec::new();
        let mut batches = vec![];
        for c in &mesh.components {
            let material = mesh
                .materials
                .iter()
                .find(|m| m.id == c.material)
                .ok_or("mesh material missing")?;
            let base = vertices.len() as u32;
            let repeated = c.role == Role::Hardware;
            for v in &c.mesh.vertices {
                let p = if repeated {
                    v.position_m
                } else {
                    v.position_m - origin
                };
                let position = f32vec(p)?;
                vertices.push(Vertex {
                    position,
                    normal: f32vec(v.normal)?,
                    uv: [v.uv[0] as f32, v.uv[1] as f32],
                    station: if repeated {
                        0.
                    } else {
                        v.source_station_m as f32
                    },
                });
            }
            let first = indices.len() as u32;
            let first_edge = edges.len() as u32;
            for t in c.mesh.indices.as_chunks::<3>().0.iter() {
                let [a, b, d] = [t[0] + base, t[1] + base, t[2] + base];
                indices.extend_from_slice(&[a, b, d]);
                edges.extend_from_slice(&[a, b, b, d, d, a]);
            }
            let first_instance = instances.len() as u32;
            let instance =
                |p: Vec3, t: Vec3, n: Vec3, l: Vec3, station: f64| -> Result<Instance, String> {
                    Ok(Instance {
                        t: f32vec(t)?,
                        n: f32vec(n)?,
                        l: f32vec(l)?,
                        position: f32vec(p)?,
                        color: material.base_color,
                        properties: [material.metallic, material.roughness, station as f32],
                    })
                };
            if repeated {
                for p in &c.placements {
                    instances.push(instance(
                        p.position_m - origin,
                        p.frame.tangent(),
                        p.frame.normal(),
                        p.frame.lateral(),
                        p.source_station_m,
                    )?);
                }
            } else {
                instances.push(instance(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z, 0.)?);
            }
            batches.push(Batch {
                indices: first..indices.len() as u32,
                edges: first_edge..edges.len() as u32,
                instances: first_instance..instances.len() as u32,
                role: c.role,
                visible: c.visible,
            });
        }
        let data = [
            bytemuck::cast_slice(&vertices),
            bytemuck::cast_slice(&indices),
            bytemuck::cast_slice(&edges),
            bytemuck::cast_slice(&instances),
        ];
        for bytes in data {
            if bytes.len() as u64 > device.limits().max_buffer_size {
                return Err("procedural mesh exceeds GPU buffer limit".into());
            }
        }
        self.upload_bytes = data.iter().map(|d| d.len()).sum();
        for ((buffer, bytes), usage) in [
            &mut self.vertices,
            &mut self.indices,
            &mut self.edges,
            &mut self.instances,
        ]
        .into_iter()
        .zip(data)
        .zip([
            wgpu::BufferUsages::VERTEX,
            wgpu::BufferUsages::INDEX,
            wgpu::BufferUsages::INDEX,
            wgpu::BufferUsages::VERTEX,
        ]) {
            if bytes.is_empty() {
                continue;
            }
            if bytes.len() as u64 > buffer.size() {
                *buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("procedural mesh growth"),
                    contents: bytes,
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                });
            } else {
                queue.write_buffer(buffer, 0, bytes);
            }
        }
        self.batches = batches;
        self.source = Some(mesh.clone());
        self.origin = origin;
        self.uploads += 1;
        self.upload_ms = start.elapsed().as_secs_f64() * 1000.;
        Ok(())
    }
    pub fn camera(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        range: Option<(f64, f64)>,
        display: &DisplayOptions,
        origin: Vec3,
        aspect: f64,
    ) {
        let eye = camera.eye(origin);
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniform {
                matrix: camera.matrix(origin, aspect),
                selection: range.map_or([0.; 4], |r| {
                    [
                        r.0 as f32,
                        r.1 as f32,
                        if display.highlights { 1. } else { 0. },
                        0.,
                    ]
                }),
                eye: [eye.x as f32, eye.y as f32, eye.z as f32, 1.],
            }),
        );
    }
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, display: &DisplayOptions) {
        if display.mode == DisplayMode::Technical {
            return;
        }
        let wire = display.mode == DisplayMode::Wireframe;
        pass.set_pipeline(if wire { &self.wire } else { &self.solid });
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(
            if wire {
                self.edges.slice(..)
            } else {
                self.indices.slice(..)
            },
            wgpu::IndexFormat::Uint32,
        );
        for b in &self.batches {
            if !b.visible
                || !match b.role {
                    Role::Running => display.running,
                    Role::Structure => display.structure,
                    Role::Hardware => display.hardware,
                }
            {
                continue;
            }
            pass.draw_indexed(
                if wire {
                    b.edges.clone()
                } else {
                    b.indices.clone()
                },
                0,
                b.instances.clone(),
            );
        }
    }
}
fn f32vec(v: Vec3) -> Result<[f32; 3], String> {
    let a = [v.x as f32, v.y as f32, v.z as f32];
    if a.iter().all(|x| x.is_finite()) {
        Ok(a)
    } else {
        Err("mesh coordinate cannot be represented on GPU after rebasing".into())
    }
}
