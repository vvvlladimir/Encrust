//! The viewport's own colour, depth and stencil planes. The scene is drawn into them and
//! copied into egui's pass, so the section cap never depends on what egui allocates; see
//! `docs/decisions/0184-the-viewport-draws-into-its-own-target.md`.

use egui::epaint::ViewportInPixels;

use crate::render::gpu::{DEPTH_FORMAT, SAMPLE_COUNT};

/// The planes the scene is drawn into, sized to the window, and the pipeline that copies
/// them out.
pub struct SceneTarget {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    present: wgpu::RenderPipeline,
    planes: Option<Planes>,
}

struct Planes {
    size_px: [u32; 2],
    colour: wgpu::TextureView,
    depth: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl SceneTarget {
    /// `format` is egui's own target format, which the scene's pipelines are built for.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport_frame_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        Self {
            format,
            present: present_pipeline(device, format, &layout),
            layout,
            planes: None,
        }
    }

    /// Begins a cleared pass over the planes, drawing into `viewport` of a window
    /// `size_px` pixels large. The planes are remade whenever the window changes size.
    pub fn begin(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        size_px: [u32; 2],
        viewport: ViewportInPixels,
    ) -> wgpu::RenderPass<'static> {
        if self
            .planes
            .as_ref()
            .is_some_and(|planes| planes.size_px != size_px)
        {
            self.planes = None;
        }
        let planes = self
            .planes
            .get_or_insert_with(|| planes(device, self.format, &self.layout, size_px));
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &planes.colour,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent, so the copy lays the scene over whatever egui
                        // painted behind the viewport, as drawing straight into it did.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &planes.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                multiview_mask: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        pass
    }

    /// Copies the drawn scene into egui's pass, pixel for pixel, inside the viewport egui
    /// has already set for the callback.
    pub fn present(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        let Some(planes) = &self.planes else {
            return;
        };
        render_pass.set_pipeline(&self.present);
        render_pass.set_bind_group(0, &planes.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn planes(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    frame_layout: &wgpu::BindGroupLayout,
    size_px: [u32; 2],
) -> Planes {
    let plane = |label, format, usage| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size_px[0].max(1),
                    height: size_px[1].max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: SAMPLE_COUNT,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    let colour = plane(
        "viewport_colour",
        format,
        wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let depth = plane("viewport_depth", DEPTH_FORMAT, wgpu::TextureUsages::empty());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport_frame"),
        layout: frame_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&colour),
        }],
    });
    Planes {
        size_px,
        colour,
        depth,
        bind_group,
    }
}

/// Lays the planes over egui's pass with the same premultiplied blend the scene was drawn
/// with, so the result is what drawing straight into that pass gave.
fn present_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    frame_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("viewport_present"),
        bind_group_layouts: &[Some(frame_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("viewport_present"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("present_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("present_fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        // egui is asked for no depth buffer: nothing in its pass needs one any more.
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            ..Default::default()
        },
        cache: None,
        multiview_mask: None,
    })
}
