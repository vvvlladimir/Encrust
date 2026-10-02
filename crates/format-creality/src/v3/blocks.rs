//! Everything of a `.cxdlp` that is not a layer: the header, the three previews, the two
//! settings blocks and the footer. Field by field in `docs/formats/creality.md`.

use std::io;

use core_format::{Fields, PrintJob, Rgb, Thumbnail, rgb565};

use crate::family::MAGIC;

/// The revision we write. Version 2 differs only in its checksum and is not written.
pub(crate) const VERSION: u16 = 3;

/// The two bytes that close a preview, the area table and every layer.
pub(crate) const PAGE_BREAK: [u8; 2] = [0x0D, 0x0A];

/// The previews the container holds, in the order it holds them.
pub(crate) const PREVIEW_SIZES_PX: [(u32, u32); 3] = [(116, 116), (290, 290), (290, 290)];

/// Colour a preview is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// What names us in the settings block, which a reader shows and a machine ignores.
const SLICER: &str = concat!("Encrust-", env!("CARGO_PKG_VERSION"));

/// Writes the header. The panel and the layer count are what a reader decodes layers with.
pub(crate) fn write_header(fields: &mut Fields<'_>, job: &PrintJob, model: &str) -> io::Result<()> {
    fields.u32_be(MAGIC.len() as u32)?;
    fields.bytes(MAGIC)?;
    fields.u16_be(VERSION)?;
    write_text(fields, model)?;
    fields.u16_be(job.layer_count() as u16)?;
    fields.u16_be(job.raster.width_px as u16)?;
    fields.u16_be(job.raster.height_px as u16)?;
    fields.zeros(64)
}

/// Writes the three previews as raw big-endian RGB565, each closed by a page break.
pub(crate) fn write_previews(
    fields: &mut Fields<'_>,
    thumbnail: Option<&Thumbnail>,
) -> io::Result<()> {
    for (width, height) in PREVIEW_SIZES_PX {
        let image = match thumbnail {
            Some(thumbnail) => thumbnail.fitted_to(width, height, PREVIEW_BACKGROUND),
            None => Thumbnail::filled(width, height, PREVIEW_BACKGROUND),
        };
        for &pixel in image.pixels() {
            fields.u16_be(rgb565(pixel))?;
        }
        fields.bytes(&PAGE_BREAK)?;
    }
    Ok(())
}

/// Writes the settings the firmware prints by.
///
/// Three of them are text in UTF-16 rather than numbers, and the exposure is in tenths of
/// a second while the bottom exposure beside it is in whole seconds — both are the
/// container's, not a choice of ours.
pub(crate) fn write_settings(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (material, display) = (&job.material, &job.printer.display);

    write_utf16(fields, &decimals(display.width_mm, 2))?;
    write_utf16(fields, &decimals(display.height_mm, 2))?;
    write_utf16(fields, &decimals(job.nominal_height_mm(), 3))?;

    fields.u16_be(tenths(job.header_exposure_s()))?;
    // One second is the floor the firmware needs: a zero here does not print.
    fields.u16_be(seconds(material.light_off_s()).max(1))?;
    fields.u16_be(seconds(material.bottom_exposure_s))?;
    fields.u16_be(material.bottom_layers as u16)?;
    fields.u16_be(whole_mm(material.bottom_lift_distance_mm))?;
    fields.u16_be(mm_per_second(material.bottom_lift_speed_mm_min))?;
    fields.u16_be(whole_mm(material.lift_distance_mm))?;
    fields.u16_be(mm_per_second(material.lift_speed_mm_min))?;
    fields.u16_be(mm_per_second(material.retract_speed_mm_min))?;
    fields.u16_be(u16::from(material.bottom_light_pwm))?;
    fields.u16_be(u16::from(material.light_pwm))
}

/// Writes the block version 3 added: what sliced the file, the resin, and the corrections
/// the firmware may apply. Every correction is written off, because ours are already in
/// the masks (`docs/design/compensation.md`).
pub(crate) fn write_slicer_info(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    write_text(fields, SLICER)?;
    write_text(fields, &job.material.name)?;

    // Distortion correction: off, with the thickness and focal length a vendor file
    // carries anyway. These four are little-endian where the rest of the file is big.
    fields.u8(0)?;
    fields.u32_le(600)?;
    fields.u32_le(300_000)?;

    fields.u8(1)?;
    fields.u16_le(0)?;
    fields.u8(0)?;
    fields.u16_le(1000)?;

    // The greys a layer may carry, which is all of them, and no blur of the firmware's.
    fields.u8(1)?;
    fields.u8(1)?;
    fields.u8(u8::MAX)?;
    fields.u8(0)?;
    fields.u8(2)?;
    fields.bytes(&PAGE_BREAK)
}

/// Writes the footer, which is the header's magic again.
pub(crate) fn write_footer(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.u32_be(MAGIC.len() as u32)?;
    fields.bytes(MAGIC)
}

/// Writes a length-prefixed, nul-terminated string: the length counts the nul.
fn write_text(fields: &mut Fields<'_>, value: &str) -> io::Result<()> {
    fields.u32_be(value.len() as u32 + 1)?;
    fields.bytes(value.as_bytes())?;
    fields.u8(0)
}

/// Writes a length-prefixed UTF-16 string, big-endian, with the length in bytes.
fn write_utf16(fields: &mut Fields<'_>, value: &str) -> io::Result<()> {
    let units: Vec<u16> = value.encode_utf16().collect();
    fields.u32_be((units.len() * 2) as u32)?;
    for unit in units {
        fields.u16_be(unit)?;
    }
    Ok(())
}

/// A number as the text the settings block holds, to `places` decimals and no further.
fn decimals(value: f32, places: usize) -> String {
    let text = format!("{value:.places$}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() { "0" } else { text }.to_owned()
}

fn tenths(seconds: f32) -> u16 {
    (seconds * 10.0).round().clamp(0.0, f32::from(u16::MAX)) as u16
}

fn seconds(value: f32) -> u16 {
    value.round().clamp(0.0, f32::from(u16::MAX)) as u16
}

fn whole_mm(value: f32) -> u16 {
    value.round().clamp(0.0, f32::from(u16::MAX)) as u16
}

/// A speed the container states in millimetres a second, where our profiles use a minute.
fn mm_per_second(mm_min: f32) -> u16 {
    (mm_min / 60.0).round().clamp(0.0, f32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    fn written(write: impl FnOnce(&mut Fields<'_>) -> io::Result<()>) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut fields = Fields::new(&mut buffer);
            write(&mut fields).expect("in memory");
        }
        buffer.into_inner()
    }

    #[test]
    fn the_header_states_the_panel_and_the_layer_count() {
        let job = sample_job(7);
        let bytes = written(|fields| write_header(fields, &job, "CL-60"));

        assert_eq!(&bytes[4..13], MAGIC);
        assert_eq!(u16::from_be_bytes([bytes[13], bytes[14]]), VERSION);
        let tail = bytes.len() - 64;
        assert_eq!(u16::from_be_bytes([bytes[tail - 6], bytes[tail - 5]]), 7);
        assert_eq!(u16::from_be_bytes([bytes[tail - 4], bytes[tail - 3]]), 8);
        assert_eq!(u16::from_be_bytes([bytes[tail - 2], bytes[tail - 1]]), 4);
    }

    #[test]
    fn a_preview_is_two_raw_bytes_a_pixel_and_a_page_break() {
        let bytes = written(|fields| write_previews(fields, None));
        let expected: usize = PREVIEW_SIZES_PX
            .iter()
            .map(|(width, height)| (width * height * 2) as usize + PAGE_BREAK.len())
            .sum();
        assert_eq!(bytes.len(), expected);
        assert_eq!(&bytes[bytes.len() - 2..], &PAGE_BREAK);
    }

    #[test]
    fn the_exposure_is_in_tenths_and_the_bottom_exposure_in_seconds() {
        let mut job = sample_job(4);
        job.material.exposure_s = 2.6;
        job.material.bottom_exposure_s = 32.0;
        job.material.light_off_delay_s = 0.0;
        let bytes = written(|fields| write_settings(fields, &job));

        // Behind the three text fields: exposure, the wait, then the bottom exposure.
        let numbers = &bytes[bytes.len() - 22..];
        assert_eq!(u16::from_be_bytes([numbers[0], numbers[1]]), 26);
        assert_eq!(
            u16::from_be_bytes([numbers[2], numbers[3]]),
            1,
            "a resin with no light-off delay still gets the one second it needs"
        );
        assert_eq!(u16::from_be_bytes([numbers[4], numbers[5]]), 32);
    }

    #[test]
    fn a_speed_in_millimetres_a_minute_is_written_per_second() {
        assert_eq!(mm_per_second(60.0), 1);
        assert_eq!(mm_per_second(180.0), 3);
        assert_eq!(mm_per_second(0.0), 0);
    }

    #[test]
    fn a_number_is_written_as_text_without_trailing_zeros() {
        assert_eq!(decimals(192.0, 2), "192");
        assert_eq!(decimals(120.32, 2), "120.32");
        assert_eq!(decimals(0.05, 3), "0.05");
    }
}
