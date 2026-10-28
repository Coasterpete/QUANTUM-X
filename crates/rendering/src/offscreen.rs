//! Noninteractive GPU verification of the actual editor controls and line shader.
//! No native window, event injection, desktop capture, or desktop input is used.
use crate::{
    DisplayOptions, EditorSurface, InspectionScene, camera::Camera, gpu::line_pipeline,
    scene::vertices,
};
use std::{
    io::BufWriter,
    path::Path,
    sync::{Arc, mpsc},
};
use wgpu::util::DeviceExt;

pub fn render_offscreen(
    scene: &InspectionScene,
    surface: &mut dyn EditorSurface,
    path: &Path,
    width: u32,
    height: u32,
) -> Result<String, String> {
    if !(640..=4096).contains(&width) || !(480..=4096).contains(&height) {
        return Err("offscreen size must be 640..4096 by 480..4096".into());
    }
    pollster::block_on(render(scene, surface, path, width, height))
}
async fn render(
    scene: &InspectionScene,
    surface: &mut dyn EditorSurface,
    path: &Path,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|e| e.to_string())?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|e| e.to_string())?;
    let (error_tx, error_rx) = mpsc::channel();
    device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
        let _ = error_tx.send(e.to_string());
    }));
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let pipeline = line_pipeline(&device, format, true);
    let grid_pipeline = line_pipeline(&device, format, false);
    let texture_descriptor = wgpu::TextureDescriptor {
        label: Some("headless editor verification"),
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
    let target = device.create_texture(&texture_descriptor);
    let view = target.create_view(&Default::default());
    let depth = device
        .create_texture(&wgpu::TextureDescriptor {
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            ..texture_descriptor
        })
        .create_view(&Default::default());
    let ctx = egui::Context::default();
    let mut display = DisplayOptions::default();
    let mut viewport = egui::Rect::NOTHING;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, Default::default());
    let mut output = None;
    let mut pending_free = vec![];
    // Font installation, nested command rows and dock sizing settle on successive passes.
    for frame in 0..4 {
        let mut next = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                )),
                ..Default::default()
            },
            |ctx| {
                let result = surface.ui(ctx, scene, &mut display);
                viewport = result.viewport;
                if frame == 0 && result.profile == 1 {
                    display.reference = true;
                    display.offsets = true;
                }
            },
        );
        for (id, deltas) in &next.textures_delta.set {
            for delta in deltas {
                renderer.update_texture(&device, &queue, *id, delta);
            }
        }
        pending_free.extend(next.textures_delta.free.iter().copied());
        next.textures_delta.clear();
        output = Some(next);
    }
    let output = output.unwrap();
    if viewport.width() < 1. || viewport.height() < 1. {
        return Err("editor has no visible 3D viewport at this size".into());
    }
    let paint = ctx.tessellate(output.shapes, output.pixels_per_point);
    let (min, max) = display
        .train_bounds
        .unwrap_or_else(|| scene.display_bounds(None, &display));
    let origin = (min + max) * 0.5;
    let mut camera = Camera::default();
    camera.fit(min, max, viewport.width() as f64 / viewport.height() as f64);
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(
            &camera.matrix(origin, viewport.width() as f64 / viewport.height() as f64),
        ),
        usage: wgpu::BufferUsages::UNIFORM,
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
        label: Some("headless grid camera"),
        layout: &grid_pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let mut mesh = crate::mesh_gpu::MeshGpu::new(&device, format);
    mesh.upload(&device, &queue, scene, origin)?;
    mesh.upload(&device, &queue, scene, origin)?;
    if scene.mesh.is_some() && mesh.uploads != 1 {
        return Err("unchanged mesh was uploaded twice".into());
    }
    mesh.camera(
        &queue,
        &camera,
        display
            .selected
            .and_then(|i| scene.track.sections.get(i))
            .map(|r| (r.start_distance_m, r.end_distance_m)),
        &display,
        origin,
        viewport.width() as f64 / viewport.height() as f64,
    );
    let mut assets = crate::asset_gpu::AssetGpu::new(&device, format);
    assets.upload(&device, &queue, &display.asset_instances, origin)?;
    let asset_upload_ms = assets.upload_ms;
    assets.upload(&device, &queue, &display.asset_instances, origin)?;
    let unique = display
        .asset_instances
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            !display.asset_instances[..*i]
                .iter()
                .any(|b| Arc::ptr_eq(&a.model, &b.model))
        })
        .count();
    if assets.uploads != unique {
        return Err("Repeated train assets duplicated GPU resources".into());
    }
    assets.camera(
        &queue,
        &camera,
        origin,
        viewport.width() as f64 / viewport.height() as f64,
    );
    let lines = vertices(scene, origin, &display);
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&lines),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [width, height],
        pixels_per_point: output.pixels_per_point,
    };
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut commands = renderer.update_buffers(&device, &queue, &mut encoder, &paint, &screen);
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("headless geometry"),
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
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.set_viewport(
            viewport.left(),
            viewport.top(),
            viewport.width(),
            viewport.height(),
            0.,
            1.,
        );
        pass.set_scissor_rect(
            viewport.left() as u32,
            viewport.top() as u32,
            viewport.width() as u32,
            viewport.height() as u32,
        );
        let grid_count = crate::scene::grid_vertex_count(&display);
        pass.set_pipeline(&grid_pipeline);
        pass.set_bind_group(0, &grid_bind, &[]);
        pass.draw(0..grid_count, 0..1);
        mesh.draw(&mut pass, &display);
        assets.draw(&mut pass);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.set_pipeline(if display.mode == crate::DisplayMode::Technical {
            &pipeline
        } else {
            &grid_pipeline
        });
        pass.set_bind_group(
            0,
            if display.mode == crate::DisplayMode::Technical {
                &bind
            } else {
                &grid_bind
            },
            &[],
        );
        pass.draw(grid_count..lines.len() as u32, 0..1);
    }
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("headless egui"),
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
        renderer.render(&mut pass.forget_lifetime(), &paint, &screen);
    }
    let row = (width * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen readback"),
        size: row as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    commands.push(encoder.finish());
    queue.submit(commands);
    for id in pending_free {
        renderer.free_texture(&id);
    }
    let (tx, rx) = mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|e| e.to_string())?;
    rx.recv_timeout(std::time::Duration::from_secs(1))
        .map_err(|e| e.to_string())??;
    if let Ok(error) = error_rx.try_recv() {
        return Err(error);
    }
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let pixels: Vec<u8> = mapped
        .chunks(row as usize)
        .flat_map(|chunk| chunk[..width as usize * 4].iter().copied())
        .collect();
    let lit_pixels = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|rgba| rgba[0] > 100 || rgba[1] > 100 || rgba[2] > 100)
        .count();
    if lit_pixels < 100 {
        return Err("offscreen render produced no meaningful visible content".into());
    }
    // A selected isolated straight must stay visible across the coplanar grid.
    // This reads actual GPU pixels, so it catches depth fighting that a finite
    // camera matrix or numerical geometry test cannot detect.
    if display.centerline
        // Train-focused captures intentionally let opaque bodies occlude the
        // reference centerline. This grid-depth check applies to track inspection.
        && display.train_bounds.is_none()
        && display.mode != crate::DisplayMode::Rendered
        && display.selected == Some(0)
        && scene.track.sections.len() == 1
        && scene.track.graphs.iter().all(|g| g.curvature_per_m < 1e-12)
    {
        let projection = camera.projection(origin, viewport);
        let mut tested = 0;
        let mut visible = 0;
        for sample in scene
            .track
            .samples
            .iter()
            .step_by((scene.track.samples.len() / 64).max(1))
        {
            let Some(p) = projection
                .project(sample.position_m)
                .filter(|p| viewport.shrink(3.).contains(*p))
            else {
                continue;
            };
            tested += 1;
            let hit = (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let x = (p.x as i32 + dx).clamp(0, width as i32 - 1) as usize;
                    let y = (p.y as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let pixel = &pixels[(y * width as usize + x) * 4..][..3];
                    pixel.iter().all(|c| *c > 220)
                })
            });
            if hit {
                visible += 1;
            }
        }
        if tested > 0 && visible * 10 < tested * 9 {
            return Err(format!(
                "coplanar straight visibility regression: {visible}/{tested} projected samples are visible"
            ));
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut png = png::Encoder::new(BufWriter::new(file), width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    let projected: Vec<_> = display
        .asset_instances
        .iter()
        .map(|a| {
            let (min, max) = a.bounds();
            let projection = camera.projection(origin, viewport);
            let mut points = Vec::new();
            for x in [min.x, max.x] {
                for y in [min.y, max.y] {
                    for z in [min.z, max.z] {
                        if let Some(p) = projection.project(qx_geometry::math::Vec3::new(x, y, z)) {
                            points.push([p.x, p.y]);
                        }
                    }
                }
            }
            serde_json::json!({"component":a.component_id,"corners":points})
        })
        .collect();
    let metadata = serde_json::json!({"viewport":[viewport.left(),viewport.top(),viewport.width(),viewport.height()],"size":[width,height],"train_fit":display.train_bounds.is_some(),"camera_target":camera.target.to_array(),"camera_distance":camera.distance,"unique_glb_resources":assets.uploads,"unique_glb_textures":assets.texture_uploads,"asset_upload_ms":asset_upload_ms,"primitive_instances":assets.instance_count,"projected_bounds":projected});
    std::fs::write(
        path.with_extension("capture.json"),
        serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "{} {:?} / {}; {} line vertices, {} visible pixels; {}x{}; GPU upload {:.3} ms, {} bytes, {} uploads (second unchanged upload reused); GLB {} unique GPU resources / {} primitive instances / {} textures, asset upload {:.3} ms",
        info.name,
        info.backend,
        info.driver_info,
        lines.len(),
        lit_pixels,
        width,
        height,
        mesh.upload_ms,
        mesh.upload_bytes,
        mesh.uploads,
        assets.uploads,
        assets.instance_count,
        assets.texture_uploads,
        asset_upload_ms
    ))
}
