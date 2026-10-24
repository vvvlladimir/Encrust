/// One layer as an 8-bit greyscale image, row-major from the top-left of the LCD. 0 is dark, 255
/// full exposure; intermediate values are anti-aliasing and expose proportionally less.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerMask {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl LayerMask {
    /// Allocates an all-dark mask.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize)],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.pixels
    }

    /// Total exposure expressed as an area, in pixels: a fully lit pixel counts as one,
    /// a half-lit one as a half.
    pub fn coverage(&self) -> f32 {
        let total: u64 = self.pixels.iter().map(|&p| u64::from(p)).sum();
        total as f32 / 255.0
    }

    pub fn is_blank(&self) -> bool {
        self.pixels.iter().all(|&p| p == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_mask_is_blank_and_correctly_sized() {
        let mask = LayerMask::new(4, 3);
        assert_eq!(mask.pixels().len(), 12);
        assert!(mask.is_blank());
    }

    #[test]
    fn writing_a_pixel_clears_blankness() {
        let mut mask = LayerMask::new(2, 2);
        mask.pixels_mut()[3] = 255;
        assert!(!mask.is_blank());
    }

    #[test]
    fn coverage_counts_a_half_lit_pixel_as_a_half() {
        let mut mask = LayerMask::new(2, 2);
        mask.pixels_mut()[0] = 255;
        mask.pixels_mut()[1] = 128;
        assert!((mask.coverage() - 1.502).abs() < 1e-3);
    }
}
