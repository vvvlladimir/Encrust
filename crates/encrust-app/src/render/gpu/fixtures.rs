//! Frames the gpu module's tests draw.

use super::{FrameInput, ModelDraw};

/// A frame of `models` alone, seen from straight above, with nothing else on it.
pub(super) fn frame(models: &[ModelDraw]) -> FrameInput<'_> {
    FrameInput {
        view_projection: core_geometry::Mat4::IDENTITY,
        eye: core_geometry::Vec3::Z,
        section_mm: None,
        lines: &[],
        wide_lines: &[],
        models,
        markers: &[],
        solids: &[],
        cut_surfaces: &[],
        cap: &[],
        bottom_cut_mm: None,
        bottom_cap: &[],
        label: &[],
        atlas: None,
        cuts: &[],
        pockets: &[],
        reliefs: &[],
        bands: &[],
        band_floor_mm: 0.0,
        volume_mm: None,
        cut_line: None,
        look: None,
        xray: false,
        floor: None,
        grid: true,
        light: false,
    }
}
