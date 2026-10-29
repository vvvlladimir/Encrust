use std::io;

use core_format::{ChituTotals, Fields, PrintJob};

use crate::blocks::{write_second_stage, write_v4_motion};
use crate::writer::encryption_key;

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
    pub stack: ChituTotals,
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
    // The print's height is left zero, as in a `.ctb`.
    fields.f32_le(0.0)?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;

    fields.u32_le(encryption_key(job))?;
    fields.u32_le(anti_alias_level(job))?;
    fields.u32_le(0x44)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(job.cost().unwrap_or(0.0))?;
    fields.u32_le(offsets.machine_name)?;
    fields.u32_le(offsets.machine_name_bytes)?;
    write_motion(fields, job)?;
    write_tail(fields, job, totals)
}

/// The anti-alias level the header states: the setting the edges
/// were drawn with, at most 16, though the layer codec keeps only lit and unlit.
pub(crate) fn anti_alias_level(job: &PrintJob) -> u32 {
    u32::from(job.raster.anti_alias_level(16))
}

/// From the light-off delays to the software version, 0x7C to 0xC0.
fn write_motion(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    fields.f32_le(material.bottom_light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(0)?;
    // A lift is both stages' whole climb, as in a `.ctb`.
    fields.f32_le(material.bottom_lift_total_mm())?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_total_mm())?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    write_second_stage(fields, job)?;
    // A word of unknown meaning, written zero.
    fields.u32_le(0)?;
    fields.u32_le(job.created_minutes())?;
    // No software version is stated.
    fields.u32_le(0)
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

/// The block from 0xD8 to the end of the header: its first 40 bytes are `.ctb`'s v4
/// parameters.
fn write_parameters(fields: &mut Fields<'_>, job: &PrintJob, totals: Totals) -> io::Result<()> {
    write_v4_motion(fields, job)?;
    fields.f32_le(totals.stack.layer_time_s)?;
    fields.u32_le(totals.stack.largest_layer_bytes)?;
    fields.u32_le(totals.last_bottom_index)?;
    fields.u32_le(totals.distinct_heights)?;
    fields.zeros(36)
}
