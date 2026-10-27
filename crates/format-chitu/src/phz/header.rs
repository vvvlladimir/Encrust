use std::io;

use core_format::{Fields, PrintJob};

use crate::fdg::{Offsets, Totals, write_tail};
use crate::writer::CtbVersion;

pub(crate) const MAGIC: u32 = 0x9FDA_83AE;
const VERSION: u32 = 4;

/// A word of the header the vendor writer always sets.
const CONSTANT_AT_B0: u32 = 0x14;

pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    offsets: Offsets,
    totals: Totals,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);

    fields.u32_le(MAGIC)?;
    fields.u32_le(VERSION)?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.u32_le(offsets.large_preview)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(job.layer_count())?;
    fields.u32_le(offsets.small_preview)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(printer.mirror_x || printer.mirror_y))?;
    fields.u32_le(1)?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;
    fields.zeros(8)?;
    fields.f32_le(job.height_mm())?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;
    // Zero leaves the layer data in the clear, as for `.ctb`; see docs/decisions/0046.
    fields.u32_le(0)?;
    write_motion(fields, job, offsets)?;
    write_tail(fields, job, totals)
}

/// From the light-off delays to the software version, 0x5C to 0xC0.
fn write_motion(fields: &mut Fields<'_>, job: &PrintJob, offsets: Offsets) -> io::Result<()> {
    let material = &job.material;
    fields.f32_le(material.light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(0)?;
    // The lifts are each the sum of both stages, of which we use the first alone.
    fields.f32_le(material.bottom_lift_distance_mm)?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_distance_mm)?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(0.0)?;
    fields.u32_le(0)?;
    fields.u32_le(offsets.machine_name)?;
    fields.u32_le(offsets.machine_name_bytes)?;
    // The second lift and retract stage, which we do not use.
    fields.zeros(24)?;
    fields.u32_le(CONSTANT_AT_B0)?;
    fields.u32_le(job.created_minutes())?;
    fields.u32_le(1)?;
    fields.u32_le(CtbVersion::V4.software_version())
}
