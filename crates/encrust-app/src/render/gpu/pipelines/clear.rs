//! The clear-resin view's pipelines: the thickness pass and the models drawn against it.

use super::super::clear::{Clear, EXIT_FORMAT, THICKNESS_FORMAT};
use super::models::STUDIO_LIGHT;
use super::*;

/// A plane the clear models read, filtered where it is sampled between texels.
fn plane(binding: u32, filterable: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Every surface's signed distance added up, whatever stands in front of it.
const ADDING: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::REPLACE,
};

impl Builder<'_> {
    pub(in super::super) fn clear(&self) -> Clear {
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("viewport_thickness_layout"),
                entries: &[
                    plane(0, false),
                    plane(1, true),
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    plane(3, false),
                ],
            });
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viewport_behind"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let reading = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport_clear"),
                bind_group_layouts: &[Some(self.globals_layout), Some(&layout)],
                immediate_size: 0,
            });
        // Front faces only: the back of the model is what the thickness already counted.
        let models = self.facing_on(
            &reading,
            PipelineKind {
                label: "viewport_clear_models",
                vertex_entry: "model_vertex",
                fragment_entry: "clear_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                cull: Some(wgpu::Face::Back),
                ..PipelineKind::default()
            },
        );
        let glowing = self.shaded(
            &reading,
            PipelineKind {
                label: "viewport_glowing_models",
                vertex_entry: "model_vertex",
                fragment_entry: "glow_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                ..PipelineKind::default()
            },
            STUDIO_LIGHT,
        );
        let thickness = Facing {
            upright: self.thickness(wgpu::FrontFace::Ccw),
            mirrored: self.thickness(wgpu::FrontFace::Cw),
        };
        let exit = Facing {
            upright: self.exit(wgpu::FrontFace::Ccw),
            mirrored: self.exit(wgpu::FrontFace::Cw),
        };
        // Every face, none hiding another, faintly: what the clear models then refract.
        let inner = self.facing(PipelineKind {
            label: "viewport_clear_inside",
            vertex_entry: "model_vertex",
            fragment_entry: "inner_fragment",
            buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
            depth_write: false,
            ..PipelineKind::default()
        });
        Clear::new(
            [thickness, exit, inner, models],
            glowing,
            layout,
            sampler,
            self.target_format,
        )
    }

    /// The render view's room behind everything: a floor and a sky worked out per pixel.
    pub(in super::super) fn backdrop(&self) -> wgpu::RenderPipeline {
        self.plain(PipelineKind {
            label: "viewport_backdrop",
            vertex_entry: "screen_vertex",
            fragment_entry: "backdrop_fragment",
            depth_write: false,
            depth_compare: wgpu::CompareFunction::Always,
            ..PipelineKind::default()
        })
    }

    /// The nearest back face of each pixel, its normal and distance kept, no blending.
    fn exit(&self, front_face: wgpu::FrontFace) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("viewport_exit"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("model_vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("exit_fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: EXIT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face,
                    cull_mode: Some(wgpu::Face::Front),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview_mask: None,
            })
    }

    fn thickness(&self, front_face: wgpu::FrontFace) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("viewport_thickness"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("model_vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("thickness_fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: THICKNESS_FORMAT,
                        blend: Some(ADDING),
                        write_mask: wgpu::ColorWrites::RED,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    front_face,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview_mask: None,
            })
    }
}
