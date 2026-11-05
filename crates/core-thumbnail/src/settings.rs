/// One pixel of a thumbnail, red, green and blue.
pub type Rgb = [u8; 3];

/// Packs one pixel as `rrrrrggg gggbbbbb`, the five-six-five form a preview record holds. Two
/// formats store previews this way and differ only in the word's byte order.
pub fn rgb565(pixel: Rgb) -> u16 {
    let [r, g, b] = pixel;
    (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3)
}

/// The pixel a five-six-five word holds, each channel widened by repeating its top bits so
/// that full intensity reads back as 255.
pub fn rgb_of_565(word: u16) -> Rgb {
    let r = ((word >> 11) & 0x1F) as u8;
    let g = ((word >> 5) & 0x3F) as u8;
    let b = (word & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

/// The resin and support colours of the window's dark viewport, so a file's picture shows the
/// plate as it was set up.
pub const RESIN: Rgb = [0x86, 0xAB, 0xBB];
pub const SUPPORT: Rgb = [0xC9, 0xC2, 0xB6];

/// Bits each channel keeps in a five-six-five word and in a five-five-five one.
pub const RGB565_BITS: [u32; 3] = [5, 6, 5];
pub const RGB15_BITS: [u32; 3] = [5, 5, 5];

/// Size and colours one thumbnail is rendered at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbnailSettings {
    pub width_px: u32,
    pub height_px: u32,
    /// Colour every pixel no triangle covers, and the colour padding is filled with when
    /// the image is fitted into a record of another shape.
    pub background: Rgb,
    /// Colour a fully lit face of a model is shaded towards, and of a support.
    pub model: Rgb,
    pub support: Rgb,
}

impl ThumbnailSettings {
    /// The size a writer's largest preview record is cut down from. The parts span 88% of it,
    /// more than every record's longest side, so a record is a reduction, never a magnification.
    pub const SOURCE_PX: u32 = 1024;
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            width_px: Self::SOURCE_PX,
            height_px: Self::SOURCE_PX,
            background: [0x1A, 0x1C, 0x20],
            model: RESIN,
            support: SUPPORT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_five_six_five_word_reads_back_to_the_pixel_it_packed_at_full_and_no_intensity() {
        for pixel in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
        ] {
            assert_eq!(
                rgb_of_565(rgb565(pixel)),
                pixel,
                "{pixel:?} survives the round trip"
            );
        }
    }

    #[test]
    fn a_read_pixel_is_within_one_step_of_the_packed_one() {
        let pixel = [0x3F, 0x8C, 0xD8];
        let back = rgb_of_565(rgb565(pixel));
        for (channel, (was, is)) in pixel.iter().zip(back).enumerate() {
            assert!(
                was.abs_diff(is) < 8,
                "channel {channel}: {was} read back as {is}"
            );
        }
    }
}
