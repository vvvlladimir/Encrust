/// One pixel of a thumbnail, red, green and blue.
pub type Rgb = [u8; 3];

/// Packs one pixel as `rrrrrggg gggbbbbb`, the five-six-five form a preview record holds.
///
/// Two sliced-file formats store their previews this way and disagree only on the byte
/// order of the word, so the packing lives here with the pixel it packs.
pub fn rgb565(pixel: Rgb) -> u16 {
    let [r, g, b] = pixel;
    (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3)
}

/// Size and colours one thumbnail is rendered at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbnailSettings {
    pub width_px: u32,
    pub height_px: u32,
    /// Colour every pixel no triangle covers, and the colour padding is filled with when
    /// the image is fitted into a record of another shape.
    pub background: Rgb,
    /// Colour a fully lit face is shaded towards.
    pub model: Rgb,
}

impl ThumbnailSettings {
    /// The size a writer's largest preview record is cut down from.
    ///
    /// Square and larger than every record either format holds, so a record is always a
    /// reduction and never a magnification of what was rendered.
    pub const SOURCE_PX: u32 = 512;
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            width_px: Self::SOURCE_PX,
            height_px: Self::SOURCE_PX,
            background: [0x1A, 0x1C, 0x20],
            model: [0x3F, 0x8C, 0xD8],
        }
    }
}
