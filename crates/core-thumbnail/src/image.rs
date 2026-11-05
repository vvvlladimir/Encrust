use crate::settings::Rgb;

/// Fraction of a record left empty around a rendered part on each side.
const MARGIN: f64 = 0.06;

/// A rendered thumbnail: plain RGB pixels, row by row from the top left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    width_px: u32,
    height_px: u32,
    pixels: Vec<Rgb>,
    /// Which pixels a part covers, for a rendered image; the rest is background a record
    /// replaces with its own. `None` for a picture read from a file, all of it content.
    coverage: Option<Vec<bool>>,
}

/// The part of a source image a record shows: a pixel rectangle and the share of the record
/// left empty around it.
struct Window {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    margin: f64,
}

impl Thumbnail {
    /// An image of one colour, which is what an empty plate renders to.
    pub fn filled(width_px: u32, height_px: u32, colour: Rgb) -> Self {
        Self {
            width_px,
            height_px,
            pixels: vec![colour; (width_px as usize) * (height_px as usize)],
            coverage: None,
        }
    }

    /// Wraps pixels that are already laid out, or `None` when there are not exactly
    /// `width_px * height_px` of them.
    pub fn from_pixels(width_px: u32, height_px: u32, pixels: Vec<Rgb>) -> Option<Self> {
        (pixels.len() == (width_px as usize) * (height_px as usize)).then_some(Self {
            width_px,
            height_px,
            pixels,
            coverage: None,
        })
    }

    /// A render whose `coverage` marks the pixels a part landed on.
    pub(crate) fn rendered(
        width_px: u32,
        height_px: u32,
        pixels: Vec<Rgb>,
        coverage: Vec<bool>,
    ) -> Option<Self> {
        let count = (width_px as usize) * (height_px as usize);
        (pixels.len() == count && coverage.len() == count).then_some(Self {
            width_px,
            height_px,
            pixels,
            coverage: Some(coverage),
        })
    }

    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    pub fn height_px(&self) -> u32 {
        self.height_px
    }

    pub fn pixels(&self) -> &[Rgb] {
        &self.pixels
    }

    /// `thumbnail` fitted to a record of `width_px` by `height_px`, or the record all
    /// `background` when there is none.
    pub fn fitted_or_filled(
        thumbnail: Option<&Self>,
        width_px: u32,
        height_px: u32,
        background: Rgb,
    ) -> Self {
        match thumbnail {
            Some(thumbnail) => thumbnail.fitted_to(width_px, height_px, background),
            None => Self::filled(width_px, height_px, background),
        }
    }

    /// This image scaled to fill a record of `width_px` by `height_px`, its aspect ratio kept
    /// and the rest `background`. A render is cropped to its parts first, so they fill the
    /// record whatever its shape, on the record's own background.
    #[must_use]
    pub fn fitted_to(&self, width_px: u32, height_px: u32, background: Rgb) -> Self {
        let mut fitted = Self::filled(width_px, height_px, background);
        if width_px == 0 || height_px == 0 {
            return fitted;
        }
        let Some(window) = self.window() else {
            return fitted;
        };

        let room = 1.0 - 2.0 * window.margin;
        let scale = (f64::from(width_px) * room / f64::from(window.width))
            .min(f64::from(height_px) * room / f64::from(window.height));
        let content_w = ((f64::from(window.width) * scale).round() as u32).clamp(1, width_px);
        let content_h = ((f64::from(window.height) * scale).round() as u32).clamp(1, height_px);
        let left = (width_px - content_w) / 2;
        let top = (height_px - content_h) / 2;

        for y in 0..content_h {
            let rows = span(y, content_h, window.top, window.height);
            for x in 0..content_w {
                let columns = span(x, content_w, window.left, window.width);
                let index = ((top + y) * width_px + left + x) as usize;
                fitted.pixels[index] = self.box_average(columns, rows, background);
            }
        }
        fitted
    }

    /// What a record shows: the whole of a picture, or the box around a render's parts with
    /// a margin. `None` when there is nothing to show.
    fn window(&self) -> Option<Window> {
        if self.width_px == 0 || self.height_px == 0 {
            return None;
        }
        let Some(coverage) = &self.coverage else {
            return Some(Window {
                left: 0,
                top: 0,
                width: self.width_px,
                height: self.height_px,
                margin: 0.0,
            });
        };

        let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
        let (mut max_x, mut max_y) = (0, 0);
        for (index, _) in coverage.iter().enumerate().filter(|(_, covered)| **covered) {
            let (x, y) = (index as u32 % self.width_px, index as u32 / self.width_px);
            (min_x, max_x) = (min_x.min(x), max_x.max(x));
            (min_y, max_y) = (min_y.min(y), max_y.max(y));
        }
        (min_x <= max_x).then(|| Window {
            left: min_x,
            top: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
            margin: MARGIN,
        })
    }

    /// Mean of the source pixels in `columns` by `rows`, an uncovered one counted as
    /// `background`. A box filter: point sampling would drop thin supports from the picture.
    fn box_average(&self, (x0, x1): (u32, u32), (y0, y1): (u32, u32), background: Rgb) -> Rgb {
        let mut totals = [0u32; 3];
        let mut count = 0u32;
        for sy in y0..y1 {
            for sx in x0..x1 {
                let index = (sy * self.width_px + sx) as usize;
                let covered = self
                    .coverage
                    .as_ref()
                    .is_none_or(|coverage| coverage[index]);
                let pixel = if covered {
                    self.pixels[index]
                } else {
                    background
                };
                for channel in 0..3 {
                    totals[channel] += u32::from(pixel[channel]);
                }
                count += 1;
            }
        }
        if count == 0 {
            return background;
        }
        totals.map(|total| (total / count) as u8)
    }
}

/// A 4 by 4 ordered dither: sixteen thresholds spread so neighbours differ most.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

impl Thumbnail {
    /// This image ready to be cut to `bits` a channel by dropping the low bits: each pixel
    /// nudged by an ordered dither, so a gradient turns to a fine pattern, not steps. Black
    /// stays black, so a background still packs into long runs.
    #[must_use]
    pub fn dithered(&self, bits: [u32; 3]) -> Self {
        let width = self.width_px.max(1) as usize;
        let pixels = self
            .pixels
            .iter()
            .enumerate()
            .map(|(index, pixel)| {
                let threshold = f32::from(BAYER[(index / width) % 4][index % width % 4]);
                let mut out = *pixel;
                for (channel, bits) in out.iter_mut().zip(bits) {
                    let step = f32::from(1u16 << (8 - bits.min(8)));
                    let nudge = (threshold + 0.5) / 16.0 * step;
                    *channel = (f32::from(*channel) + nudge).min(255.0) as u8;
                }
                out
            })
            .collect();
        Self {
            pixels,
            ..self.clone()
        }
    }
}

/// Source pixels `start..end` that destination pixel `index` of `count` covers, inside a run
/// of `length` source pixels from `from`; never empty.
fn span(index: u32, count: u32, from: u32, length: u32) -> (u32, u32) {
    let start = (u64::from(index) * u64::from(length) / u64::from(count)) as u32;
    let end = ((u64::from(index) + 1) * u64::from(length) / u64::from(count)) as u32;
    (from + start, from + end.max(start + 1).min(length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filled_image_holds_one_colour_per_pixel() {
        let image = Thumbnail::filled(4, 3, [1, 2, 3]);
        assert_eq!(image.pixels().len(), 12);
        assert!(image.pixels().iter().all(|pixel| *pixel == [1, 2, 3]));
    }

    #[test]
    fn pixels_of_the_wrong_count_are_refused() {
        assert!(Thumbnail::from_pixels(2, 2, vec![[0, 0, 0]; 3]).is_none());
    }

    #[test]
    fn fitting_a_square_into_a_wide_record_pads_the_sides() {
        let square = Thumbnail::filled(8, 8, [255, 255, 255]);
        let wide = square.fitted_to(16, 8, [0, 0, 0]);

        assert_eq!(wide.width_px(), 16);
        // 8 by 8 fits a 16 by 8 record as an 8 by 8 block with four columns either side.
        assert_eq!(
            wide.pixels()[0],
            [0, 0, 0],
            "the left padding is background"
        );
        assert_eq!(wide.pixels()[8], [255, 255, 255], "the middle is the image");
        assert_eq!(
            wide.pixels()[15],
            [0, 0, 0],
            "the right padding is background"
        );
    }

    /// A 10 by 10 render on grey whose part is the white block of columns 2..8, rows 4..6.
    fn wide_render() -> Thumbnail {
        let covered =
            |index: usize| (2..8).contains(&(index % 10)) && (4..6).contains(&(index / 10));
        let pixels = (0..100)
            .map(|index| {
                if covered(index) {
                    [255; 3]
                } else {
                    [26, 28, 32]
                }
            })
            .collect();
        let coverage = (0..100).map(covered).collect();
        Thumbnail::rendered(10, 10, pixels, coverage).expect("a hundred of each")
    }

    #[test]
    fn a_render_is_cropped_to_its_part_so_it_fills_a_record_of_another_shape() {
        let fitted = wide_render().fitted_to(30, 10, [0, 0, 0]);
        let middle_row = &fitted.pixels()[5 * 30..6 * 30];
        let white = middle_row
            .iter()
            .filter(|pixel| **pixel == [255; 3])
            .count();
        // The 6 by 2 part fits 88% of the record, 26 pixels wide; uncropped it would be 6.
        assert_eq!(white, 26, "the part spans the record less its margins");
    }

    #[test]
    fn a_record_shows_its_own_background_where_a_render_has_no_part() {
        let fitted = wide_render().fitted_to(30, 10, [0, 0, 0]);
        assert!(
            fitted.pixels().iter().all(|[r, g, b]| r == g && g == b),
            "only the white part and the black record, never the render's grey"
        );
    }

    #[test]
    fn a_render_with_no_part_fits_to_the_record_background_alone() {
        let empty = Thumbnail::rendered(4, 4, vec![[26, 28, 32]; 16], vec![false; 16])
            .expect("sixteen of each");
        assert_eq!(
            empty.fitted_to(6, 3, [1, 2, 3]),
            Thumbnail::filled(6, 3, [1, 2, 3])
        );
    }

    #[test]
    fn a_dithered_grey_averages_back_to_itself_once_cut_to_five_bits() {
        let grey = Thumbnail::filled(4, 4, [100; 3]).dithered([5, 5, 5]);
        let mean = grey
            .pixels()
            .iter()
            .map(|pixel| f32::from(pixel[0] >> 3) * 8.0)
            .sum::<f32>()
            / 16.0;
        // Cut without a dither, 100 is 96 everywhere.
        assert!(
            (mean - 100.0).abs() < 1.0,
            "the mean of the cut words is {mean}"
        );
    }

    #[test]
    fn a_dither_leaves_black_black() {
        let black = Thumbnail::filled(4, 4, [0; 3]).dithered([5, 6, 5]);
        assert!(
            black
                .pixels()
                .iter()
                .all(|pixel| crate::settings::rgb565(*pixel) == 0)
        );
    }

    #[test]
    fn halving_an_image_averages_each_block() {
        // Two white pixels and two black ones per destination pixel, so mid grey.
        let checker: Vec<Rgb> = (0..4)
            .flat_map(|y: u32| (0..4).map(move |x: u32| [((x + y) % 2) as u8 * 255; 3]))
            .collect();
        let image = Thumbnail::from_pixels(4, 4, checker).expect("sixteen pixels");
        let half = image.fitted_to(2, 2, [0, 0, 0]);

        assert!(
            half.pixels().iter().all(|pixel| pixel[0] == 127),
            "two of four pixels white averages to 127"
        );
    }
}
