use std::io;

use core_raster::Shading;

use core_format::Fields;
use core_format::PrintJob;
use core_format::Thumbnail;
use core_format::rgb565;

use crate::version::{ByteOrder, GooVersion};

/// Bytes the header occupies. The format fixes it, and `offset of layer content` repeats it.
pub(crate) const HEADER_SIZE: u32 = 0x0002_FB95;

pub(crate) const DELIMITER: [u8; 2] = [0x0D, 0x0A];

/// Closes the file: three pad bytes followed by the magic tag.
pub(crate) const ENDING_STRING: [u8; 11] = [
    0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00,
];

const MAGIC_TAG: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];

const SMALL_PREVIEW_PX: usize = 116;
const BIG_PREVIEW_PX: usize = 290;

/// Colour the previews are padded with where the square image does not reach.
const BACKGROUND: [u8; 3] = [0, 0, 0];

/// Writes the header both revisions share, up to and including the transition layers.
pub(crate) fn write(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: GooVersion,
) -> io::Result<()> {
    let order = version.order();
    write_identity(fields, job, version)?;
    write_previews(fields, job, order)?;
    write_geometry(fields, job, order)?;
    write_exposure(fields, job, order)?;
    write_motion(fields, job, version)?;
    write_totals(fields, job, order)
}

fn write_identity(fields: &mut Fields<'_>, job: &PrintJob, version: GooVersion) -> io::Result<()> {
    let order = version.order();
    fields.text(version.tag(), 4)?;
    fields.bytes(&MAGIC_TAG)?;
    fields.text("Encrust", 32)?;
    fields.text(env!("CARGO_PKG_VERSION"), 24)?;
    fields.text(&job.created_utc(), 24)?;
    fields.text(job.printer.machine_name(), 32)?;
    fields.text(job.printer.machine_name(), 32)?;
    fields.text(&job.material.name, 32)?;

    // A binary file states the dormant level and blur a printed file states.
    order.put_u16(
        fields,
        match job.raster.shading {
            Shading::Coverage => 8,
            Shading::Binary => job.raster.anti_alias_level(8),
        },
    )?;
    // Zero is what a file rounded to no ladder at all carries.
    order.put_u16(
        fields,
        match job.raster.shading {
            Shading::Coverage => job
                .raster
                .grey
                .levels
                .map_or(0, |levels| levels.get().into()),
            Shading::Binary => 0,
        },
    )?;
    order.put_u16(fields, job.raster.stated_blur_px().into())
}

/// Writes both preview images, black when the job carries no thumbnail.
///
/// Their size is fixed, so the bytes cannot be skipped whatever they hold.
fn write_previews(fields: &mut Fields<'_>, job: &PrintJob, order: ByteOrder) -> io::Result<()> {
    write_preview(fields, job.thumbnail.as_ref(), SMALL_PREVIEW_PX, order)?;
    fields.bytes(&DELIMITER)?;
    write_preview(fields, job.thumbnail.as_ref(), BIG_PREVIEW_PX, order)?;
    fields.bytes(&DELIMITER)
}

/// One square preview as RGB565, in the byte order of every other field of the revision.
fn write_preview(
    fields: &mut Fields<'_>,
    thumbnail: Option<&Thumbnail>,
    side_px: usize,
    order: ByteOrder,
) -> io::Result<()> {
    let Some(thumbnail) = thumbnail else {
        return fields.zeros(2 * side_px * side_px);
    };
    let side = side_px as u32;
    for pixel in thumbnail.fitted_to(side, side, BACKGROUND).pixels() {
        order.put_u16(fields, rgb565(*pixel))?;
    }
    Ok(())
}

fn write_geometry(fields: &mut Fields<'_>, job: &PrintJob, order: ByteOrder) -> io::Result<()> {
    let raster = &job.raster;
    order.put_u32(fields, job.layer_count())?;
    order.put_u16(fields, raster.width_px as u16)?;
    order.put_u16(fields, raster.height_px as u16)?;
    // The panel's mirror, which the masks carry as well; see docs/formats/goo.md.
    fields.bool(job.printer.mirror.mirrors_x())?;
    fields.bool(false)?;
    order.put_f32(fields, raster.width_px as f32 * raster.pitch.x)?;
    order.put_f32(fields, raster.height_px as f32 * raster.pitch.y)?;
    order.put_f32(fields, job.printer.build_volume.z)?;
    order.put_f32(fields, job.nominal_height_mm())
}

fn write_exposure(fields: &mut Fields<'_>, job: &PrintJob, order: ByteOrder) -> io::Result<()> {
    let material = &job.material;
    order.put_f32(fields, job.header_exposure_s())?;
    // The delay mode is 1 whatever the resin waits by: the field
    // left unused, light-off delay or rests, is zero instead (`docs/formats/goo.md`).
    fields.bool(true)?;
    order.put_f32(fields, material.light_off_s())?;
    for _ in 0..2 {
        for rest_s in material.waits.rests_s() {
            order.put_f32(fields, rest_s)?;
        }
    }
    order.put_f32(fields, material.bottom_exposure_s)?;
    order.put_u32(fields, material.bottom_layers)
}

fn write_motion(fields: &mut Fields<'_>, job: &PrintJob, version: GooVersion) -> io::Result<()> {
    let (order, material) = (version.order(), &job.material);
    let thickness_mm = job.nominal_height_mm();
    // Each stage's lifts, bottom then normal, then its retracts the same way.
    let (bottom, normal) = (material.peel(true), material.peel(false));
    for value in [
        bottom.lift_mm,
        bottom.lift_speed_mm_min,
        normal.lift_mm,
        normal.lift_speed_mm_min,
        version.retract_mm(bottom.retract_mm, thickness_mm),
        bottom.retract_speed_mm_min,
        version.retract_mm(normal.retract_mm, thickness_mm),
        normal.retract_speed_mm_min,
        bottom.second_lift_mm,
        bottom.second_lift_speed_mm_min,
        normal.second_lift_mm,
        normal.second_lift_speed_mm_min,
        bottom.second_retract_mm,
        bottom.second_retract_speed_mm_min,
        normal.second_retract_mm,
        normal.second_retract_speed_mm_min,
    ] {
        order.put_f32(fields, value)?;
    }
    order.put_u16(fields, u16::from(material.bottom_light_pwm))?;
    order.put_u16(fields, u16::from(material.light_pwm))?;
    // Advance mode makes the printer read exposure per layer instead of from the header,
    // which only matters where a layer's exposure differs from the header's.
    fields.bool(job.varies_by_layer())
}

fn write_totals(fields: &mut Fields<'_>, job: &PrintJob, order: ByteOrder) -> io::Result<()> {
    order.put_u32(fields, job.print_time_s())?;
    order.put_f32(fields, job.volume_mm3)?;
    order.put_f32(fields, job.weight_g())?;
    order.put_f32(fields, job.cost().unwrap_or(0.0))?;
    fields.text(&job.material.price_label(), 8)?;
    order.put_u32(fields, HEADER_SIZE)?;
    // True means the masks use the full 0x00 to 0xFF range, which coverage shading needs.
    fields.bool(true)?;
    order.put_u16(fields, job.material.transition_layers)
}

#[cfg(test)]
mod tests;
