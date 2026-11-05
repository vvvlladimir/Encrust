//! The twenty-eight byte header and the two previews behind it. A preview is a plain
//! 24-bit bitmap, bottom row first; see `docs/formats/svgx.md`.

use std::io;

use core_format::{
    Fields, FormatError, PrintJob, ReadSeek, Reads, Rgb, Thumbnail, preview_in_range,
};

/// What the file is headed by, in a sixteen-byte field padded with nuls.
pub(crate) const IDENTIFIER: &str = "DLP-II 1.1\n";

/// Bytes of the header: the identifier and the three addresses behind it.
pub(crate) const HEADER_BYTES: u64 = 28;

/// Bytes of a bitmap's own header, which is where its pixels begin.
const BITMAP_HEADER_BYTES: u32 = 54;

/// The previews the container holds, in the order it holds them.
const PREVIEW_SIZES_PX: [(u32, u32); 2] = [(128, 128), (200, 240)];

/// Colour a preview is padded with where the thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Writes the header. The addresses are only known once the previews and the document
/// have been written, so this runs twice: once over reserved space and once over itself.
pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    previews: [u32; 2],
    document: u32,
) -> io::Result<()> {
    fields.text(IDENTIFIER, 16)?;
    fields.u32_le(previews[0])?;
    fields.u32_le(previews[1])?;
    fields.u32_le(document)
}

/// Writes both previews and says where each began.
pub(crate) fn write_previews(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<[u32; 2]> {
    let mut addresses = [0; 2];
    for (address, (width, height)) in addresses.iter_mut().zip(PREVIEW_SIZES_PX) {
        let image =
            Thumbnail::fitted_or_filled(job.thumbnail.as_ref(), width, height, PREVIEW_BACKGROUND);
        *address = write_bitmap(fields, width, height, image.pixels())?;
    }
    Ok(addresses)
}

/// Writes one 24-bit bitmap and says where it began.
///
/// Both preview widths are a multiple of four pixels, so a row needs no padding; a width
/// that did would have to be padded to a four-byte boundary.
fn write_bitmap(
    fields: &mut Fields<'_>,
    width: u32,
    height: u32,
    pixels: &[Rgb],
) -> io::Result<u32> {
    let at = fields.position()? as u32;
    let data_bytes = width * height * 3;

    fields.bytes(b"BM")?;
    fields.u32_le(BITMAP_HEADER_BYTES + data_bytes)?;
    fields.u32_le(0)?;
    fields.u32_le(BITMAP_HEADER_BYTES)?;
    fields.u32_le(40)?;
    fields.u32_le(width)?;
    fields.u32_le(height)?;

    // One plane of twenty-four bits, uncompressed, at 96 dots an inch either way.
    fields.u32_le(0x0018_0001)?;
    fields.u32_le(0)?;
    fields.u32_le(data_bytes)?;
    fields.u32_le(3780)?;
    fields.u32_le(3780)?;
    fields.zeros(8)?;

    for row in (0..height).rev() {
        for column in 0..width {
            let [r, g, b] = pixels[(row * width + column) as usize];
            fields.bytes(&[b, g, r])?;
        }
    }
    Ok(at)
}

/// Reads the 24-bit bitmap at `at`: its shape, where its pixels begin, then the rows, each
/// padded to four bytes, bottom first unless the height is negative.
pub(crate) fn read_bitmap<S: ReadSeek>(
    reads: &mut Reads<S>,
    at: u64,
) -> Result<Thumbnail, FormatError> {
    reads.seek_to(at)?;
    let head = reads.bytes(BITMAP_HEADER_BYTES as usize)?;
    let bytes = |offset: usize| -> [u8; 4] { [0, 1, 2, 3].map(|byte| head[offset + byte]) };
    let word = |offset: usize| u32::from_le_bytes(bytes(offset));
    let bits = u16::from_le_bytes([head[28], head[29]]);
    if &head[..2] != b"BM" || bits != 24 || word(30) != 0 {
        return Err(FormatError::Encoding {
            what: "a preview",
            reason: format!("it is not an uncompressed 24-bit bitmap ({bits} bits)"),
        });
    }
    let width_px = word(18);
    let stated_height = i32::from_le_bytes(bytes(22));
    let height_px = stated_height.unsigned_abs();
    preview_in_range(width_px, height_px)?;

    let stride = (width_px as usize * 3).next_multiple_of(4);
    reads.seek_to(at + u64::from(word(10)))?;
    let data = reads.bytes(stride * height_px as usize)?;
    let mut pixels = Vec::with_capacity(width_px as usize * height_px as usize);
    for row in 0..height_px as usize {
        let stored = if stated_height > 0 {
            height_px as usize - 1 - row
        } else {
            row
        };
        let line = &data[stored * stride..][..width_px as usize * 3];
        pixels.extend(line.as_chunks::<3>().0.iter().map(|&[b, g, r]| [r, g, b]));
    }
    let count = pixels.len() as u64;
    Thumbnail::from_pixels(width_px, height_px, pixels).ok_or(FormatError::PreviewShort {
        width_px,
        height_px,
        pixels: count,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    #[test]
    fn the_header_is_as_wide_as_the_document_address_it_ends_with() {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut fields = Fields::new(&mut buffer);
            write_header(&mut fields, [28, 1000], 2000).expect("in memory");
        }
        let bytes = buffer.into_inner();
        assert_eq!(bytes.len() as u64, HEADER_BYTES);
        assert!(bytes.starts_with(IDENTIFIER.as_bytes()));
        assert_eq!(bytes[11..16], [0; 5], "the field is padded with nuls");
    }

    #[test]
    fn a_preview_is_a_bitmap_of_its_stated_size() {
        let mut buffer = Cursor::new(Vec::new());
        let addresses = {
            let mut fields = Fields::new(&mut buffer);
            write_previews(&mut fields, &sample_job(1)).expect("in memory")
        };
        let bytes = buffer.into_inner();

        assert_eq!(addresses, [0, (54 + 128 * 128 * 3) as u32]);
        assert_eq!(&bytes[..2], b"BM");
        let stated = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        assert_eq!(stated, 54 + 128 * 128 * 3);
        assert_eq!(
            bytes.len() as u32,
            stated + 54 + 200 * 240 * 3,
            "the second preview follows the first with nothing between them"
        );
    }

    #[test]
    fn a_written_bitmap_reads_back_top_row_first() {
        let mut pixels = vec![[255, 0, 0]; 4];
        pixels.extend([[0, 0, 255]; 4]);
        let mut buffer = Cursor::new(Vec::new());
        let at = write_bitmap(&mut Fields::new(&mut buffer), 4, 2, &pixels).expect("in memory");

        let mut reads = Reads::new(Cursor::new(buffer.into_inner()));
        let read = read_bitmap(&mut reads, u64::from(at)).expect("it was just written");

        assert_eq!(
            read,
            Thumbnail::from_pixels(4, 2, pixels).expect("eight pixels fill 4x2")
        );
    }

    #[test]
    fn a_bitmap_of_another_depth_is_refused() {
        let mut buffer = Cursor::new(Vec::new());
        write_bitmap(&mut Fields::new(&mut buffer), 4, 1, &[[0; 3]; 4]).expect("in memory");
        let mut bytes = buffer.into_inner();
        bytes[28] = 32;

        let err = read_bitmap(&mut Reads::new(Cursor::new(bytes)), 0).unwrap_err();
        assert!(matches!(
            err,
            FormatError::Encoding {
                what: "a preview",
                ..
            }
        ));
    }
}
