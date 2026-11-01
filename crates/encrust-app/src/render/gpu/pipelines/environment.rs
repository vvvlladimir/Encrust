//! The pipeline that prefilters one mip of the studio's map.

use super::super::environment::{ENVIRONMENT_FORMAT, Environment};
use super::*;

impl Builder<'_> {
    pub(in super::super) fn environment(&self) -> Environment {
        let mip_layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("viewport_environment_mip_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport_environment"),
                bind_group_layouts: &[Some(self.globals_layout), Some(&mip_layout)],
                immediate_size: 0,
            });
        let prefilter = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("viewport_environment"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("screen_vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("prefilter_fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: ENVIRONMENT_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview_mask: None,
            });
        Environment::new(self.device, prefilter, &mip_layout)
    }
}
