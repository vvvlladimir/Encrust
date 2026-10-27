use std::io;

use core_format::{Fields, PrintJob};

use crate::blocks::write_second_stage;
use crate::fdg::{Offsets, Totals, anti_alias_level, write_tail};
use crate::writer::encryption_key;

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
    // Bilevel passes per layer, one as in a `.ctb` header; the level proper is at 0xB8.
    fields.u32_le(1)?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;
    fields.zeros(8)?;
    // The vendor's slicer leaves the print's height zero, as in a `.ctb`.
    fields.f32_le(0.0)?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;
    fields.u32_le(encryption_key(job))?;
    write_motion(fields, job, offsets)?;
    write_tail(fields, job, totals)
}

/// From the light-off delays to the software version, 0x5C to 0xC0.
fn write_motion(fields: &mut Fields<'_>, job: &PrintJob, offsets: Offsets) -> io::Result<()> {
    let material = &job.material;
    fields.f32_le(material.bottom_light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(0)?;
    // A lift is both stages' whole climb, as in a `.ctb`.
    fields.f32_le(material.bottom_lift_total_mm())?;
    fields.f32_le(material.bottom_lift_speed_mm_min)?;
    fields.f32_le(material.lift_total_mm())?;
    fields.f32_le(material.lift_speed_mm_min)?;
    fields.f32_le(material.retract_speed_mm_min)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(job.cost().unwrap_or(0.0))?;
    // A word the vendor writer copies from a setting we have not found, as `.fdg` at 0xB4.
    fields.u32_le(0)?;
    fields.u32_le(offsets.machine_name)?;
    fields.u32_le(offsets.machine_name_bytes)?;
    write_second_stage(fields, job)?;
    fields.u32_le(CONSTANT_AT_B0)?;
    fields.u32_le(job.created_minutes())?;
    fields.u32_le(anti_alias_level(job))?;
    // The vendor's slicer states no software version.
    fields.u32_le(0)
}
