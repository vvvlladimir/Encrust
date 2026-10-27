use std::io;

use core_raster::Shading;

use core_format::Fields;
use core_format::PrintJob;
use core_format::Thumbnail;
use core_format::rgb565;

/// Bytes the header occupies. The format fixes it, and `offset of layer content` repeats it.
pub(crate) const HEADER_SIZE: u32 = 0x0002_FB95;

pub(crate) const DELIMITER: [u8; 2] = [0x0D, 0x0A];

/// Closes the file: three pad bytes followed by the magic tag.
pub(crate) const ENDING_STRING: [u8; 11] = [
    0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00,
];

const MAGIC_TAG: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];

/// The only version Elegoo's firmware accepts.
const FORMAT_VERSION: &str = "V3.0";

const SMALL_PREVIEW_PX: usize = 116;
const BIG_PREVIEW_PX: usize = 290;

/// Colour the previews are padded with where the square image does not reach.
const BACKGROUND: [u8; 3] = [0, 0, 0];

pub(crate) fn write(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    write_identity(fields, job)?;
    write_previews(fields, job)?;
    write_geometry(fields, job)?;
    write_exposure(fields, job)?;
    write_motion(fields, job)?;
    write_totals(fields, job)
}

fn write_identity(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    fields.text(FORMAT_VERSION, 4)?;
    fields.bytes(&MAGIC_TAG)?;
    fields.text("Encrust", 32)?;
    fields.text(env!("CARGO_PKG_VERSION"), 24)?;
    fields.text(&job.created_utc(), 24)?;
    fields.text(job.printer.machine_name(), 32)?;
    fields.text(job.printer.machine_name(), 32)?;
    fields.text(&job.material.name, 32)?;

    // A binary file states the vendor's dormant level and blur, as a vendor file does.
    fields.u16_be(match job.raster.shading {
        Shading::Coverage => 8,
        Shading::Binary => job.raster.anti_alias_level(8),
    })?;
    // Zero is what a file rounded to no ladder at all carries, as a vendor file does.
    fields.u16_be(match job.raster.shading {
        Shading::Coverage => job
            .raster
            .grey
            .levels
            .map_or(0, |levels| levels.get().into()),
        Shading::Binary => 0,
    })?;
    fields.u16_be(job.raster.stated_blur_px().into())
}

/// Writes both preview images, black when the job carries no thumbnail.
///
/// Their size is fixed, so the bytes cannot be skipped whatever they hold.
fn write_previews(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    write_preview(fields, job.thumbnail.as_ref(), SMALL_PREVIEW_PX)?;
    fields.bytes(&DELIMITER)?;
    write_preview(fields, job.thumbnail.as_ref(), BIG_PREVIEW_PX)?;
    fields.bytes(&DELIMITER)
}

/// One square preview as RGB565, big-endian like every other field of the container.
fn write_preview(
    fields: &mut Fields<'_>,
    thumbnail: Option<&Thumbnail>,
    side_px: usize,
) -> io::Result<()> {
    let Some(thumbnail) = thumbnail else {
        return fields.zeros(2 * side_px * side_px);
    };
    let side = side_px as u32;
    for pixel in thumbnail.fitted_to(side, side, BACKGROUND).pixels() {
        fields.u16_be(rgb565(*pixel))?;
    }
    Ok(())
}

fn write_geometry(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let raster = &job.raster;
    fields.u32_be(job.layer_count())?;
    fields.u16_be(raster.width_px as u16)?;
    fields.u16_be(raster.height_px as u16)?;
    // How the panel is mounted, not what the masks were drawn as; see docs/formats/goo.md.
    fields.bool(job.printer.mirror_x)?;
    fields.bool(job.printer.mirror_y)?;
    fields.f32_be(raster.width_px as f32 * raster.pitch.x)?;
    fields.f32_be(raster.height_px as f32 * raster.pitch.y)?;
    fields.f32_be(job.printer.build_volume.z)?;
    fields.f32_be(job.nominal_height_mm())
}

fn write_exposure(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    fields.f32_be(job.header_exposure_s())?;
    // False selects "turn off time", the light-off delay; true the static waits below,
    // bottom layers first and then the rest, which take the same rests.
    fields.bool(material.waits.resting())?;
    fields.f32_be(material.light_off_s())?;
    for _ in 0..2 {
        for rest_s in material.waits.rests_s() {
            fields.f32_be(rest_s)?;
        }
    }
    fields.f32_be(material.bottom_exposure_s)?;
    fields.u32_be(material.bottom_layers)
}

fn write_motion(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    fields.f32_be(material.bottom_lift_distance_mm)?;
    fields.f32_be(material.bottom_lift_speed_mm_min)?;
    fields.f32_be(material.lift_distance_mm)?;
    fields.f32_be(material.lift_speed_mm_min)?;
    fields.f32_be(material.bottom_lift_distance_mm)?;
    fields.f32_be(material.bottom_retract_speed_mm_min)?;
    fields.f32_be(material.retract_distance_mm)?;
    fields.f32_be(material.retract_speed_mm_min)?;
    let stage = &material.second_stage;
    for value in [
        stage.bottom_lift_distance_mm,
        stage.bottom_lift_speed_mm_min,
        stage.lift_distance_mm,
        stage.lift_speed_mm_min,
        stage.bottom_retract_distance_mm,
        stage.bottom_retract_speed_mm_min,
        stage.retract_distance_mm,
        stage.retract_speed_mm_min,
    ] {
        fields.f32_be(value)?;
    }
    fields.u16_be(u16::from(material.bottom_light_pwm))?;
    fields.u16_be(u16::from(material.light_pwm))?;
    // Advance mode makes the printer read exposure per layer instead of from the header,
    // which only matters where a layer's exposure differs from the header's.
    fields.bool(job.varies_by_layer())
}

fn write_totals(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    fields.u32_be(job.print_time_s())?;
    fields.f32_be(job.volume_mm3)?;
    fields.f32_be(job.weight_g())?;
    fields.f32_be(job.cost().unwrap_or(0.0))?;
    fields.text(&job.material.price_label(), 8)?;
    fields.u32_be(HEADER_SIZE)?;
    // True means the masks use the full 0x00 to 0xFF range, which coverage shading needs.
    fields.bool(true)?;
    fields.u16_be(job.material.transition_layers)
}

#[cfg(test)]
mod tests;
