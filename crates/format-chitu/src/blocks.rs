use std::io;

use core_format::{Fields, PREVIEW_SIZES_PX, PrintJob, Rgb, minutes_since_epoch, write_preview};

use crate::writer::CtbVersion;

pub(crate) const HEADER_BYTES: u64 = 112;
const PRINT_PARAMETERS_BYTES: u32 = 60;
pub(crate) const SLICER_INFO_BYTES: u32 = 76;
const PRINT_PARAMETERS_V4_BYTES: u32 = 464;
const RESIN_PARAMETERS_BYTES: u32 = 40;

/// The notice CBD Technology puts in every version 4 file, at the length it expects.
///
/// It is a field of the container, not a claim of ours; see `docs/formats/chitu.md`.
const DISCLAIMER: &str = "Layout and record format for the ctb and cbddlp file types are the copyrighted programs or codes of CBD Technology (China) Inc..The Customer or User shall not in any manner reproduce, distribute, modify, decompile, disassemble, decrypt, extract, reverse engineer, lease, assign, or sublicense the said programs or codes.";
const DISCLAIMER_BYTES: u32 = 320;

/// Colour a preview record is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// What the header says about the container it heads, which is all that separates a
/// `.ctb` from a `.cbddlp`. See `docs/formats/chitu.md`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Head {
    pub magic: u32,
    pub version: u32,
    /// Bilevel passes each layer is written in; one means the layers carry grey.
    pub grey_passes: u32,
    /// Bytes of the slicer info block, or zero when the container has none.
    pub slicer_info_bytes: u32,
}

/// Where each block the header points at ended up.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Offsets {
    pub large_preview: u32,
    pub small_preview: u32,
    pub print_parameters: u32,
    pub slicer_info: u32,
    pub layer_table: u32,
}

/// Writes everything between the header and the layer table, and says where it went.
///
/// The header itself is written last, over the space this leaves at the front of the
/// file, because half its fields are offsets into what follows.
pub(crate) fn write_body(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: CtbVersion,
) -> io::Result<Offsets> {
    let mut offsets = write_shared_body(fields, job)?;

    offsets.slicer_info = fields.position()? as u32;
    write_slicer_info(fields, job, version, offsets.slicer_info)?;
    write_v4_parameters(fields, job, version)?;

    offsets.layer_table = fields.position()? as u32;
    Ok(offsets)
}

/// Writes everything between the header and the layer table of a `.cbddlp`, which is
/// where the older container stops: it carries no slicer info and no version 4 block.
pub(crate) fn write_cbddlp_body(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<Offsets> {
    let mut offsets = write_shared_body(fields, job)?;
    offsets.layer_table = fields.position()? as u32;
    Ok(offsets)
}

/// The two previews and the print parameters, which every version of the container holds
/// in the same shape at the same place.
fn write_shared_body(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<Offsets> {
    let mut offsets = Offsets::default();

    fields.seek_to(HEADER_BYTES)?;

    // The records have to be here whatever they hold, because the header addresses them.
    let large = preview_pixels(job, PREVIEW_SIZES_PX[0]);
    offsets.large_preview =
        write_preview(fields, PREVIEW_SIZES_PX[0].0, PREVIEW_SIZES_PX[0].1, &large)?;
    let small = preview_pixels(job, PREVIEW_SIZES_PX[1]);
    offsets.small_preview =
        write_preview(fields, PREVIEW_SIZES_PX[1].0, PREVIEW_SIZES_PX[1].1, &small)?;

    offsets.print_parameters = fields.position()? as u32;
    write_print_parameters(fields, job)?;
    Ok(offsets)
}

/// The job's thumbnail cut to one record's shape, or black when there is none.
fn preview_pixels(job: &PrintJob, (width, height): (u32, u32)) -> Vec<Rgb> {
    match job.thumbnail.as_ref() {
        Some(thumbnail) => thumbnail
            .fitted_to(width, height, PREVIEW_BACKGROUND)
            .pixels()
            .to_vec(),
        None => vec![PREVIEW_BACKGROUND; (width * height) as usize],
    }
}

pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    head: Head,
    offsets: Offsets,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);

    fields.u32_le(head.magic)?;
    fields.u32_le(head.version)?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;
    fields.zeros(8)?;
    fields.f32_le(job.height_mm())?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.u32_le(offsets.large_preview)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(job.layer_count())?;
    fields.u32_le(offsets.small_preview)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(job.printer.mirror_x || job.printer.mirror_y))?;
    fields.u32_le(offsets.print_parameters)?;
    fields.u32_le(PRINT_PARAMETERS_BYTES)?;

    fields.u32_le(head.grey_passes)?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;

    // Zero leaves the layer data in the clear; see docs/decisions/0046.
    fields.u32_le(0)?;
    fields.u32_le(offsets.slicer_info)?;
    fields.u32_le(head.slicer_info_bytes)
}

/// Lays the print parameters down again, for the totals only known once the stack is
/// cut; see `docs/decisions/0067-the-resin-volume-is-patched-in-at-finish.md`.
pub(crate) fn rewrite_print_parameters(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    at: u32,
) -> io::Result<()> {
    fields.seek_to(u64::from(at))?;
    write_print_parameters(fields, job)
}

fn write_print_parameters(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;

    fields.f32_le(material.bottom_lift_distance_mm)?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_distance_mm)?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(0.0)?;
    fields.f32_le(material.light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.zeros(16)
}

fn write_slicer_info(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: CtbVersion,
    at: u32,
) -> io::Result<()> {
    let name = job.printer.machine_name().as_bytes();
    let machine_name_address = at + SLICER_INFO_BYTES;
    let v4_parameters_address = machine_name_address + name.len() as u32 + DISCLAIMER_BYTES;

    fields.zeros(28)?;
    fields.u32_le(machine_name_address)?;
    fields.u32_le(name.len() as u32)?;
    fields.u8(0x0F)?;
    fields.u16_le(0)?;
    fields.u8(version.per_layer_settings())?;
    fields.u32_le(minutes_since_epoch())?;
    fields.u32_le(1)?;
    fields.u32_le(version.software_version())?;
    let [_, after_lift_s, after_retract_s] = job.material.waits.rests_s();
    fields.f32_le(after_retract_s)?;
    fields.f32_le(after_lift_s)?;
    fields.u32_le(u32::from(job.material.transition_layers))?;
    fields.u32_le(v4_parameters_address)?;
    fields.zeros(8)?;
    fields.bytes(name)
}

fn write_v4_parameters(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: CtbVersion,
) -> io::Result<()> {
    let disclaimer_address = fields.position()? as u32;
    fields.text(DISCLAIMER, DISCLAIMER_BYTES as usize)?;

    let at = fields.position()? as u32;
    let resin_parameters_address = match version {
        CtbVersion::V4 => 0,
        CtbVersion::V5 => at + PRINT_PARAMETERS_V4_BYTES,
    };

    fields.f32_le(job.material.bottom_retract_speed_mm_min)?;
    fields.f32_le(0.0)?;
    fields.u32_le(0)?;

    // Two fields the machine checks for the literal 4.0, and one for the literal 5.
    fields.f32_le(4.0)?;
    fields.u32_le(0)?;
    fields.f32_le(4.0)?;

    let [before_lift_s, after_lift_s, after_retract_s] = job.material.waits.rests_s();
    fields.f32_le(after_retract_s)?;
    fields.f32_le(after_lift_s)?;
    fields.f32_le(before_lift_s)?;
    fields.zeros(8)?;
    fields.u32_le(0)?;
    fields.u32_le(5)?;
    fields.u32_le(job.layer_count().saturating_sub(1))?;
    fields.zeros(16)?;
    fields.u32_le(disclaimer_address)?;
    fields.u32_le(DISCLAIMER_BYTES)?;
    fields.u32_le(resin_parameters_address)?;
    fields.zeros(380)?;

    if version == CtbVersion::V5 {
        write_resin_parameters(fields, job, resin_parameters_address)?;
    }
    Ok(())
}

fn write_resin_parameters(fields: &mut Fields<'_>, job: &PrintJob, at: u32) -> io::Result<()> {
    let resin_type = b"UV Resin";
    let resin_name = job.material.name.as_bytes();
    let machine_name = job.printer.machine_name().as_bytes();

    let type_address = at + RESIN_PARAMETERS_BYTES;
    let name_address = type_address + resin_type.len() as u32;
    let machine_address = name_address + resin_name.len() as u32;

    fields.u32_le(0)?;
    fields.bytes(&[0, 0, 0, 0])?;
    fields.u32_le(machine_address)?;
    fields.u32_le(resin_type.len() as u32)?;
    fields.u32_le(type_address)?;
    fields.u32_le(resin_name.len() as u32)?;
    fields.u32_le(name_address)?;
    fields.u32_le(machine_name.len() as u32)?;
    fields.f32_le(job.material.density_g_cm3)?;
    fields.u32_le(0)?;

    fields.bytes(resin_type)?;
    fields.bytes(resin_name)?;
    fields.bytes(machine_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_disclaimer_fits_the_field_the_format_fixes() {
        assert_eq!(
            DISCLAIMER.len(),
            DISCLAIMER_BYTES as usize,
            "the notice is written whole, not truncated, at 320 bytes"
        );
    }
}
