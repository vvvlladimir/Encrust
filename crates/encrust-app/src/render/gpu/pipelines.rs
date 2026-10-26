//! Every render pipeline the viewport builds, and the stencil states that decide what
//! each pass is allowed to touch; see `docs/decisions/0062`, `0074` and `0184`.

use crate::render::vertex::{LabelVertex, LineVertex, ModelInstance, ModelVertex, ReliefVertex};

use super::*;

mod section;
mod shadow;
mod wide;

pub(super) use section::{Capping, Cutting, MATERIAL_ABOVE};
pub(super) use shadow::{SHADOW_FORMAT, Shadowing};

/// The one uniform every pipeline reads: the camera, the light and the section.
pub(super) fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viewport_globals_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// One pipeline built twice: for a model as imported, and for one a negative scale
/// mirrors. A mirror turns every triangle's winding on screen, and the front face, the
/// shading and the section's stencil count all read that winding.
pub(super) struct Facing {
    pub(super) upright: wgpu::RenderPipeline,
    pub(super) mirrored: wgpu::RenderPipeline,
}

impl Facing {
    pub(super) fn of(&self, mirrored: bool) -> &wgpu::RenderPipeline {
        if mirrored {
            &self.mirrored
        } else {
            &self.upright
        }
    }
}

/// The pipelines that draw the plate and the models in their own flat colours.
pub(super) struct Solid {
    pub(super) cutting: Cutting,
    pub(super) model: Facing,
    /// The same models seen through: no depth at all, so every surface behind one still
    /// paints and the cavity inside shows.
    pub(super) xray: Facing,
    pub(super) line: wgpu::RenderPipeline,
    pub(super) capping: Capping,
    pub(super) markers: Markers,
}

/// A translucent marker drawn far half first and near half over it; in one pass its faces
/// would cover each other in whatever order they were meshed in.
pub(super) struct Markers {
    pub(super) far: Facing,
    pub(super) near: Facing,
}

/// A pipeline that samples one texture, with the layout its bind group is built against
/// and the sampler it reads through.
pub(super) struct Textured<P> {
    pub(super) pipelines: P,
    pub(super) layout: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
}

/// What every viewport pipeline is built from: one shader, one target, one uniform.
pub(super) struct Builder<'a> {
    device: &'a wgpu::Device,
    shader: wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    globals_layout: &'a wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
}

impl<'a> Builder<'a> {
    pub(super) fn new(
        device: &'a wgpu::Device,
        target_format: wgpu::TextureFormat,
        globals_layout: &'a wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            device,
            shader: device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl")),
            target_format,
            globals_layout,
            layout: device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport"),
                bind_group_layouts: &[Some(globals_layout)],
                immediate_size: 0,
            }),
        }
    }

    fn plain(&self, kind: PipelineKind<'_>) -> wgpu::RenderPipeline {
        self.on(&self.layout, kind)
    }

    fn on(&self, layout: &wgpu::PipelineLayout, kind: PipelineKind<'_>) -> wgpu::RenderPipeline {
        pipeline(self.device, layout, &self.shader, self.target_format, kind)
    }

    fn facing(&self, kind: PipelineKind<'_>) -> Facing {
        self.facing_on(&self.layout, kind)
    }

    fn facing_on(&self, layout: &wgpu::PipelineLayout, kind: PipelineKind<'_>) -> Facing {
        Facing {
            upright: self.on(layout, kind.clone()),
            mirrored: self.on(
                layout,
                PipelineKind {
                    front_face: wgpu::FrontFace::Cw,
                    ..kind
                },
            ),
        }
    }

    pub(super) fn solid(&self) -> Solid {
        Solid {
            model: self.facing(PipelineKind {
                label: "viewport_models",
                vertex_entry: "model_vertex",
                fragment_entry: "model_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                ..PipelineKind::default()
            }),
            xray: self.facing(PipelineKind {
                label: "viewport_models_seen_through",
                vertex_entry: "model_vertex",
                fragment_entry: "model_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                // Nothing hides anything: each surface adds its own translucent wash, so
                // the wall, the cavity and the lattice behind it all show at once.
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                ..PipelineKind::default()
            }),
            line: self.plain(PipelineKind {
                label: "viewport_lines",
                vertex_entry: "line_vertex",
                fragment_entry: "line_fragment",
                buffers: &[Some(LineVertex::layout())],
                topology: wgpu::PrimitiveTopology::LineList,
                ..PipelineKind::default()
            }),
            capping: self.capping(),
            cutting: self.cutting(),
            markers: self.markers(),
        }
    }

    fn markers(&self) -> Markers {
        let half = |label, cull| {
            self.facing(PipelineKind {
                label,
                vertex_entry: "model_vertex",
                fragment_entry: "marker_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                cull: Some(cull),
                ..PipelineKind::default()
            })
        };
        Markers {
            far: half("viewport_markers_far", wgpu::Face::Front),
            near: half("viewport_markers_near", wgpu::Face::Back),
        }
    }

    /// Draws the word in front of the plate, sampled out of the font atlas.
    pub(super) fn label(&self) -> Textured<wgpu::RenderPipeline> {
        self.textured(
            wgpu::TextureViewDimension::D2,
            wgpu::AddressMode::ClampToEdge,
            Self::on,
            PipelineKind {
                label: "viewport_label",
                vertex_entry: "label_vertex",
                fragment_entry: "label_fragment",
                buffers: &[Some(LabelVertex::layout())],
                // Takes no depth of its own: what hides it is the model standing in front
                // of it.
                depth_write: false,
                ..PipelineKind::default()
            },
        )
    }

    /// Draws a model with its own texture washed over it, for the Relief tool.
    pub(super) fn relief(&self) -> Textured<Facing> {
        self.textured(
            wgpu::TextureViewDimension::D2Array,
            // Wrapping, because a coordinate outside the unit square wraps everywhere else
            // a texture is read; see ADR 0116.
            wgpu::AddressMode::Repeat,
            Self::facing_on,
            PipelineKind {
                label: "viewport_relief",
                vertex_entry: "relief_vertex",
                fragment_entry: "relief_fragment",
                buffers: &[Some(ReliefVertex::layout()), Some(ModelInstance::layout())],
                ..PipelineKind::default()
            },
        )
    }

    fn textured<P>(
        &self,
        dimension: wgpu::TextureViewDimension,
        address_mode: wgpu::AddressMode,
        build: impl FnOnce(&Self, &wgpu::PipelineLayout, PipelineKind<'_>) -> P,
        kind: PipelineKind<'_>,
    ) -> Textured<P> {
        let layout = texture_layout(self.device, kind.label, dimension);
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(kind.label),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(kind.label),
                bind_group_layouts: &[Some(self.globals_layout), Some(&layout)],
                immediate_size: 0,
            });
        Textured {
            pipelines: build(self, &pipeline_layout, kind),
            layout,
            sampler,
        }
    }
}

/// One filtered texture and the sampler it is read through, as a fragment shader sees them.
fn texture_layout(
    device: &wgpu::Device,
    label: &str,
    dimension: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: dimension,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

#[derive(Clone)]
struct PipelineKind<'a> {
    label: &'a str,
    vertex_entry: &'a str,
    fragment_entry: &'a str,
    buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
    topology: wgpu::PrimitiveTopology,
    front_face: wgpu::FrontFace,
    writes_color: bool,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    stencil: wgpu::StencilState,
    /// Which side of a face is dropped, or `None` to keep both. Only the passes that
    /// resolve a cut ask for it: there the near wall of a tube is never what is looked at.
    cull: Option<wgpu::Face>,
}

impl Default for PipelineKind<'_> {
    fn default() -> Self {
        Self {
            label: "viewport",
            vertex_entry: "model_vertex",
            fragment_entry: "model_fragment",
            buffers: &[],
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            writes_color: true,
            depth_write: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            cull: None,
        }
    }
}

/// Back faces are kept: an imported mesh may have inverted faces, and hiding them would
/// hide the defect the Scene panel is reporting.
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    kind: PipelineKind<'_>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(kind.label),
        layout: Some(layout),
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
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: if kind.writes_color {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                },
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: kind.topology,
            front_face: kind.front_face,
            cull_mode: kind.cull,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(kind.depth_write),
            depth_compare: Some(kind.depth_compare),
            stencil: kind.stencil.clone(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: SAMPLE_COUNT,
            ..Default::default()
        },
        cache: None,
        multiview_mask: None,
    })
}
