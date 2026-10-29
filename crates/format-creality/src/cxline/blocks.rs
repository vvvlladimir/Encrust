//! Everything of a `.cxline` that is not a layer. Field by field in `docs/formats/cxline.md`.

use std::io;

use core_format::{Fields, PrintJob};

use crate::v3::blocks::{
    PAGE_BREAK, as_text, mm_per_second, seconds, tenths, whole_mm, whole_seconds, write_text,
    write_utf16,
};

/// What a file is headed by, nul included, and what it ends with.
pub(crate) const MAGIC: &[u8] = b"CXLINEV2\0";
pub(crate) const FOOTER: &[u8] = b"CXSW3D\0";
pub(crate) const VERSION: u16 = 1;

/// Bytes of the block behind the panel, and where in it the volume stands.
pub(crate) const MACHINE_BLOCK_BYTES: usize = 64;
const VOLUME_IN_BLOCK: u64 = 0x12;

/// The model code the header names: the D158 is `CD60` and every other name passes
/// through as it stands, the D160's included.
pub fn cxline_model_code(machine_name: &str) -> &str {
    match machine_name {
        "D158" => "CD60",
        other => other,
    }
}

/// Writes the header and returns where its volume field stands, which is only known at the end.
pub(crate) fn write_header(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<u64> {
    fields.u32_be(MAGIC.len() as u32)?;
    fields.bytes(MAGIC)?;
    fields.u16_be(VERSION)?;
    write_text(fields, cxline_model_code(job.printer.machine_name()))?;
    fields.u16_be(job.layer_count() as u16)?;
    fields.u16_be(job.raster.width_px as u16)?;
    fields.u16_be(job.raster.height_px as u16)?;

    let volume_at = fields.position()? + VOLUME_IN_BLOCK;
    fields.u32_be((job.printer.build_volume.z * 1000.0).round() as u32)?;
    // The shrinkage flag, the support time offset and the firmware's blur: ours are in the
    // masks already (`docs/design/compensation.md`).
    fields.u32_be(0)?;
    fields.f32_be(0.0)?;
    // The blur a binary panel states.
    fields.u32_be(u32::from(job.raster.stated_blur_px()))?;
    fields.u16_be(tenths(job.header_exposure_s()))?;
    fields.f32_be(job.volume_mm3 / 1000.0)?;
    fields.zeros(MACHINE_BLOCK_BYTES - VOLUME_IN_BLOCK as usize - 4)?;
    Ok(volume_at)
}

/// Writes the panel and the layer height as text, then the settings the firmware prints by.
pub(crate) fn write_settings(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (material, display) = (&job.material, &job.printer.display);
    write_utf16(fields, &as_text(display.width_mm))?;
    write_utf16(fields, &as_text(display.height_mm))?;
    write_utf16(fields, &as_text(job.nominal_height_mm()))?;

    // The exposure is in the block above, in tenths; its old place here stays zero.
    fields.u16_be(0)?;
    fields.u16_be(whole_seconds(material.light_off_s()))?;
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

/// Writes the block behind the area table: the firmware's time compensation, off.
pub(crate) fn write_corrections(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.zeros(8)?;
    fields.bytes(&PAGE_BREAK)
}

pub(crate) fn write_footer(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.u32_be(FOOTER.len() as u32)?;
    fields.bytes(FOOTER)
}
