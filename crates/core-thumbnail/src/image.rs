use crate::settings::Rgb;

/// A rendered thumbnail: plain RGB pixels, row by row from the top left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    width_px: u32,
    height_px: u32,
    pixels: Vec<Rgb>,
}

impl Thumbnail {
    /// An image of one colour, which is what an empty plate renders to.
    pub fn filled(width_px: u32, height_px: u32, colour: Rgb) -> Self {
        Self {
            width_px,
            height_px,
            pixels: vec![colour; (width_px as usize) * (height_px as usize)],
        }
    }

    /// Wraps pixels that are already laid out, or `None` when there are not exactly
    /// `width_px * height_px` of them.
    pub fn from_pixels(width_px: u32, height_px: u32, pixels: Vec<Rgb>) -> Option<Self> {
        (pixels.len() == (width_px as usize) * (height_px as usize)).then_some(Self {
            width_px,
            height_px,
            pixels,
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

    /// This image scaled to fill a record of `width_px` by `height_px`.
    ///
    /// The aspect ratio is kept and what is left over is padded with `background`: every
    /// preview record has a shape of its own, and a model squashed sideways to fill a
    /// 4:3 record looks like a model that was sliced squashed.
    #[must_use]
    pub fn fitted_to(&self, width_px: u32, height_px: u32, background: Rgb) -> Self {
        let mut fitted = Self::filled(width_px, height_px, background);
        if self.width_px == 0 || self.height_px == 0 || width_px == 0 || height_px == 0 {
            return fitted;
        }

        let scale = f64::from(width_px) / f64::from(self.width_px);
        let scale = scale.min(f64::from(height_px) / f64::from(self.height_px));
        let content_w = ((f64::from(self.width_px) * scale).round() as u32).clamp(1, width_px);
        let content_h = ((f64::from(self.height_px) * scale).round() as u32).clamp(1, height_px);
        let left = (width_px - content_w) / 2;
        let top = (height_px - content_h) / 2;

        for y in 0..content_h {
            for x in 0..content_w {
                let colour = self.box_average(x, y, content_w, content_h);
                let index = ((top + y) * width_px + left + x) as usize;
                fitted.pixels[index] = colour;
            }
        }
        fitted
    }

    /// Mean of the source pixels that fall inside destination pixel `(x, y)`.
    ///
    /// A box filter rather than a point sample: a thumbnail is a heavy reduction, and
    /// point sampling drops thin supports out of the picture altogether.
    fn box_average(&self, x: u32, y: u32, content_w: u32, content_h: u32) -> Rgb {
        let span = |index: u32, count: u32, source: u32| {
            let start = (u64::from(index) * u64::from(source) / u64::from(count)) as u32;
            let end = ((u64::from(index) + 1) * u64::from(source) / u64::from(count)) as u32;
            (start, end.max(start + 1).min(source))
        };
        let (x0, x1) = span(x, content_w, self.width_px);
        let (y0, y1) = span(y, content_h, self.height_px);

        let mut totals = [0u32; 3];
        let mut count = 0u32;
        for sy in y0..y1 {
            for sx in x0..x1 {
                let pixel = self.pixels[(sy * self.width_px + sx) as usize];
                for channel in 0..3 {
                    totals[channel] += u32::from(pixel[channel]);
                }
                count += 1;
            }
        }
        if count == 0 {
            return [0, 0, 0];
        }
        [
            (totals[0] / count) as u8,
            (totals[1] / count) as u8,
            (totals[2] / count) as u8,
        ]
    }
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
