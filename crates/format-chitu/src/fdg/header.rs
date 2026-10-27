use std::io;

use core_format::{Fields, PrintJob};

use crate::writer::CtbVersion;

/// Bytes of the header; the first preview follows it.
pub(crate) const HEADER_BYTES: u64 = 0x134;

pub(crate) const MAGIC: u32 = 0xBD3C_7AC8;
const VERSION: u32 = 4;

/// Where the block of later parameters sits inside the header, and its length.
const PARAMETERS_AT: u32 = 0xD8;
const PARAMETERS_BYTES: u32 = 0x50;

/// Where each block the header points at ended up.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Offsets {
    pub large_preview: u32,
    pub small_preview: u32,
    pub machine_name: u32,
    pub machine_name_bytes: u32,
    pub layer_table: u32,
}

/// What the header sums over the stack, only known once every layer is written.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Totals {
    /// Exposure, light-off delay and rests of every layer, seconds.
    pub layer_time_s: f32,
    pub largest_layer_bytes: u32,
    pub last_bottom_index: u32,
    pub distinct_heights: u32,
}

pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    offsets: Offsets,
    totals: Totals,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);

    fields.u32_le(MAGIC)?;
    fields.u32_le(VERSION)?;
    fields.u32_le(job.layer_count())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(u32::from(printer.mirror_x || printer.mirror_y))?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.u32_le(offsets.large_preview)?;
    fields.u32_le(offsets.small_preview)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(1)?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.zeros(8)?;
    fields.f32_le(job.height_mm())?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;

    // Zero leaves the layer data in the clear, as for `.ctb`; see docs/decisions/0046.
    fields.u32_le(0)?;
    fields.u32_le(1)?;
    fields.u32_le(0x44)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(0.0)?;
    fields.u32_le(offsets.machine_name)?;
    fields.u32_le(offsets.machine_name_bytes)?;
    write_motion(fields, job)?;
    write_tail(fields, job, totals)
}

/// From the light-off delays to the software version, 0x7C to 0xC0.
fn write_motion(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    fields.f32_le(material.light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(0)?;
    fields.f32_le(material.bottom_lift_distance_mm)?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_distance_mm)?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    // The second lift and retract stage, and one more word of it, which we do not use.
    fields.zeros(28)?;
    fields.u32_le(job.created_minutes())?;
    fields.u32_le(CtbVersion::V4.software_version())
}

/// From the rests to the end, 0xC0 to 0x134, which `.phz` holds at the same offsets.
pub(crate) fn write_tail(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    totals: Totals,
) -> io::Result<()> {
    let material = &job.material;
    let [_, after_lift_s, after_retract_s] = material.waits.rests_s();
    fields.f32_le(after_retract_s)?;
    fields.f32_le(after_lift_s)?;
    fields.u16_le(material.transition_layers)?;
    fields.u16_le(0)?;
    fields.u32_le(PARAMETERS_AT)?;
    fields.u32_le(PARAMETERS_BYTES)?;
    fields.u32_le(0)?;
    write_parameters(fields, job, totals)
}

/// The block from 0xD8 to the end of the header.
fn write_parameters(fields: &mut Fields<'_>, job: &PrintJob, totals: Totals) -> io::Result<()> {
    fields.f32_le(job.material.bottom_retract_speed_mm_min)?;
    fields.zeros(36)?;
    fields.f32_le(totals.layer_time_s)?;
    fields.u32_le(totals.largest_layer_bytes)?;
    fields.u32_le(totals.last_bottom_index)?;
    fields.u32_le(totals.distinct_heights)?;
    fields.zeros(36)
}
