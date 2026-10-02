//! The run-length RGB15 the preview records of the `.ctb` family and `.cxdlp` version 4
//! hold. See `docs/formats/chitu.md`.

use std::io;

use crate::Fields;

/// Set on a colour word when a repeat count follows it.
const REPEAT_FLAG: u16 = 0x0020;

/// Tag the repeat count carries in its top nibble.
const REPEAT_TAG: u16 = 0x3000;

/// Longest run one repeat word can carry.
const MAX_RUN: u32 = 0x0FFF;

/// The two preview sizes these containers expect, largest first.
pub const PREVIEW_SIZES_PX: [(u32, u32); 2] = [(400, 300), (200, 125)];

/// Bytes of the record in front of a preview's pixels.
pub const PREVIEW_HEADER_BYTES: u64 = 32;

/// Encodes an RGB image as the run-length RGB15 the preview records hold.
///
/// Green keeps six bits in the word but its lowest bit is the repeat flag, so it is
/// written five bits wide; that is what makes this RGB15 rather than RGB565.
pub fn encode_rgb15(pixels: &[[u8; 3]]) -> Vec<u8> {
    let mut data = Vec::new();
    let mut open: Option<(u16, u32)> = None;

    for &[r, g, b] in pixels {
        let colour = u16::from(b >> 3) | (u16::from(g >> 2) << 5) | (u16::from(r >> 3) << 11);
        match open {
            Some((current, run)) if current == colour && run < MAX_RUN => {
                open = Some((current, run + 1));
            }
            Some((current, run)) => {
                push_run(&mut data, current, run);
                open = Some((colour, 1));
            }
            None => open = Some((colour, 1)),
        }
    }
    if let Some((colour, run)) = open {
        push_run(&mut data, colour, run);
    }
    data
}

fn push_run(data: &mut Vec<u8>, colour: u16, run: u32) {
    // One or two pixels are cheaper written out than described, and the repeat flag has
    // to be cleared when they are: it is a bit of the colour otherwise.
    if run <= 2 {
        for _ in 0..run {
            data.extend_from_slice(&(colour & !REPEAT_FLAG).to_le_bytes());
        }
        return;
    }
    data.extend_from_slice(&(colour | REPEAT_FLAG).to_le_bytes());
    data.extend_from_slice(&(((run - 1) as u16) | REPEAT_TAG).to_le_bytes());
}

/// Writes one preview record and its pixels, and returns where the record began.
pub fn write_preview(
    fields: &mut Fields<'_>,
    width_px: u32,
    height_px: u32,
    pixels: &[[u8; 3]],
) -> io::Result<u32> {
    let record = fields.position()?;
    let data = encode_rgb15(pixels);

    fields.u32_le(width_px)?;
    fields.u32_le(height_px)?;
    fields.u32_le((record + PREVIEW_HEADER_BYTES) as u32)?;
    fields.u32_le(data.len() as u32)?;
    fields.zeros(16)?;
    fields.bytes(&data)?;

    Ok(record as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_pixel_is_one_word_with_the_repeat_bit_clear() {
        // Pure red is 0xF800, whose repeat bit is already zero.
        assert_eq!(encode_rgb15(&[[255, 0, 0]]), [0x00, 0xF8]);
    }

    #[test]
    fn a_run_is_a_colour_word_then_a_tagged_count() {
        let data = encode_rgb15(&[[0, 0, 0]; 5]);
        assert_eq!(data, [0x20, 0x00, 0x04, 0x30]);
    }

    #[test]
    fn a_run_longer_than_the_count_field_is_split() {
        let data = encode_rgb15(&[[0, 0, 0]; 0x1000]);
        assert_eq!(
            data.len(),
            6,
            "a run of 0xFFF as a colour and a count, then the last pixel on its own"
        );
    }
}
