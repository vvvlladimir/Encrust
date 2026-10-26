//! The contact shadow's passes: baked into a texture over the plate, then laid on it.

use super::{Builder, PipelineKind, Textured};
use crate::render::vertex::{ModelInstance, ModelVertex, ReliefVertex};

/// The contact shadow: the passes that bake it into a texture over the plate, and the one
/// that lays it on the plate every frame.
pub(in crate::render::gpu) struct Shadowing {
    /// Lays every model flat onto the texture, from the flat and the textured vertices.
    pub(in crate::render::gpu) silhouette: wgpu::RenderPipeline,
    pub(in crate::render::gpu) silhouette_relief: wgpu::RenderPipeline,
    /// Softens what the silhouette pass laid down into the second texture.
    pub(in crate::render::gpu) blur: wgpu::RenderPipeline,
    /// Darkens the plate by what the blur left.
    pub(in crate::render::gpu) floor: Textured<wgpu::RenderPipeline>,
    /// Fills the plate's surface under everything else.
    pub(in crate::render::gpu) plate: wgpu::RenderPipeline,
}

/// What the shadow is baked into: one channel, enough for how dark the plate goes.
pub(in crate::render::gpu) const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

impl Builder<'_> {
    /// The contact shadow's passes. The floor is drawn over the plate and under nothing
    /// that stands on it, so it takes no depth of its own.
    pub(in crate::render::gpu) fn shadowing(&self) -> Shadowing {
        let floor = self.textured(
            wgpu::TextureViewDimension::D2,
            wgpu::AddressMode::ClampToEdge,
            Self::on,
            PipelineKind {
                label: "viewport_shadow_floor",
                vertex_entry: "floor_vertex",
                fragment_entry: "floor_fragment",
                depth_write: false,
                ..PipelineKind::default()
            },
        );
        let sampled = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport_shadow_blur"),
                bind_group_layouts: &[Some(self.globals_layout), Some(&floor.layout)],
                immediate_size: 0,
            });
        // The nearest surface over a texel decides how dark it goes, whatever order the
        // models arrive in.
        let nearest = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        let bake = |label, layout, vertex_entry, fragment_entry, buffers, blend| {
            baking(
                self.device,
                &self.shader,
                Baking {
                    label,
                    layout,
                    vertex_entry,
                    fragment_entry,
                    buffers,
                    blend,
                },
            )
        };
        Shadowing {
            silhouette: bake(
                "viewport_shadow_silhouette",
                &self.layout,
                "silhouette_vertex",
                "silhouette_fragment",
                &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                Some(nearest),
            ),
            silhouette_relief: bake(
                "viewport_shadow_silhouette_relief",
                &self.layout,
                "silhouette_vertex",
                "silhouette_fragment",
                &[Some(ReliefVertex::layout()), Some(ModelInstance::layout())],
                Some(nearest),
            ),
            blur: bake(
                "viewport_shadow_blur",
                &sampled,
                "blur_vertex",
                "blur_fragment",
                &[],
                None,
            ),
            floor,
            // First of all and taking no depth: the lines over it never tie with it, and a
            // model seen through still shows over it.
            plate: self.plain(PipelineKind {
                label: "viewport_plate",
                vertex_entry: "floor_vertex",
                fragment_entry: "plate_fragment",
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                ..PipelineKind::default()
            }),
        }
    }
}

/// A pass that draws into the shadow's texture rather than the viewport's planes.
struct Baking<'a> {
    label: &'a str,
    layout: &'a wgpu::PipelineLayout,
    vertex_entry: &'a str,
    fragment_entry: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    /// How a texel takes what is drawn over it, or `None` to replace it.
    blend: Option<wgpu::BlendComponent>,
}

/// Both faces drawn and no depth: the shadow wants every surface over a texel, and the
/// blend is what picks among them.
fn baking(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    kind: Baking<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(kind.label),
        layout: Some(kind.layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(kind.vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: kind.buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(kind.fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: SHADOW_FORMAT,
                blend: kind.blend.map(|component| wgpu::BlendState {
                    color: component,
                    alpha: component,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        cache: None,
        multiview_mask: None,
    })
}
