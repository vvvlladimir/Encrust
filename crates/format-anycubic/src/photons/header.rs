use std::io;

use core_format::{Fields, PrintJob, Rgb, Thumbnail};

/// The two words a file opens with, which name the container.
pub(crate) const TAG: u32 = 2;
pub(crate) const SUBTAG: u16 = 0x31;

/// Bytes from the start of the file to the preview: the tag, then the header proper.
pub(crate) const HEADER_BYTES: u64 = 6 + 0x5C;

/// The one preview the container holds.
pub(crate) const PREVIEW_PX: (u32, u32) = (224, 168);

/// Two words of the header the vendor writer always sets, between the preview's sides.
const PREVIEW_WORDS: [u32; 2] = [42, 10];

const BACKGROUND: Rgb = [0, 0, 0];

/// Writes the tag and the header; speeds go in millimetres a second.
pub(crate) fn write_header(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    fields.u32_be(TAG)?;
    fields.u16_be(SUBTAG)?;
    fields.f64_be(f64::from(job.raster.pitch.x))?;
    fields.f64_be(f64::from(job.nominal_height_mm()))?;
    fields.f64_be(f64::from(job.header_exposure_s()))?;
    fields.f64_be(f64::from(material.light_off_s()))?;
    fields.f64_be(f64::from(material.bottom_exposure_s))?;
    fields.u32_be(material.bottom_layers)?;
    fields.f64_be(f64::from(material.lift_distance_mm))?;
    fields.f64_be(f64::from(material.lift_speed_mm_min / 60.0))?;
    fields.f64_be(f64::from(material.retract_speed_mm_min / 60.0))?;
    fields.f64_be(f64::from(job.volume_mm3 / 1000.0))?;
    let (width, height) = PREVIEW_PX;
    fields.u32_be(width)?;
    fields.u32_be(PREVIEW_WORDS[0])?;
    fields.u32_be(height)?;
    fields.u32_be(PREVIEW_WORDS[1])
}

/// Writes the preview: raw little-endian words of five bits a channel, red highest and
/// bit 5 clear, which is the `.ctb` preview's colour without its runs.
pub(crate) fn write_preview(
    fields: &mut Fields<'_>,
    thumbnail: Option<&Thumbnail>,
) -> io::Result<()> {
    let (width, height) = PREVIEW_PX;
    let image = match thumbnail {
        Some(thumbnail) => thumbnail.fitted_to(width, height, BACKGROUND),
        None => Thumbnail::filled(width, height, BACKGROUND),
    };
    let mut data = Vec::with_capacity(image.pixels().len() * 2);
    for &[r, g, b] in image.pixels() {
        let word = (u16::from(r >> 3) << 11) | (u16::from(g >> 3) << 6) | u16::from(b >> 3);
        data.extend_from_slice(&word.to_le_bytes());
    }
    fields.bytes(&data)
}
