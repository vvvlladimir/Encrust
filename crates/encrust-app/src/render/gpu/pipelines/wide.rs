//! The pipeline that draws a line wider than the card's own one pixel.

use super::{Builder, PipelineKind};
use crate::render::vertex::WideLine;

impl Builder<'_> {
    /// The edges a tool marks on the models, each a quad the shader lays across the screen.
    pub(in crate::render::gpu) fn wide_line(&self) -> wgpu::RenderPipeline {
        self.plain(PipelineKind {
            label: "viewport_wide_lines",
            vertex_entry: "wide_line_vertex",
            fragment_entry: "line_fragment",
            buffers: &[Some(WideLine::layout())],
            // A later mark on the same edge wins the depth it shares with an earlier one.
            depth_compare: wgpu::CompareFunction::LessEqual,
            ..PipelineKind::default()
        })
    }
}
