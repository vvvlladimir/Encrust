//! The run-length RGB15 the preview records of the `.ctb` family and `.cxdlp` version 4
//! hold. See `docs/formats/chitu.md`.

use std::io;

use core_thumbnail::{Rgb, Thumbnail};

use crate::preview::{preview_in_range, thumbnail};
use crate::{Fields, FormatError, ReadSeek, Reads};

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

    for &pixel in pixels {
        let colour = rgb15_word(pixel);
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

/// One pixel's colour word, red highest, green six bits wide but for its lowest bit, which
/// a run-length record keeps for its repeat flag and a raw one leaves clear.
pub fn rgb15(pixel: [u8; 3]) -> u16 {
    rgb15_word(pixel) & !REPEAT_FLAG
}

/// The pixel an RGB15 word holds, its flag bit ignored and each five-bit channel widened by
/// repeating its top bits, so that full intensity reads back as 255.
pub fn rgb_of_rgb15(word: u16) -> Rgb {
    let widen = |channel: u16| {
        let channel = (channel & 0x1F) as u8;
        (channel << 3) | (channel >> 2)
    };
    [widen(word >> 11), widen(word >> 6), widen(word)]
}

fn rgb15_word([r, g, b]: [u8; 3]) -> u16 {
    u16::from(b >> 3) | (u16::from(g >> 2) << 5) | (u16::from(r >> 3) << 11)
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

/// Decodes the run-length RGB15 of one preview record into a picture of the stated shape.
pub fn decode_rgb15(data: &[u8], width_px: u32, height_px: u32) -> Result<Thumbnail, FormatError> {
    let count = preview_in_range(width_px, height_px)?;
    let mut pixels: Vec<Rgb> = Vec::with_capacity(count);
    let mut words = data.as_chunks::<2>().0.iter().enumerate();
    while let Some((index, pair)) = words.next() {
        let word = u16::from_le_bytes(*pair);
        let mut run = 1;
        if word & REPEAT_FLAG != 0 {
            let Some((_, count_pair)) = words.next() else {
                return Err(FormatError::MalformedRun { offset: index * 2 });
            };
            run = usize::from(u16::from_le_bytes(*count_pair) & MAX_RUN as u16) + 1;
        }
        if pixels.len() + run > count {
            return Err(FormatError::MalformedRun { offset: index * 2 });
        }
        pixels.extend(std::iter::repeat_n(rgb_of_rgb15(word), run));
    }
    thumbnail(width_px, height_px, pixels)
}

/// The preview whose record starts at `record`: its shape, where its data sits and how long
/// it is, then the run-length pixels there. A record at zero is a header stating none.
pub fn read_rgb15_preview<S: ReadSeek>(
    reads: &mut Reads<S>,
    record: u64,
) -> Result<Thumbnail, FormatError> {
    if record == 0 {
        return Err(FormatError::Missing {
            what: "a preview".to_owned(),
        });
    }
    reads.seek_to(record)?;
    let width_px = reads.u32_le()?;
    let height_px = reads.u32_le()?;
    let address = reads.u32_le()?;
    let length = reads.u32_le()?;
    reads.seek_to(u64::from(address))?;
    let data = reads.bytes(length as usize)?;
    decode_rgb15(&data, width_px, height_px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raw_word_leaves_the_repeat_bit_clear() {
        assert_eq!(rgb15([255, 255, 255]), 0xFFDF);
        assert_eq!(
            rgb15([0, 0x0C, 0]),
            0x0040,
            "green's sixth bit lands above the flag"
        );
    }

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

    #[test]
    fn a_written_preview_reads_back_as_the_picture_it_was_written_from() {
        let mut pixels = vec![[255, 0, 0]; 6];
        pixels.extend([[0, 0, 255], [255, 255, 255]]);
        let mut buffer = std::io::Cursor::new(vec![0; 3]);
        buffer.set_position(3);
        let record = write_preview(&mut Fields::new(&mut buffer), 4, 2, &pixels)
            .expect("a cursor does not fail");

        let mut reads = Reads::new(std::io::Cursor::new(buffer.into_inner()));
        let preview =
            read_rgb15_preview(&mut reads, u64::from(record)).expect("it was just written");

        assert_eq!(
            preview,
            Thumbnail::from_pixels(4, 2, pixels).expect("eight pixels fill 4x2")
        );
    }

    #[test]
    fn a_header_pointing_at_zero_states_no_preview() {
        let mut reads = Reads::new(std::io::Cursor::new(vec![0; 64]));
        let err = read_rgb15_preview(&mut reads, 0).unwrap_err();
        assert!(matches!(err, FormatError::Missing { .. }));
    }

    #[test]
    fn a_run_past_the_stated_shape_is_refused() {
        let data = encode_rgb15(&[[0, 0, 0]; 9]);
        let err = decode_rgb15(&data, 4, 2).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { .. }));
    }

    #[test]
    fn a_repeat_flag_with_no_count_behind_it_is_refused() {
        let err = decode_rgb15(&REPEAT_FLAG.to_le_bytes(), 4, 2).unwrap_err();
        assert!(matches!(err, FormatError::MalformedRun { offset: 0 }));
    }

    #[test]
    fn runs_that_stop_short_of_the_shape_are_refused() {
        let data = encode_rgb15(&[[0, 0, 0]; 7]);
        let err = decode_rgb15(&data, 4, 2).unwrap_err();
        assert!(matches!(err, FormatError::PreviewShort { pixels: 7, .. }));
    }
}
