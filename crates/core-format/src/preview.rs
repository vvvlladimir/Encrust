//! The preview pictures a sliced file carries, read back as a `Thumbnail` (ADR 0401).

use core_thumbnail::{Rgb, Thumbnail};

use zip::ZipArchive;

use crate::{FormatError, ReadSeek, Reads, decode_colour, read_entry};

/// Most pixels a preview a file states may claim; the largest known is a 954 x 850 PNG.
pub const MAX_PREVIEW_PX: u32 = 2048 * 2048;

/// The pixel count of a preview a container states, checked before anything is sized from
/// it. An empty preview is refused too: it is no picture.
pub fn preview_in_range(width_px: u32, height_px: u32) -> Result<usize, FormatError> {
    match width_px.checked_mul(height_px) {
        Some(pixels) if pixels > 0 && pixels <= MAX_PREVIEW_PX => Ok(pixels as usize),
        _ => Err(FormatError::PreviewOutOfRange {
            width_px,
            height_px,
        }),
    }
}

/// A raw preview of `width_px` by `height_px` at the reader's position, two bytes a pixel,
/// each turned into colour by `pixel`.
pub fn read_raw_preview<S: ReadSeek>(
    reads: &mut Reads<S>,
    width_px: u32,
    height_px: u32,
    pixel: fn([u8; 2]) -> Rgb,
) -> Result<Thumbnail, FormatError> {
    let count = preview_in_range(width_px, height_px)?;
    let bytes = reads.bytes(count * 2)?;
    let pixels = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| pixel(pair))
        .collect();
    thumbnail_of_pixels(width_px, height_px, pixels)
}

/// The PNG preview an archive container holds under `name`.
pub fn read_png_preview<S: ReadSeek>(
    zip: &mut ZipArchive<S>,
    name: &str,
) -> Result<Thumbnail, FormatError> {
    let mut entry = zip.by_name(name).map_err(|_| FormatError::Missing {
        what: name.to_owned(),
    })?;
    let bytes = read_entry(&mut entry, name)?;
    decode_colour(&bytes)
}

/// Wraps decoded pixels, refusing a count that does not fill the stated shape.
pub fn thumbnail_of_pixels(
    width_px: u32,
    height_px: u32,
    pixels: Vec<Rgb>,
) -> Result<Thumbnail, FormatError> {
    let count = pixels.len();
    Thumbnail::from_pixels(width_px, height_px, pixels).ok_or(FormatError::PreviewShort {
        width_px,
        height_px,
        pixels: count as u64,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use core_thumbnail::{rgb_of_565, rgb565};

    use super::*;

    #[test]
    fn a_raw_five_six_five_preview_reads_back_in_either_byte_order() {
        let red = rgb565([255, 0, 0]);
        let mut big = Reads::new(Cursor::new([red.to_be_bytes(), red.to_be_bytes()].concat()));
        let mut little = Reads::new(Cursor::new([red.to_le_bytes(), red.to_le_bytes()].concat()));

        let from_big =
            read_raw_preview(&mut big, 2, 1, |pair| rgb_of_565(u16::from_be_bytes(pair)))
                .expect("two pixels read");
        let from_little = read_raw_preview(&mut little, 2, 1, |pair| {
            rgb_of_565(u16::from_le_bytes(pair))
        })
        .expect("two pixels read");

        assert_eq!(from_big, Thumbnail::filled(2, 1, [255, 0, 0]));
        assert_eq!(from_little, from_big);
    }

    #[test]
    fn a_raw_preview_past_the_end_of_the_file_is_refused() {
        let mut reads = Reads::new(Cursor::new(vec![0; 6]));
        let err = read_raw_preview(&mut reads, 2, 2, |pair| {
            rgb_of_565(u16::from_be_bytes(pair))
        })
        .unwrap_err();
        assert!(matches!(err, FormatError::ImpossibleCount { .. }));
    }

    #[test]
    fn an_empty_or_oversized_preview_is_refused_before_anything_is_read() {
        for (width_px, height_px) in [(0, 10), (10, 0), (4096, 4096), (u32::MAX, 2)] {
            assert!(
                matches!(
                    preview_in_range(width_px, height_px),
                    Err(FormatError::PreviewOutOfRange { .. })
                ),
                "{width_px}x{height_px} is refused"
            );
        }
    }

    #[test]
    fn an_archive_reads_back_the_png_preview_it_holds_and_names_one_it_lacks() {
        let image = Thumbnail::filled(3, 2, [0, 128, 255]);
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file("preview.png", zip::write::SimpleFileOptions::default())
            .expect("a cursor takes an entry");
        std::io::Write::write_all(
            &mut archive,
            &crate::encode_colour(&image).expect("encodes"),
        )
        .expect("a cursor takes the bytes");
        let bytes = archive.finish().expect("the archive closes").into_inner();
        let mut zip = ZipArchive::new(Cursor::new(bytes)).expect("the archive opens");

        assert_eq!(
            read_png_preview(&mut zip, "preview.png").expect("it is there"),
            image
        );
        let err = read_png_preview(&mut zip, "other.png").unwrap_err();
        assert!(matches!(err, FormatError::Missing { .. }));
    }

    #[test]
    fn pixels_that_do_not_fill_the_shape_are_refused() {
        let err = thumbnail_of_pixels(2, 2, vec![[0; 3]; 3]).unwrap_err();
        assert!(matches!(err, FormatError::PreviewShort { pixels: 3, .. }));
    }
}
