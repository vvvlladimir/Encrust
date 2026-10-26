//! The passes that resolve a cut and fill the section with a cap, and the stencil states
//! they count material in; see `docs/decisions/0074` and `0201`.

use super::{Builder, Facing, PipelineKind};
use crate::render::vertex::{LineVertex, ModelInstance, ModelVertex};

/// Half the stencil's range, which the counting below wraps around: a pixel whose count
/// ran negative — the plane inside material — lands above it, one whose count ran positive
/// lands below. See `docs/decisions/0074-the-section-cap-counts-material-not-crossings.md`.
pub(in crate::render::gpu) const MATERIAL_ABOVE: u32 = 127;

/// Counting the crossings: a ray leaving a solid above the cut counts up, one entering it
/// counts down, so the plane is inside material wherever the count ran negative. Back
/// faces are the ones a ray leaves through, and a body wound inward counts the other way
/// round, which is what takes a drain hole back out of the count.
fn counting_stencil() -> wgpu::StencilState {
    let face = |pass_op| wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op,
    };
    wgpu::StencilState {
        front: face(wgpu::StencilOperation::DecrementWrap),
        back: face(wgpu::StencilOperation::IncrementWrap),
        read_mask: 0xff,
        write_mask: 0xff,
    }
}

/// Drawing only where the count says the plane is inside material, and leaving the count
/// alone while doing it. Negative counts wrap past [`MATERIAL_ABOVE`]; a positive one is a
/// body wound inward standing in the air, which encloses nothing.
fn inside_only_stencil() -> wgpu::StencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Less,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 0xff,
        write_mask: 0x00,
    }
}

/// The other half of [`inside_only_stencil`]: drawing only where the count says there was
/// no material in front of the cut, which is where its own depth has to go back out.
fn outside_only_stencil() -> wgpu::StencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::GreaterEqual,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 0xff,
        write_mask: 0x00,
    }
}

/// Putting the count back to nothing, so the next object counts from zero and the section
/// cap after them counts from zero too.
fn clearing_stencil() -> wgpu::StencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Zero,
    };
    wgpu::StencilState {
        front: face,
        back: face,
        read_mask: 0xff,
        write_mask: 0xff,
    }
}

/// The passes that resolve the surface of a cut: the far wall of the cut body, kept where
/// material stands in front of it. See ADR 0201.
pub(in crate::render::gpu) struct Cutting {
    /// Lays that far wall into the depth plane, painting nothing.
    pub(in crate::render::gpu) candidate: Facing,
    /// Counts the material standing in front of it, into the stencil plane.
    pub(in crate::render::gpu) counting: Facing,
    /// Draws it where the count says there was material.
    pub(in crate::render::gpu) surface: Facing,
    /// Takes its depth back out where the count says there was none.
    pub(in crate::render::gpu) wipe: Facing,
    /// Puts the stencil back to zero between one object and the next.
    pub(in crate::render::gpu) reset: wgpu::RenderPipeline,
}

/// The two passes that fill the section cut with a flat face.
pub(in crate::render::gpu) struct Capping {
    /// Counts, in the stencil plane, how often a view ray crosses a solid above the cut.
    pub(in crate::render::gpu) crossing: Facing,
    /// The same count under the bottom cut, which its cap is filled from.
    pub(in crate::render::gpu) bottom_crossing: Facing,
    /// Fills the cut wherever that count says the plane is inside a solid.
    pub(in crate::render::gpu) cap: wgpu::RenderPipeline,
}

impl Builder<'_> {
    /// Four passes over one object's cut bodies and the object itself; see ADR 0201. The
    /// bodies are wound inward, so the face that survives culling is the far wall of the
    /// tube — the one a hole is looked at through.
    pub(in crate::render::gpu) fn cutting(&self) -> Cutting {
        let body = |label, fragment_entry, kind: PipelineKind<'_>| {
            self.facing(PipelineKind {
                label,
                vertex_entry: "model_vertex",
                fragment_entry,
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                cull: Some(wgpu::Face::Back),
                ..kind
            })
        };
        Cutting {
            candidate: body(
                "viewport_cut_candidate",
                "cut_candidate_fragment",
                PipelineKind {
                    writes_color: false,
                    ..PipelineKind::default()
                },
            ),
            counting: self.facing(PipelineKind {
                label: "viewport_cut_crossings",
                vertex_entry: "model_vertex",
                fragment_entry: "cut_crossing_fragment",
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                writes_color: false,
                // Only what stands in front of the cut's own surface counts, and the
                // counting leaves that surface where it is.
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: counting_stencil(),
                ..PipelineKind::default()
            }),
            surface: body(
                "viewport_cut_surface",
                "model_fragment",
                PipelineKind {
                    // The candidate pass already wrote this depth, so only the fragment
                    // that laid it down is drawn.
                    depth_compare: wgpu::CompareFunction::Equal,
                    stencil: inside_only_stencil(),
                    ..PipelineKind::default()
                },
            ),
            wipe: body(
                "viewport_cut_wipe",
                "cut_wipe_fragment",
                PipelineKind {
                    writes_color: false,
                    // The shader writes the depth, so the test cannot be made against the
                    // fragment's own: the stencil is what picks the pixels to wipe.
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: outside_only_stencil(),
                    ..PipelineKind::default()
                },
            ),
            reset: self.plain(PipelineKind {
                label: "viewport_cut_stencil_reset",
                vertex_entry: "screen_vertex",
                fragment_entry: "screen_fragment",
                writes_color: false,
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: clearing_stencil(),
                ..PipelineKind::default()
            }),
        }
    }

    pub(in crate::render::gpu) fn capping(&self) -> Capping {
        let crossing = |label, fragment_entry| {
            self.facing(PipelineKind {
                label,
                vertex_entry: "model_vertex",
                fragment_entry,
                buffers: &[Some(ModelVertex::layout()), Some(ModelInstance::layout())],
                writes_color: false,
                // Every crossing counts, whatever is in front of it, so the depth buffer
                // takes no part in this pass.
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: counting_stencil(),
                ..PipelineKind::default()
            })
        };
        Capping {
            crossing: crossing("viewport_section_crossings", "section_crossing_fragment"),
            bottom_crossing: crossing("viewport_bottom_crossings", "bottom_crossing_fragment"),
            cap: self.plain(PipelineKind {
                label: "viewport_section_cap",
                vertex_entry: "line_vertex",
                fragment_entry: "line_fragment",
                buffers: &[Some(LineVertex::layout())],
                // No depth, so the markers drawn after it show what stands in the material.
                depth_write: false,
                stencil: inside_only_stencil(),
                ..PipelineKind::default()
            }),
        }
    }
}
