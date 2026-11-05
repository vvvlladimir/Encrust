//! Lighting each covered pixel from its normal, and drawing a line where the depth jumps.

use core_geometry::Vec3;

use super::Look;
use super::occlusion::occlusion;
use super::raster::Surfaces;
use crate::image::Thumbnail;
use crate::settings::{Rgb, ThumbnailSettings};

/// Directions towards each light, in view coordinates: x right, y up, z away from the eye.
/// The key high over the camera's left shoulder, the fill low on its right.
const KEY: Vec3 = Vec3::new(-0.5, 0.8, -0.6);
const FILL: Vec3 = Vec3::new(0.7, -0.25, -0.6);
const TOWARDS_EYE: Vec3 = Vec3::new(0.0, 0.0, -1.0);

/// Share of the model colour each light adds at full strength. The ambient keeps a face turned
/// from both lights a surface rather than a hole, and is never zero: a lit pixel is never black.
const AMBIENT: f32 = 0.24;
const KEY_STRENGTH: f32 = 0.8;
const FILL_STRENGTH: f32 = 0.25;

/// The white highlight of the key light: its peak share and how tight it is.
const SPECULAR: f32 = 0.22;
const SHININESS: i32 = 40;

/// The light along the outline that parts the model from a dark background: its peak share and
/// how fast it falls off towards a face seen square on.
const RIM: f32 = 0.45;
const RIM_FALLOFF: i32 = 3;

/// Pixels either side a depth jump is looked for over, which sets the line's width, and how
/// deep a jump must be, in pixel widths, to be drawn: deeper than a steep face's slope bends.
const LINE_REACH_PX: i64 = 2;
const LINE_JUMP_PX: f32 = 6.0;

/// Share of the parts' extent a jump must also pass to be drawn: a limb in front of a body,
/// not the pores and cracks of a sculpted surface.
const LINE_JUMP_SHARE: f32 = 0.025;

/// Share of its colour a pixel on a line keeps.
const LINE_SHADE: f32 = 0.4;

/// The shaded image of `surfaces`, `pixel_mm` the width of one pixel on the plate and
/// `span_mm` the parts' largest extent. A support takes neither lines nor occlusion.
pub(super) fn image(
    surfaces: &Surfaces,
    pixel_mm: f32,
    span_mm: f32,
    settings: &ThumbnailSettings,
) -> Option<Thumbnail> {
    let jump_mm = line_jump_mm(pixel_mm, span_mm);
    let mut pixels = Vec::with_capacity(surfaces.depths.len());
    let mut coverage = Vec::with_capacity(surfaces.depths.len());
    for y in 0..i64::from(surfaces.height_px) {
        for x in 0..i64::from(surfaces.width_px) {
            let index = (y * i64::from(surfaces.width_px) + x) as usize;
            let covered = surfaces.depths[index].is_finite();
            coverage.push(covered);
            if !covered {
                pixels.push(settings.background);
                continue;
            }
            let shaded = match surfaces.looks[index] {
                Look::Model => {
                    let open = 1.0 - occlusion(surfaces, x, y, pixel_mm);
                    let shaded = lit(surfaces.normals[index], settings.model, open);
                    if on_line(surfaces, x, y, jump_mm) {
                        scaled(shaded, LINE_SHADE)
                    } else {
                        shaded
                    }
                }
                Look::Support => lit(surfaces.normals[index], settings.support, 1.0),
            };
            pixels.push(shaded);
        }
    }
    Thumbnail::rendered(surfaces.width_px, surfaces.height_px, pixels, coverage)
}

/// The colour of a surface facing `normal`, `open` the share of light its surroundings let
/// in; turned towards the eye first so an unrepaired mesh shades its back faces as fronts.
fn lit(normal: Vec3, model: Rgb, open: f32) -> Rgb {
    let normal = normal.normalize_or_zero();
    let normal = if normal.dot(TOWARDS_EYE) < 0.0 {
        -normal
    } else {
        normal
    };
    let key = KEY.normalize();
    let diffuse = (AMBIENT
        + KEY_STRENGTH * normal.dot(key).max(0.0)
        + FILL_STRENGTH * normal.dot(FILL.normalize()).max(0.0))
        * open;
    let halfway = (key + TOWARDS_EYE).normalize();
    let white = SPECULAR * normal.dot(halfway).max(0.0).powi(SHININESS)
        + RIM * (1.0 - normal.dot(TOWARDS_EYE).max(0.0)).powi(RIM_FALLOFF);
    model.map(|channel| {
        let channel = f32::from(channel);
        (channel * diffuse + (255.0 - channel * 0.5) * white)
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

fn scaled(colour: Rgb, share: f32) -> Rgb {
    colour.map(|channel| (f32::from(channel) * share).round() as u8)
}

/// How deep a jump in depth is drawn as a line: past a few pixels and a share of the parts.
fn line_jump_mm(pixel_mm: f32, span_mm: f32) -> f32 {
    (LINE_JUMP_PX * pixel_mm).max(LINE_JUMP_SHARE * span_mm)
}

/// Whether the depth jumps across `(x, y)` by more than `jump_mm`, one surface passing in front of another. The
/// second difference ignores a plane however steep; an edge against nothing is the rim's.
fn on_line(surfaces: &Surfaces, x: i64, y: i64, jump_mm: f32) -> bool {
    let Some(here) = surfaces.depth(x, y) else {
        return false;
    };
    [(LINE_REACH_PX, 0), (0, LINE_REACH_PX)]
        .into_iter()
        .any(|(dx, dy)| {
            let before = surfaces.depth(x - dx, y - dy);
            let after = surfaces.depth(x + dx, y + dy);
            match (before, after) {
                (Some(before), Some(after)) => (before + after - 2.0 * here).abs() > jump_mm,
                _ => false,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::occlusion::MAX_OCCLUSION;
    use crate::settings::SUPPORT;

    /// A 9 by 1 strip whose depth at column `x` is `depth(x)`, a pixel 1 mm wide.
    fn strip(depth: impl Fn(i64) -> f32) -> Surfaces {
        let mut surfaces = Surfaces::new(9, 1);
        for x in 0..9 {
            surfaces.depths[x as usize] = depth(x);
        }
        surfaces
    }

    #[test]
    fn a_face_turned_away_is_lit_as_the_face_towards_the_eye() {
        let normal = Vec3::new(0.3, 0.4, -0.866);
        assert_eq!(
            lit(normal, [63, 140, 216], 1.0),
            lit(-normal, [63, 140, 216], 1.0)
        );
    }

    #[test]
    fn a_lit_pixel_is_never_black_whichever_way_it_faces() {
        for x in -4..=4 {
            for y in -4..=4 {
                let normal = Vec3::new(x as f32, y as f32, -1.0);
                let colour = scaled(lit(normal, [255; 3], 1.0 - MAX_OCCLUSION), LINE_SHADE);
                assert!(colour[0] > 0, "{normal:?} shades to {colour:?}");
            }
        }
    }

    #[test]
    fn a_steep_plane_draws_no_line() {
        let surfaces = strip(|x| 20.0 * x as f32);
        assert!((0..9).all(|x| !on_line(&surfaces, x, 0, 6.0)));
    }

    #[test]
    fn a_surface_passing_in_front_of_another_draws_a_line_at_the_step() {
        let surfaces = strip(|x| if x < 4 { 0.0 } else { 20.0 });
        assert!(on_line(&surfaces, 4, 0, 6.0), "the step is a line");
        assert!(!on_line(&surfaces, 8, 0, 6.0), "far from the step is not");
    }

    #[test]
    fn a_pore_shallow_beside_the_part_draws_no_line_however_many_pixels_deep() {
        // 0.1 mm pixels on a 100 mm part: six pixels are 0.6 mm, a fortieth is 2.5 mm.
        let jump_mm = line_jump_mm(0.1, 100.0);
        let pore = strip(|x| if x == 4 { 1.0 } else { 0.0 });
        assert!(!on_line(&pore, 4, 0, jump_mm), "a 1 mm pore is texture");
        let limb = strip(|x| if x < 4 { 0.0 } else { 20.0 });
        assert!(
            on_line(&limb, 4, 0, jump_mm),
            "a 20 mm step is a limb in front"
        );
    }

    #[test]
    fn a_support_takes_no_line() {
        let mut supports = strip(|x| if x < 4 { 0.0 } else { 20.0 });
        supports.looks.fill(Look::Support);
        supports.normals.fill(TOWARDS_EYE);
        let image = image(&supports, 1.0, 9.0, &ThumbnailSettings::default()).expect("9 by 1");
        let plain = lit(TOWARDS_EYE, SUPPORT, 1.0);
        assert!(image.pixels().iter().all(|pixel| *pixel == plain));
    }
}
