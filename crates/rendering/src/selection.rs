//! A depth-tested selection mask dilated only into background pixels.
//! Shared by native and offscreen rendering; no material or document mutation.
use crate::{DisplayMode, DisplayOptions, mesh_gpu::MeshGpu};

pub(crate) struct SelectionGpu {
    pipeline: wgpu::RenderPipeline,
    mask: wgpu::TextureView,
    bind: wgpu::BindGroup,
}
impl SelectionGpu {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        depth: &wgpu::TextureView,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("selection mask and scene depth"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("color-preserving selection silhouette"),
            source: wgpu::ShaderSource::Wgsl(include_str!("selection.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("exterior selection silhouette"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
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
        });
        let (mask, bind) = Self::target(device, &layout, size, depth);
        Self {
            pipeline,
            mask,
            bind,
        }
    }
    fn target(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        size: [u32; 2],
        depth: &wgpu::TextureView,
    ) -> (wgpu::TextureView, wgpu::BindGroup) {
        let mask = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("visible selected track pixels"),
                size: wgpu::Extent3d {
                    width: size[0].max(1),
                    height: size[1].max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection silhouette inputs"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&mask),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
            ],
        });
        (mask, bind)
    }
    pub fn resize(&mut self, device: &wgpu::Device, size: [u32; 2], depth: &wgpu::TextureView) {
        (self.mask, self.bind) =
            Self::target(device, &self.pipeline.get_bind_group_layout(0), size, depth);
    }
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        mesh: &MeshGpu,
        display: &DisplayOptions,
        viewport: [f32; 4],
    ) {
        if !display.highlights
            || display.selected.is_none()
            || !matches!(
                display.mode,
                DisplayMode::Rendered | DisplayMode::RenderedCenterline
            )
        {
            return;
        }
        let [x, y, width, height] = viewport;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth-tested selected geometry mask"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.mask,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    // Read-only attachment: the selection pass cannot affect occlusion.
                    depth_ops: None,
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_viewport(x, y, width, height, 0., 1.);
            pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
            mesh.draw_selection_mask(&mut pass, display);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("selection outline outside shaded surfaces"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_viewport(x, y, width, height, 0., 1.);
        pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
