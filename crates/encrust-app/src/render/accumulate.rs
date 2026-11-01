//! The render view at rest, sharpened over frames: each drawn a fraction of a pixel aside
//! and averaged into a plane, which the copy then shows. See docs/design/viewport.md.

use egui::epaint::ViewportInPixels;

use crate::render::gpu::SAMPLE_COUNT;

/// Half-floats, so sixteen frames average without banding.
const ACCUMULATED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Each frame laid over the plane by a constant share: `1/(n+1)` for the n-th, a running mean.
const AVERAGING: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::OneMinusConstant,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Constant,
        dst_factor: wgpu::BlendFactor::OneMinusConstant,
        operation: wgpu::BlendOperation::Add,
    },
};

/// The plane frames are averaged into, the pipeline that adds one, and the one that shows it.
pub struct Accumulation {
    layout: wgpu::BindGroupLayout,
    adding: wgpu::RenderPipeline,
    showing: wgpu::RenderPipeline,
    plane: Option<Plane>,
    /// Whether the copy shows the plane this frame rather than the scene as drawn.
    shown: bool,
}

struct Plane {
    size_px: [u32; 2],
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl Accumulation {
    /// Built against the frame's layout, whose copy each frame is averaged through.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        frame_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport_accumulated_layout"),
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
        let shader = device.create_shader_module(wgpu::include_wgsl!("present.wgsl"));
        let adding = copy_pipeline(
            device,
            &shader,
            &[Some(frame_layout)],
            ("present_fragment", ACCUMULATED_FORMAT, AVERAGING),
        );
        let showing = copy_pipeline(
            device,
            &shader,
            &[Some(frame_layout), Some(&layout)],
            (
                "accumulated_fragment",
                format,
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            ),
        );
        Self {
            layout,
            adding,
            showing,
            plane: None,
            shown: false,
        }
    }

    /// Averages the frame copied through `frame` into the plane as its `sample`-th, the first
    /// replacing what it held, and shows the plane from now on.
    pub fn add(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::BindGroup,
        (size_px, viewport): ([u32; 2], &ViewportInPixels),
        sample: u32,
    ) {
        if self
            .plane
            .as_ref()
            .is_some_and(|plane| plane.size_px != size_px)
        {
            self.plane = None;
        }
        let layout = &self.layout;
        let plane = self
            .plane
            .get_or_insert_with(|| plane(device, layout, size_px));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport_accumulate"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &plane.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            multiview_mask: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        let share = 1.0 / f64::from(sample + 1);
        pass.set_blend_constant(wgpu::Color {
            r: share,
            g: share,
            b: share,
            a: share,
        });
        pass.set_pipeline(&self.adding);
        pass.set_bind_group(0, frame, &[]);
        pass.draw(0..3, 0..1);
        self.shown = true;
    }

    /// Shows the scene as drawn from now on, until a frame is added again.
    pub fn stop(&mut self) {
        self.shown = false;
    }

    /// Whether the copy shows the plane; `show` lays it when it does.
    pub fn shown(&self) -> bool {
        self.shown && self.plane.is_some()
    }

    pub fn show(&self, render_pass: &mut wgpu::RenderPass<'static>, frame: &wgpu::BindGroup) {
        let Some(plane) = &self.plane else {
            return;
        };
        render_pass.set_pipeline(&self.showing);
        render_pass.set_bind_group(0, frame, &[]);
        render_pass.set_bind_group(1, &plane.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn plane(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size_px: [u32; 2]) -> Plane {
    let view = device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport_accumulated"),
            size: wgpu::Extent3d {
                width: size_px[0].max(1),
                height: size_px[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ACCUMULATED_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport_accumulated"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&view),
        }],
    });
    Plane {
        size_px,
        view,
        bind_group,
    }
}

/// A full-screen pass of `present.wgsl` reading `layouts`: its fragment entry, the format
/// it writes and how it blends.
fn copy_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layouts: &[Option<&wgpu::BindGroupLayout>],
    (entry, format, blend): (&str, wgpu::TextureFormat, wgpu::BlendState),
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry),
        bind_group_layouts: layouts,
        immediate_size: 0,
    });
    let samples = match format {
        ACCUMULATED_FORMAT => 1,
        _ => SAMPLE_COUNT,
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("present_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        cache: None,
        multiview_mask: None,
    })
}
