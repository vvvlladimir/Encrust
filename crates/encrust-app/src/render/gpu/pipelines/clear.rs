//! The clear-resin view's pipelines: the thickness pass and the models drawn against it.

use super::super::clear::{Clear, THICKNESS_FORMAT};
use super::*;

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
        let thickness = Facing {
            upright: self.thickness(wgpu::FrontFace::Ccw),
            mirrored: self.thickness(wgpu::FrontFace::Cw),
        };
        Clear::new(thickness, models, layout)
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
