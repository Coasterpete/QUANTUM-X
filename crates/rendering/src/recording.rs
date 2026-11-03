//! Persistent, viewport-only recording through the native mesh/asset/PBR paths.
//! Every call rasterizes a new presentation pose; no video resampling or generated
//! intermediate images. Readback and PNG costs are measured separately from GPU work.
use crate::{DisplayOptions, InspectionScene, camera::Camera, gpu::line_pipeline, scene::vertices};
use qx_geometry::math::Vec3;
use serde::Serialize;
use std::{
    hash::{Hash, Hasher},
    io::BufWriter,
    path::Path,
    sync::{Arc, mpsc},
    time::Instant,
};
use wgpu::util::DeviceExt;

#[derive(Serialize)]
pub struct RecordedFrame {
    pub upload_ms: f64,
    pub encode_submit_ms: f64,
    pub gpu_ms: Option<f64>,
    pub wait_readback_ms: f64,
    pub png_ms: f64,
    pub pixel_hash: u64,
}
pub struct ViewportRecorder {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    depth: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    grid_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    grid_bind: wgpu::BindGroup,
    mesh: crate::mesh_gpu::MeshGpu,
    assets: crate::asset_gpu::AssetGpu,
    selection: crate::selection::SelectionGpu,
    readback: wgpu::Buffer,
    queries: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer)>,
    errors: mpsc::Receiver<String>,
    pub adapter: String,
    width: u32,
    height: u32,
    row: u32,
    origin: Vec3,
}
impl ViewportRecorder {
    pub fn new(scene: &InspectionScene, width: u32, height: u32) -> Result<Self, String> {
        pollster::block_on(Self::create(scene, width, height))
    }
    async fn create(scene: &InspectionScene, width: u32, height: u32) -> Result<Self, String> {
        if !(640..=4096).contains(&width) || !(480..=4096).contains(&height) {
            return Err("recording size outside supported bounds".into());
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let timing = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: if timing {
                    wgpu::Features::TIMESTAMP_QUERY
                } else {
                    wgpu::Features::empty()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let (tx, errors) = mpsc::channel();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            let _ = tx.send(e.to_string());
        }));
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let descriptor = wgpu::TextureDescriptor {
            label: Some("authentic POV recording"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let target = device.create_texture(&descriptor);
        let view = target.create_view(&Default::default());
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                ..descriptor
            })
            .create_view(&Default::default());
        let pipeline = line_pipeline(&device, format, true);
        let grid_pipeline = line_pipeline(&device, format, false);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
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
        let grid_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &grid_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let mesh = crate::mesh_gpu::MeshGpu::new(&device, format);
        let assets = crate::asset_gpu::AssetGpu::new(&device, format);
        let selection =
            crate::selection::SelectionGpu::new(&device, format, [width, height], &depth);
        let row = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: row as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let queries = timing.then(|| {
            (
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("recording GPU frame time"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }),
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 256,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 16,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
            )
        });
        let (min, max) = scene.bounds(None);
        Ok(Self {
            device,
            queue,
            target,
            view,
            depth,
            pipeline,
            grid_pipeline,
            uniform,
            bind,
            grid_bind,
            mesh,
            assets,
            selection,
            readback,
            queries,
            errors,
            adapter: format!("{:?}", adapter.get_info()),
            width,
            height,
            row,
            origin: (min + max) * 0.5,
        })
    }
    pub fn frame(
        &mut self,
        scene: &InspectionScene,
        display: &DisplayOptions,
        path: &Path,
    ) -> Result<RecordedFrame, String> {
        let upload = Instant::now();
        let camera = Camera {
            attached: Some(
                display
                    .camera_view
                    .ok_or("POV recording requires an attached camera")?
                    .validate()?,
            ),
            ..Default::default()
        };
        self.mesh
            .upload(&self.device, &self.queue, scene, self.origin)?;
        self.assets.upload(
            &self.device,
            &self.queue,
            &display.asset_instances,
            self.origin,
        )?;
        let aspect = self.width as f64 / self.height as f64;
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&camera.matrix(self.origin, aspect)),
        );
        self.mesh.camera(
            &self.queue,
            &camera,
            display
                .selected
                .and_then(|i| scene.track.sections.get(i))
                .map(|s| (s.start_distance_m, s.end_distance_m)),
            display,
            self.origin,
            aspect,
        );
        self.assets
            .camera(&self.queue, &camera, self.origin, aspect);
        let lines = vertices(scene, self.origin, display);
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&lines),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let upload_ms = upload.elapsed().as_secs_f64() * 1000.;
        let encoding = Instant::now();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("recorded native PBR geometry"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
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
                timestamp_writes: self
                    .queries
                    .as_ref()
                    .map(|q| wgpu::RenderPassTimestampWrites {
                        query_set: &q.0,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: None,
                    }),
                ..Default::default()
            });
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.set_pipeline(&self.grid_pipeline);
            pass.set_bind_group(0, &self.grid_bind, &[]);
            pass.draw(0..crate::scene::grid_vertex_count(display), 0..1);
            self.mesh.draw(&mut pass, display);
            self.assets.draw(&mut pass);
        }
        self.selection.draw(
            &mut encoder,
            &self.view,
            &self.depth,
            &self.mesh,
            display,
            [0., 0., self.width as f32, self.height as f32],
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("recorded native overlays"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self
                    .queries
                    .as_ref()
                    .map(|q| wgpu::RenderPassTimestampWrites {
                        query_set: &q.0,
                        beginning_of_pass_write_index: None,
                        end_of_pass_write_index: Some(1),
                    }),
                ..Default::default()
            });
            let technical = display.mode == crate::DisplayMode::Technical;
            pass.set_pipeline(if technical {
                &self.pipeline
            } else {
                &self.grid_pipeline
            });
            pass.set_bind_group(
                0,
                if technical {
                    &self.bind
                } else {
                    &self.grid_bind
                },
                &[],
            );
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(
                crate::scene::grid_vertex_count(display)..lines.len() as u32,
                0..1,
            );
        }
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        if let Some((set, resolve, read)) = &self.queries {
            encoder.resolve_query_set(set, 0..2, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, read, 0, 16);
        }
        self.queue.submit([encoder.finish()]);
        let encode_submit_ms = encoding.elapsed().as_secs_f64() * 1000.;
        let waiting = Instant::now();
        let (tx, rx) = mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r.map_err(|e| e.to_string()));
            });
        let timing_receive = self.queries.as_ref().map(|(_, _, read)| {
            let (tx, rx) = mpsc::channel();
            read.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r.map_err(|e| e.to_string()));
            });
            rx
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())??;
        let gpu_ms = if let (Some(rx), Some((_, _, read))) = (timing_receive, &self.queries) {
            rx.recv().map_err(|e| e.to_string())??;
            let mapped = read
                .slice(..)
                .get_mapped_range()
                .map_err(|e| e.to_string())?;
            let ticks: &[u64] = bytemuck::cast_slice(&mapped);
            let ms = (ticks[1] - ticks[0]) as f64 * self.queue.get_timestamp_period() as f64 / 1e6;
            drop(mapped);
            read.unmap();
            Some(ms)
        } else {
            None
        };
        if let Ok(e) = self.errors.try_recv() {
            return Err(e);
        }
        let mapped = self
            .readback
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let pixels: Vec<u8> = mapped
            .chunks(self.row as usize)
            .flat_map(|r| r[..self.width as usize * 4].iter().copied())
            .collect();
        drop(mapped);
        self.readback.unmap();
        let wait_readback_ms = waiting.elapsed().as_secs_f64() * 1000.;
        let png_start = Instant::now();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pixels.hash(&mut hasher);
        let pixel_hash = hasher.finish();
        let file = std::fs::File::create_new(path).map_err(|e| e.to_string())?;
        let mut png = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&pixels)
            .map_err(|e| e.to_string())?;
        Ok(RecordedFrame {
            upload_ms,
            encode_submit_ms,
            gpu_ms,
            wait_readback_ms,
            png_ms: png_start.elapsed().as_secs_f64() * 1000.,
            pixel_hash,
        })
    }
}
