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

    fields.u16_be(match job.raster.shading {
        Shading::Coverage => 8,
        Shading::Binary => 1,
    })?;
    // Zero is what a file rounded to no ladder at all carries, as a vendor file does.
    fields.u16_be(match job.raster.shading {
        Shading::Coverage => job
            .raster
            .grey
            .levels
            .map_or(0, |levels| levels.get().into()),
        Shading::Binary => 1,
    })?;
    fields.u16_be(match job.raster.shading {
        Shading::Coverage => job.raster.blur_px.into(),
        Shading::Binary => 0,
    })
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
    // Both second stages are unused: eight zeroed distances and speeds.
    for _ in 0..8 {
        fields.f32_be(0.0)?;
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
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    fn header_of(job: &PrintJob) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut fields = Fields::new(&mut buffer);
        write(&mut fields, job).expect("in-memory write");
        buffer.into_inner()
    }

    /// Where the exposure delay mode sits, then the light-off time, then the six waits.
    const DELAY_MODE_OFFSET: usize = 195_340;

    fn f32_at(header: &[u8], offset: usize) -> f32 {
        f32::from_be_bytes([
            header[offset],
            header[offset + 1],
            header[offset + 2],
            header[offset + 3],
        ])
    }

    #[test]
    fn a_resting_resin_selects_the_static_waits_and_fills_them() {
        let mut job = sample_job(10);
        job.material.waits = printer_profiles::Waits {
            mode: printer_profiles::WaitMode::Rest,
            before_lift_s: 1.5,
            after_lift_s: 0.5,
            after_retract_s: 2.0,
        };
        let header = header_of(&job);
        assert_eq!(
            header[DELAY_MODE_OFFSET], 1,
            "static waits, not turn-off time"
        );
        assert!(
            f32_at(&header, DELAY_MODE_OFFSET + 1).abs() < f32::EPSILON,
            "no light-off delay"
        );
        let waits: Vec<f32> = (0..6)
            .map(|index| f32_at(&header, DELAY_MODE_OFFSET + 5 + 4 * index))
            .collect();
        assert_eq!(
            waits,
            [1.5, 0.5, 2.0, 1.5, 0.5, 2.0],
            "bottom layers, then the rest"
        );
    }

    #[test]
    fn a_resin_holding_the_light_off_writes_no_waits() {
        let job = sample_job(10);
        let header = header_of(&job);
        assert_eq!(header[DELAY_MODE_OFFSET], 0);
        assert!(
            (f32_at(&header, DELAY_MODE_OFFSET + 1) - job.material.light_off_delay_s).abs() < 1e-6
        );
    }

    /// Both machine-name fields, the first straight after the three 32-byte strings.
    const MACHINE_NAME_OFFSET: usize = 4 + 8 + 32 + 24 + 24;

    /// Total price, then the eight-byte unit it is quoted in.
    const PRICE_OFFSET: usize = 195_458;

    fn text_at(header: &[u8], offset: usize, width: usize) -> String {
        let field = &header[offset..offset + width];
        let end = field.iter().position(|&b| b == 0).unwrap_or(width);
        String::from_utf8(field[..end].to_vec()).expect("a header string is utf-8")
    }

    #[test]
    fn both_machine_fields_carry_the_name_the_firmware_matches() {
        let mut job = sample_job(10);
        job.printer.name = "Saturn 4 Ultra".to_owned();
        job.printer.machine_name = Some("ELEGOO Saturn 4 Ultra".to_owned());
        let header = header_of(&job);
        assert_eq!(
            text_at(&header, MACHINE_NAME_OFFSET, 32),
            "ELEGOO Saturn 4 Ultra"
        );
        assert_eq!(
            text_at(&header, MACHINE_NAME_OFFSET + 32, 32),
            "ELEGOO Saturn 4 Ultra"
        );
    }

    #[test]
    fn the_totals_carry_what_the_resin_costs_and_what_it_is_quoted_in() {
        let mut job = sample_job(10);
        job.volume_mm3 = 1_000_000.0;
        job.material.details.price = 20.0;
        job.material.details.currency = "€".to_owned();
        job.material.details.price_per = printer_profiles::PriceUnit::Litre;
        let header = header_of(&job);
        assert!(
            (f32_at(&header, PRICE_OFFSET) - 20.0).abs() < 1e-3,
            "a litre of resin at 20 a litre"
        );
        assert_eq!(text_at(&header, PRICE_OFFSET + 4, 8), "€/L");
    }

    #[test]
    fn the_header_is_exactly_the_size_the_format_fixes() {
        let header = header_of(&sample_job(10));
        assert_eq!(
            header.len(),
            HEADER_SIZE as usize,
            "the format fixes the header at 0x2FB95 bytes and layer content starts there"
        );
    }

    #[test]
    fn the_header_opens_with_the_version_and_the_magic_tag() {
        let header = header_of(&sample_job(10));
        assert_eq!(&header[..4], b"V3.0");
        assert_eq!(&header[4..12], &MAGIC_TAG);
    }

    #[test]
    fn the_layer_content_offset_is_the_header_size() {
        let header = header_of(&sample_job(10));
        let offset = HEADER_SIZE as usize - 7;
        assert_eq!(
            u32::from_be_bytes([
                header[offset],
                header[offset + 1],
                header[offset + 2],
                header[offset + 3]
            ]),
            HEADER_SIZE
        );
    }

    /// Anti-aliasing level sits after the eight fixed-width strings, the grey level next.
    const AA_OFFSET: usize = 4 + 8 + 32 + 24 + 24 + 32 + 32 + 32;

    fn u16_at(header: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes([header[offset], header[offset + 1]])
    }

    #[test]
    fn binary_shading_is_reported_as_a_single_anti_aliasing_level() {
        let mut job = sample_job(10);
        job.raster.shading = Shading::Binary;
        let header = header_of(&job);
        assert_eq!(u16_at(&header, AA_OFFSET), 1);
        assert_eq!(u16_at(&header, AA_OFFSET + 2), 1, "and a single grey level");
    }

    #[test]
    fn the_grey_level_is_the_count_the_masks_were_rounded_to() {
        let mut job = sample_job(10);
        job.raster.grey.levels = std::num::NonZeroU8::new(4);
        let header = header_of(&job);
        assert_eq!(u16_at(&header, AA_OFFSET), 8, "the masks are still 8-bit");
        assert_eq!(u16_at(&header, AA_OFFSET + 2), 4);
    }

    #[test]
    fn unrounded_grey_is_reported_as_no_level_count_at_all() {
        let header = header_of(&sample_job(10));
        assert_eq!(u16_at(&header, AA_OFFSET + 2), 0);
    }

    #[test]
    fn the_blur_level_is_the_radius_the_edges_were_faded_over() {
        let mut job = sample_job(10);
        assert_eq!(
            u16_at(&header_of(&job), AA_OFFSET + 4),
            0,
            "sharp by default"
        );
        job.raster.blur_px = 2;
        assert_eq!(u16_at(&header_of(&job), AA_OFFSET + 4), 2);
        job.raster.shading = Shading::Binary;
        assert_eq!(
            u16_at(&header_of(&job), AA_OFFSET + 4),
            0,
            "a binary mask is never blurred"
        );
    }
}
