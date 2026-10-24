use crate::{camera::Camera, scene::Vertex};
use qx_geometry::math::Vec3;
use std::sync::{Arc, mpsc};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};
pub(crate) struct Gpu {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
    depth: wgpu::TextureView,
    size: PhysicalSize<u32>,
    errors: mpsc::Receiver<String>,
    egui: egui_wgpu::Renderer,
    pending_free: Vec<egui::TextureId>,
}
impl Gpu {
    pub async fn new(window: Arc<Window>) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("cannot create native surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no compatible DX12/Vulkan adapter: {e}"))?;
        println!("Inspector adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("QUANTUM-X inspector"),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("graphics device unavailable: {e}"))?;
        let (tx, errors) = mpsc::channel();
        let lost = tx.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            let _ = tx.send(format!("graphics error: {e}"));
        }));
        device.set_device_lost_callback(move |reason, message| {
            let _ = lost.send(format!(
                "graphics device lost ({reason:?}): {message}; restart inspector"
            ));
        });
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface has no supported configuration")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities.formats.iter().find(|f| f.is_srgb()) {
            config.format = *format;
        }
        surface.configure(&device, &config);
        let pipeline = line_pipeline(&device, config.format);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera matrix"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 24,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth = Self::depth(&device, &config);
        let egui = egui_wgpu::Renderer::new(&device, config.format, Default::default());
        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform,
            bind,
            vertices,
            count: 0,
            depth,
            size,
            errors,
            egui,
            pending_free: vec![],
        })
    }
    fn depth(device: &wgpu::Device, c: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("inspector depth"),
                size: wgpu::Extent3d {
                    width: c.width,
                    height: c.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }
    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.size = size;
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth = Self::depth(&self.device, &self.config);
        }
    }
    pub fn upload(&mut self, vertices: &[Vertex]) -> Result<(), String> {
        let bytes = bytemuck::cast_slice(vertices);
        if bytes.len() as u64 > self.device.limits().max_buffer_size {
            return Err("inspection lines exceed graphics buffer limit".into());
        }
        if bytes.len() as u64 <= self.vertices.size() {
            self.queue.write_buffer(&self.vertices, 0, bytes);
        } else {
            self.vertices = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("rebased centerline data"),
                    contents: bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
        }
        self.count = vertices.len() as u32;
        Ok(())
    }
    pub fn draw(
        &mut self,
        window: &Arc<Window>,
        camera: &Camera,
        origin: Vec3,
    ) -> Result<(), String> {
        self.draw_ui(window, camera, origin, None)
    }
    pub fn draw_ui(
        &mut self,
        window: &Arc<Window>,
        camera: &Camera,
        origin: Vec3,
        mut ui: Option<GuiFrame>,
    ) -> Result<(), String> {
        if let Some(ui) = &mut ui {
            for (id, deltas) in &ui.textures.set {
                for delta in deltas {
                    self.egui
                        .update_texture(&self.device, &self.queue, *id, delta);
                }
            }
            self.pending_free.extend(ui.textures.free.iter().copied());
            ui.textures.clear();
        }
        if let Ok(error) = self.errors.try_recv() {
            return Err(error);
        }
        if self.size.width == 0 || self.size.height == 0 {
            return Ok(());
        }
        let (frame, reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => (f, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => (f, true),
            wgpu::CurrentSurfaceTexture::Timeout => {
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size);
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(window.clone())
                    .map_err(|e| e.to_string())?;
                self.resize(self.size);
                window.request_redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(
                    "surface validation failed; check graphics driver and restart inspector".into(),
                );
            }
        };
        let viewport = ui.as_ref().map_or(
            [0., 0., self.size.width as f32, self.size.height as f32],
            |u| {
                let p = u.pixels_per_point;
                [
                    u.viewport.min.x * p,
                    u.viewport.min.y * p,
                    (u.viewport.width() * p).max(1.),
                    (u.viewport.height() * p).max(1.),
                ]
            },
        );
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&camera.matrix(origin, viewport[2] as f64 / viewport[3] as f64)),
        );
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: ui.as_ref().map_or(1., |u| u.pixels_per_point),
        };
        let mut buffers = if let Some(ui) = &ui {
            self.egui
                .update_buffers(&self.device, &self.queue, &mut encoder, &ui.jobs, &screen)
        } else {
            vec![]
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("geometry inspection"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.018,
                            g: 0.025,
                            b: 0.035,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            let x = viewport[0].clamp(0., self.size.width.saturating_sub(1) as f32);
            let y = viewport[1].clamp(0., self.size.height.saturating_sub(1) as f32);
            let width = viewport[2].min(self.size.width as f32 - x).max(1.);
            let height = viewport[3].min(self.size.height as f32 - y).max(1.);
            pass.set_viewport(x, y, width, height, 0., 1.);
            pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..self.count, 0..1);
        }
        if let Some(ui) = &ui {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("development editor controls"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.egui
                .render(&mut pass.forget_lifetime(), &ui.jobs, &screen);
        }
        buffers.push(encoder.finish());
        self.queue.submit(buffers);
        for id in self.pending_free.drain(..) {
            self.egui.free_texture(&id);
        }
        window.pre_present_notify();
        self.queue.present(frame);
        if reconfigure {
            self.resize(self.size);
        }
        Ok(())
    }
}
pub(crate) struct GuiFrame {
    pub jobs: Vec<egui::ClippedPrimitive>,
    pub textures: egui::TexturesDelta,
    pub pixels_per_point: f32,
    pub viewport: egui::Rect,
}

pub(crate) fn line_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("debug centerlines"),
        source: wgpu::ShaderSource::Wgsl(include_str!("lines.wgsl").into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("centerline line list"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3],
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
