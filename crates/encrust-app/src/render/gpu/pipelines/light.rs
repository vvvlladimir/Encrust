//! The pipeline that draws the key light's shadow map: depth alone, from the key.

use super::super::light::KEY_FORMAT;
use super::*;

impl Builder<'_> {
    pub(in super::super) fn key_cast(&self) -> Facing {
        Facing {
            upright: self.caster(wgpu::FrontFace::Ccw),
            mirrored: self.caster(wgpu::FrontFace::Cw),
        }
    }

    fn caster(&self, front_face: wgpu::FrontFace) -> wgpu::RenderPipeline {
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("viewport_key_shadow"),
                layout: Some(&self.layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("key_cast_vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    front_face,
                    ..Default::default()
                },
                // Pushed back by the slope of each face, so a lit surface never shades itself.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: KEY_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview_mask: None,
            })
    }
}
