//! How long the render view has rested on one picture, and where each frame of it is drawn:
//! a fraction of a pixel aside, so the frames average into a sharper one.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use core_geometry::Mat4;
use core_geometry::glam::Vec4;

use super::globals::Globals;
use super::shadow::{hash_casters, hash_draw};
use super::*;

/// Frames averaged before the picture is held: enough that an edge has no step left.
pub const SETTLED_SAMPLES: u32 = 16;

/// The picture the frames are averaging, how many it has, and where the window hears it.
#[derive(Default)]
pub(super) struct Settling {
    picture: Option<u64>,
    samples: u32,
    report: Option<Arc<AtomicU32>>,
}

/// The `index`-th of the radical inverse in `base`: Halton's low-discrepancy sequence.
fn halton(mut index: u32, base: u32) -> f32 {
    let (mut result, mut fraction) = (0.0, 1.0);
    while index > 0 {
        fraction /= base as f32;
        result += fraction * (index % base) as f32;
        index /= base;
    }
    result
}

/// Where the `sample`-th frame is drawn, pixels off the centre: Halton (2, 3), as TAA does.
pub(super) fn jitter_px(sample: u32) -> [f32; 2] {
    [halton(sample + 1, 2) - 0.5, halton(sample + 1, 3) - 0.5]
}

/// A lens a picture at rest is seen through: its aperture radius and where it is focused,
/// mm, and the projection behind it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lens {
    pub aperture_mm: f32,
    pub focus_mm: f32,
    pub projection: Mat4,
}

/// `view_projection` seen from the `sample`-th point of `lens`'s aperture, sheared back so
/// the plane in focus stands still; averaged over the frames, what stands off it blurs.
pub(super) fn through_lens(view_projection: Mat4, lens: &Lens, sample: u32) -> Mat4 {
    let out = halton(sample + 1, 5).sqrt() * lens.aperture_mm;
    let around = halton(sample + 1, 7) * std::f32::consts::TAU;
    let (x, y) = (out * around.cos(), out * around.sin());
    let focus = lens.focus_mm.max(1e-3);
    let shear = Mat4::from_cols(
        Vec4::X,
        Vec4::Y,
        Vec4::new(-x / focus, -y / focus, 1.0, 0.0),
        Vec4::new(-x, -y, 0.0, 1.0),
    );
    let view = lens.projection.inverse() * view_projection;
    lens.projection * shear * view
}

/// `view_projection` moved `jitter_px` pixels over a viewport `size_px` large.
pub(super) fn jittered(view_projection: Mat4, jitter_px: [f32; 2], size_px: [f32; 2]) -> Mat4 {
    let shift = Mat4::from_cols(
        Vec4::X,
        Vec4::Y,
        Vec4::Z,
        Vec4::new(
            2.0 * jitter_px[0] / size_px[0].max(1.0),
            -2.0 * jitter_px[1] / size_px[1].max(1.0),
            0.0,
            1.0,
        ),
    );
    shift * view_projection
}

/// The picture at rest as one number: what the shader is told, the lens it is seen through
/// and what `casters` adds, so a change to any of them starts the average again.
fn picture_of(
    globals: &Globals,
    lens: Option<&Lens>,
    casters: impl FnOnce(&mut DefaultHasher),
) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytemuck::bytes_of(globals).hash(&mut hasher);
    lens.map(|lens| [lens.aperture_mm.to_bits(), lens.focus_mm.to_bits()])
        .hash(&mut hasher);
    casters(&mut hasher);
    hasher.finish()
}

/// Everything `frame` draws besides the globals, into `hasher`: the casters, and the lines,
/// marks, label and cut surfaces a held picture would otherwise go on showing stale.
fn hash_drawn(frame: &FrameInput<'_>, hasher: &mut DefaultHasher) {
    hash_casters(frame, hasher);
    for vertices in [frame.lines, frame.cap, frame.bottom_cap] {
        bytemuck::cast_slice::<_, u8>(vertices).hash(hasher);
    }
    bytemuck::cast_slice::<_, u8>(frame.wide_lines).hash(hasher);
    bytemuck::cast_slice::<_, u8>(frame.label).hash(hasher);
    let cut = frame
        .cut_surfaces
        .iter()
        .flat_map(|cut| [&cut.body, &cut.solid]);
    for draw in frame.solids.iter().chain(cut) {
        hash_draw(draw, hasher);
    }
}

impl ViewportResources {
    /// Where the window hears how many frames the picture at rest has averaged.
    pub fn report_samples_to(&mut self, report: Arc<AtomicU32>) {
        self.settling.report = Some(report);
    }

    /// Counts this frame into the picture at rest, or starts a new one when anything in it
    /// changed; `None` while the render view moves or is off.
    pub(super) fn settle(&mut self, frame: &FrameInput<'_>, globals: &Globals) -> Option<u32> {
        let resting = frame.look.is_some() && !frame.light && self.reads_planes;
        let lens = self.frame.lens.as_ref();
        let picture =
            resting.then(|| picture_of(globals, lens, |hasher| hash_drawn(frame, hasher)));
        let settling = &mut self.settling;
        settling.samples = match picture.is_some() && picture == settling.picture {
            true => settling.samples.saturating_add(1),
            false => 0,
        };
        settling.picture = picture;
        let sample = picture.map(|_| settling.samples);
        if let Some(report) = &settling.report {
            report.store(sample.map_or(0, |sample| sample + 1), Ordering::Relaxed);
        }
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::vertex::WideLine;

    #[test]
    fn the_jitter_stays_inside_the_pixel_and_never_repeats_over_a_picture() {
        let offsets: Vec<[f32; 2]> = (0..SETTLED_SAMPLES).map(jitter_px).collect();
        for [x, y] in &offsets {
            assert!(
                x.abs() <= 0.5 && y.abs() <= 0.5,
                "({x}, {y}) leaves the pixel"
            );
        }
        for (index, one) in offsets.iter().enumerate() {
            assert!(!offsets[index + 1..].contains(one), "{one:?} repeats");
        }
    }

    #[test]
    fn a_lens_keeps_the_plane_in_focus_still_and_moves_what_stands_off_it() {
        let projection =
            core_geometry::glam::camera::rh::proj::directx::perspective(0.8, 1.0, 1.0, 1000.0);
        let lens = Lens {
            aperture_mm: 5.0,
            focus_mm: 200.0,
            projection,
        };
        let seen = |sample, z: f32| {
            through_lens(projection, &lens, sample)
                .project_point3(core_geometry::Vec3::new(10.0, 5.0, -z))
        };
        let (focused, near) = (
            seen(3, 200.0) - seen(7, 200.0),
            seen(3, 60.0) - seen(7, 60.0),
        );
        assert!(
            focused.truncate().length() < 1e-4,
            "the plane in focus moved by {focused}"
        );
        assert!(
            near.truncate().length() > 1e-3,
            "a point off it stayed still"
        );
    }

    #[test]
    fn opening_the_lens_starts_the_picture_at_rest_again() {
        let globals = <Globals as bytemuck::Zeroable>::zeroed();
        let lens = Lens {
            aperture_mm: 0.4,
            focus_mm: 25.0,
            projection: Mat4::IDENTITY,
        };
        let pinhole = picture_of(&globals, None, |_| {});
        assert_ne!(pinhole, picture_of(&globals, Some(&lens), |_| {}));
    }

    #[test]
    fn marking_an_edge_starts_the_picture_at_rest_again() {
        let globals = <Globals as bytemuck::Zeroable>::zeroed();
        let ends = [core_geometry::Vec3::ZERO, core_geometry::Vec3::X];
        let size = core_geometry::Vec2::new(800.0, 600.0);
        let mark = [WideLine::new(ends, egui::Color32::RED, 2.0, size)];
        let marked = FrameInput {
            wide_lines: &mark,
            ..fixtures::frame(&[])
        };
        let picture =
            |frame: &FrameInput<'_>| picture_of(&globals, None, |hasher| hash_drawn(frame, hasher));
        assert_ne!(picture(&fixtures::frame(&[])), picture(&marked));
    }

    #[test]
    fn a_jittered_view_moves_what_it_sees_by_the_jitter() {
        let size_px = [800.0, 600.0];
        let moved = jittered(Mat4::IDENTITY, [0.5, 0.25], size_px);
        let seen = moved.project_point3(core_geometry::Vec3::ZERO);
        let pixels = [seen.x * size_px[0] / 2.0, -seen.y * size_px[1] / 2.0];
        assert!(
            (pixels[0] - 0.5).abs() < 1e-4 && (pixels[1] - 0.25).abs() < 1e-4,
            "expected (0.5, 0.25) px, got {pixels:?}"
        );
    }
}
