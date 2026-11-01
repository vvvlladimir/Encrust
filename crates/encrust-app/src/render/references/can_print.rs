//! The print round a can's straight wall: any picture, as high as the wall, blended round the
//! back from its own edges. See docs/design/viewport.md.

use std::f32::consts::TAU;

use core_geometry::Scalar;
use egui::ecolor::{gamma_u8_from_linear_f32, linear_f32_from_gamma_u8};
use image::imageops::{FilterType, resize};
use image::{ImageError, Rgba, RgbaImage};

use super::shapes::{CAN_RADIUS_MM, CAN_WALL_MM};

/// The label: any picture `image` decodes, replaced by swapping the file.
const LABEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/props/can-label.jpg"
));

/// Texels round the can: the most a browser's graphics API takes along one side.
const GIRTH_PX: u32 = 2048;
/// The share of the label's width at either edge averaged into the colour the back blends.
const EDGE_SHARE: Scalar = 0.01;

/// The print as sRGB texels, row 0 at its top, each level of its mip chain half the last.
pub struct CanPrint {
    pub size: [u32; 2],
    pub levels: Vec<Vec<[u8; 4]>>,
}

/// Why the print could not be drawn.
#[derive(Debug, thiserror::Error)]
pub enum CanPrintError {
    #[error(transparent)]
    Image(#[from] ImageError),
}

impl CanPrint {
    /// The print as the can carries it.
    pub fn new() -> Result<Self, CanPrintError> {
        drawn(LABEL, GIRTH_PX)
    }
}

/// The label `bytes` holds round a wall `girth_px` texels round: as high as the wall, as wide
/// as it is in proportion but never past the girth, in the middle.
fn drawn(bytes: &[u8], girth_px: u32) -> Result<CanPrint, CanPrintError> {
    let label = image::load_from_memory(bytes)?.to_rgba8();
    let girth_mm = TAU * CAN_RADIUS_MM;
    let [from_mm, to_mm] = CAN_WALL_MM;
    let tall_px = ((to_mm - from_mm) / girth_mm * girth_px as Scalar)
        .round()
        .max(1.0) as u32;
    let wide_px = (tall_px as Scalar * label.width() as Scalar / label.height().max(1) as Scalar)
        .round()
        .clamp(1.0, girth_px as Scalar) as u32;
    let fitted = resize(&label, wide_px, tall_px, FilterType::Lanczos3);
    let size = [girth_px, tall_px];
    Ok(CanPrint {
        size,
        levels: mip_chain(wrapped(&fitted, girth_px), size),
    })
}

/// Linear texels of a wall `girth_px` round with `label` in its middle; each row of the gap
/// behind fades from the label's right edge round to its left.
fn wrapped(label: &RgbaImage, girth_px: u32) -> Vec<[f32; 3]> {
    let wide = label.width();
    let start = (girth_px - wide) / 2;
    let gap = girth_px - wide;
    let band = ((wide as Scalar * EDGE_SHARE).round() as usize).max(1);
    let mut texels = Vec::with_capacity((girth_px * label.height()) as usize);
    for row in label.rows() {
        let row: Vec<[f32; 3]> = row.map(linear).collect();
        let (left, right) = (mean(&row[..band]), mean(&row[row.len() - band..]));
        texels.extend((0..girth_px).map(|x| match x.checked_sub(start) {
            Some(inside) if inside < wide => row[inside as usize],
            _ => {
                let past = (x + girth_px - start - wide) % girth_px;
                let share = (past as Scalar + 0.5) / gap as Scalar;
                [0, 1, 2].map(|channel| right[channel] + (left[channel] - right[channel]) * share)
            }
        }));
    }
    texels
}

fn mean(texels: &[[f32; 3]]) -> [f32; 3] {
    [0, 1, 2]
        .map(|channel| texels.iter().map(|texel| texel[channel]).sum::<f32>() / texels.len() as f32)
}

fn linear(pixel: &Rgba<u8>) -> [f32; 3] {
    [pixel[0], pixel[1], pixel[2]].map(linear_f32_from_gamma_u8)
}

fn encoded(colour: [f32; 3]) -> [u8; 4] {
    let [r, g, b] = colour.map(gamma_u8_from_linear_f32);
    [r, g, b, u8::MAX]
}

/// Every level from `texels` down to one texel, each averaging the four under it in linear
/// light, then encoded.
fn mip_chain(mut texels: Vec<[f32; 3]>, mut size: [u32; 2]) -> Vec<Vec<[u8; 4]>> {
    let mut levels = vec![texels.iter().copied().map(encoded).collect()];
    while size != [1, 1] {
        let [wide, tall] = size;
        let next = [(wide / 2).max(1), (tall / 2).max(1)];
        let at = |x: u32, y: u32| texels[(y.min(tall - 1) * wide + x.min(wide - 1)) as usize];
        texels = (0..next[1])
            .flat_map(|y| (0..next[0]).map(move |x| (x, y)))
            .map(|(x, y)| {
                mean(&[
                    at(2 * x, 2 * y),
                    at(2 * x + 1, 2 * y),
                    at(2 * x, 2 * y + 1),
                    at(2 * x + 1, 2 * y + 1),
                ])
            })
            .collect();
        size = next;
        levels.push(texels.iter().copied().map(encoded).collect());
    }
    levels
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    const GIRTH_TEST_PX: u32 = 256;
    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    /// A PNG `wide` by `tall` in three upright stripes: red, green, blue from the left.
    fn striped(wide: u32, tall: u32) -> Vec<u8> {
        let label = RgbaImage::from_fn(wide, tall, |x, _| {
            Rgba([RED, GREEN, BLUE][(3 * x / wide) as usize])
        });
        let mut bytes = Cursor::new(Vec::new());
        label
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("an image in memory encodes");
        bytes.into_inner()
    }

    fn print_of(bytes: &[u8]) -> CanPrint {
        drawn(bytes, GIRTH_TEST_PX).expect("the test label decodes")
    }

    /// The texel `x` texels round the wall, halfway up it.
    fn round_at(print: &CanPrint, x: u32) -> [u8; 4] {
        let [wide, tall] = print.size;
        print.levels[0][(wide * (tall / 2) + x) as usize]
    }

    /// The first texel round a square label's print and the one just past its last.
    fn square_ends(print: &CanPrint) -> (u32, u32) {
        let start = (GIRTH_TEST_PX - print.size[1]) / 2;
        (start, start + print.size[1])
    }

    fn assert_near(seen: [u8; 4], expected: [u8; 4], what: &str) {
        let off = seen.iter().zip(expected).map(|(a, b)| a.abs_diff(b)).max();
        assert!(off <= Some(2), "expected {expected:?} {what}, got {seen:?}");
    }

    #[test]
    fn the_bundled_label_wraps_the_can() {
        let print = CanPrint::new().expect("the bundled label decodes");
        assert_eq!(print.size[0], GIRTH_PX);
    }

    #[test]
    fn a_narrow_label_sits_in_the_middle_at_its_own_proportions() {
        let print = print_of(&striped(90, 90));
        let (start, end) = square_ends(&print);
        assert_near(round_at(&print, start), RED, "at the label's left");
        assert_near(round_at(&print, GIRTH_TEST_PX / 2), GREEN, "in the middle");
        assert_near(round_at(&print, end - 1), BLUE, "at the label's right");
    }

    #[test]
    fn the_back_fades_from_the_right_edge_round_to_the_left() {
        let print = print_of(&striped(90, 90));
        let (start, end) = square_ends(&print);
        let [after, before] = [round_at(&print, end), round_at(&print, start - 1)];
        assert!(
            after[2] > 250 && after[0] < 20,
            "expected blue just past the right, got {after:?}"
        );
        assert!(
            before[0] > 250 && before[2] < 20,
            "expected red just before the left, got {before:?}"
        );
        let behind = round_at(&print, 0);
        assert!(
            behind[0] > 100 && behind[2] > 100 && behind[1] < 5,
            "expected red and blue mixed behind, got {behind:?}"
        );
    }

    #[test]
    fn a_label_wider_than_the_wall_is_squeezed_round_it_whole() {
        let print = print_of(&striped(900, 30));
        assert_near(round_at(&print, 0), RED, "at the seam's left");
        assert_near(round_at(&print, GIRTH_TEST_PX / 2), GREEN, "in the middle");
        assert_near(
            round_at(&print, GIRTH_TEST_PX - 1),
            BLUE,
            "at the seam's right",
        );
    }

    #[test]
    fn the_mip_chain_halves_down_to_one_texel() {
        let print = print_of(&striped(90, 90));
        let [wide, tall] = print.size;
        assert_eq!(print.levels[0].len(), (wide * tall) as usize);
        assert_eq!(
            print.levels.last().map(Vec::len),
            Some(1),
            "expected one last texel"
        );
        assert_eq!(print.levels.len(), GIRTH_TEST_PX.ilog2() as usize + 1);
    }

    #[test]
    fn bytes_that_are_no_image_are_an_image_error() {
        assert!(matches!(
            drawn(b"not an image", GIRTH_TEST_PX),
            Err(CanPrintError::Image(_))
        ));
    }
}
