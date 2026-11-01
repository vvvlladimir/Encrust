//! The one uniform every pipeline reads, and how a frame fills it.

use core_geometry::Vec3;

use super::light::key_view_projection;
use super::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct Globals {
    view_projection: [[f32; 4]; 4],
    /// Its inverse: the render view's backdrop turns a pixel back into a line of sight.
    inverse_view_projection: [[f32; 4]; 4],
    /// The key light's view over the render view's set, which its shadow map is drawn in.
    key_view_projection: [[f32; 4]; 4],
    /// The lights and the ambient, from `theme::Lighting`; every `w` is unused padding.
    key_towards: [f32; 4],
    fill_towards: [f32; 4],
    key_color: [f32; 4],
    fill_color: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    /// The plate the contact shadow lies on, plate millimetres: `xy` its near corner and
    /// `zw` its size. `shadow_color.a` is how dark the shadow goes.
    shadow_area: [f32; 4],
    shadow_color: [f32; 4],
    /// The colour the plate's own surface is filled with, over the same area, and the
    /// grid drawn into it: `grid` is the minor and the major spacing, millimetres, and
    /// 1.0 in `z` while the grid is shown.
    plate_color: [f32; 4],
    grid_minor: [f32; 4],
    grid_major: [f32; 4],
    grid: [f32; 4],
    /// `xyz` is where the camera stands, plate millimetres; `w` is how much of itself a
    /// surface keeps at most while the models are drawn seen through, and 0.0 while they
    /// are not. See `docs/design/viewport.md`.
    eye: [f32; 4],
    /// `x` is the height the models are cut at, plate millimetres, and `y` is 1.0 while
    /// there is a cut at all; `z` and `w` are the same for the cut from below.
    section: [f32; 4],
    /// `x` is how many of `cuts` carry a drain this frame, `y` how many of `bands` carry
    /// an exposure, and `z` the height below which a band has no effect.
    counts: [f32; 4],
    /// `xyz` washes a surface that needs holding up; `w` is unused padding.
    overhang_color: [f32; 4],
    /// `xyz` washes whatever shows of a model's inside; `w` is unused padding.
    inside_color: [f32; 4],
    /// `xyz` is the high corner of where a model stands unmarked, plate millimetres; `w` is
    /// 1.0 while what stands past it is marked.
    volume: [f32; 4],
    /// `xyz` is the low corner of the same box; `w` is unused padding.
    volume_low: [f32; 4],
    /// `xyz` marks whatever stands past the build volume; `w` is unused padding.
    outside_color: [f32; 4],
    /// The Cut tool's plane as `xyz` its normal and `w` its offset along it, millimetres.
    cut_plane: [f32; 4],
    /// The box the plane is traced on, plate millimetres; `cut_low.w` is 1.0 while it is.
    cut_low: [f32; 4],
    cut_high: [f32; 4],
    /// `xyz` is the colour the plane is traced in; `w` is unused padding.
    cut_color: [f32; 4],
    /// The render view: `x` the millimetres between layers stepped into the models, `y` 1.0
    /// under studio light, `z` [`Look::attenuation_mm`], `w` [`Look::pivot_mm`].
    look: [f32; 4],
    /// The viewport in framebuffer pixels: `xy` its corner, `zw` its size. Written by
    /// `draw`, the first to know it.
    pub(super) screen: [f32; 4],
    /// What egui paints under the viewport, which shows wherever the scene drew nothing.
    backdrop: [f32; 4],
    /// The render view's mat: `desk` its corners in plate millimetres, `xy` low and `zw`
    /// high, and `mat.xy` its own size across and along.
    desk: [f32; 4],
    mat: [f32; 4],
    cuts: [DrainCut; MAX_CUTS],
    bands: [ExposureBand; MAX_BANDS],
    pockets: [TrapBox; MAX_POCKETS],
}

/// What a fixed-size array the shader reads takes of `from`, and how much it took.
fn clamped<T: Copy + Default, const N: usize>(from: &[T]) -> ([T; N], f32) {
    let mut into = [T::default(); N];
    let taken = from.len().min(N);
    into[..taken].copy_from_slice(&from[..taken]);
    (into, taken as f32)
}

/// One frame's uniform: the camera, the light, and everything subtracted or washed over a
/// model per fragment.
pub(super) fn globals_of(frame: &FrameInput<'_>) -> Globals {
    let (cuts, taken) = clamped::<_, MAX_CUTS>(frame.cuts);
    let (bands, banded) = clamped::<_, MAX_BANDS>(frame.bands);
    let (pockets, held) = clamped::<_, MAX_POCKETS>(frame.pockets);
    let light = &theme::scene().light;
    let towards = |[x, y, z]: [f32; 3]| [x, y, z, 0.0];
    let desk = frame.look.map(|look| look.desk).unwrap_or_default();
    Globals {
        view_projection: frame.view_projection.to_cols_array_2d(),
        inverse_view_projection: frame.view_projection.inverse().to_cols_array_2d(),
        key_view_projection: key_view_projection(
            &desk,
            frame.volume_mm.map_or(0.0, |[_, high]| high.z),
            Vec3::from(light.key_towards),
        )
        .to_cols_array_2d(),
        key_towards: towards(light.key_towards),
        fill_towards: towards(light.fill_towards),
        key_color: theme::gamma(light.key),
        fill_color: theme::gamma(light.fill),
        sky: theme::gamma(light.sky),
        ground: theme::gamma(light.ground),
        shadow_area: frame.floor.map_or([0.0; 4], |floor| floor.area()),
        shadow_color: theme::gamma(theme::scene().shadow),
        plate_color: theme::gamma(theme::scene().plate),
        grid_minor: theme::gamma(theme::scene().grid_minor),
        grid_major: theme::gamma(theme::scene().grid_major),
        grid: [
            GRID_SPACING_MM,
            GRID_MAJOR_MM,
            if frame.grid { 1.0 } else { 0.0 },
            0.0,
        ],
        eye: frame
            .eye
            .extend(if frame.xray { theme::SEEN_THROUGH } else { 0.0 })
            .to_array(),
        section: section(frame.section_mm, frame.bottom_cut_mm),
        counts: [taken, banded, frame.band_floor_mm, held],
        overhang_color: theme::gamma(theme::scene().overhang),
        inside_color: theme::gamma(theme::scene().section_wash),
        volume: frame
            .volume_mm
            .map_or([0.0; 4], |[_, high]| high.extend(1.0).to_array()),
        volume_low: frame
            .volume_mm
            .map_or([0.0; 4], |[low, _]| low.extend(0.0).to_array()),
        outside_color: theme::gamma(theme::scene().outside),
        cut_plane: frame.cut_line.map_or([0.0; 4], |line| {
            line.normal.extend(line.offset_mm).to_array()
        }),
        cut_low: frame
            .cut_line
            .map_or([0.0; 4], |line| line.bounds.mins.extend(1.0).to_array()),
        cut_high: frame
            .cut_line
            .map_or([0.0; 4], |line| line.bounds.maxs.extend(0.0).to_array()),
        cut_color: theme::gamma(theme::scene().cut_line),
        look: frame.look.map_or([0.0; 4], |look| {
            [
                look.layer_height_mm,
                1.0,
                look.attenuation_mm,
                look.pivot_mm,
            ]
        }),
        screen: [0.0; 4],
        backdrop: theme::gamma(theme::colors().sunken),
        desk: desk.rect,
        mat: [desk.mat_mm[0], desk.mat_mm[1], 0.0, 0.0],
        cuts,
        bands,
        pockets,
    }
}

/// The two cuts as the shader reads them. A height on its own could not say "no cut": a
/// model standing below the plate is legal, so no value of it is free to mean nothing.
fn section(top_mm: Option<Scalar>, bottom_mm: Option<Scalar>) -> [f32; 4] {
    let [top, top_on] = top_mm.map_or([0.0; 2], |height| [height, 1.0]);
    let [bottom, bottom_on] = bottom_mm.map_or([0.0; 2], |height| [height, 1.0]);
    [top, top_on, bottom, bottom_on]
}
