//! Everything of a version 4 file that is not a layer: the header, the two previews, the
//! print parameters, the slicer block and the layer table. Field by field in
//! `docs/formats/creality.md`.

use std::io;

use core_format::{Fields, PREVIEW_HEADER_BYTES, PrintJob, Rgb, encode_rgb15};

use crate::family::MAGIC;

/// The revision this writes.
pub(crate) const VERSION: u16 = 4;

/// Bytes of one row of the layer table.
pub(crate) const LAYER_ROW_BYTES: u64 = 40;

/// Bytes of the motion block in front of a layer's run-length data. The row's size field
/// counts it along with the data, which is how a layer is found.
pub(crate) const LAYER_BLOCK_BYTES: u32 = 44;

const PRINT_PARAMETERS_BYTES: u32 = 68;
const SLICER_INFO_BYTES: u32 = 76;

/// The two previews, smallest first, as the container holds them.
const PREVIEW_SIZES_PX: [(u32, u32); 2] = [(120, 120), (300, 300)];

/// Colour a preview is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Where each block the header points at ended up.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Offsets {
    pub small_preview: u32,
    pub large_preview: u32,
    pub print_parameters: u32,
    pub slicer_info: u32,
    pub layer_table: u32,
}

/// Bytes the header takes, which is fixed but for the model code it carries.
pub(crate) fn header_bytes(model: &str) -> u64 {
    // The magic and its length, the revision, the model as a counted string with its nul,
    // the panel, and the twenty-two fields behind it.
    4 + MAGIC.len() as u64 + 2 + (4 + model.len() as u64 + 1) + 4 + 76
}

/// Writes the header, which is written last because half its fields address what follows.
pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    model: &str,
    offsets: Offsets,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);

    // The magic and the revision are big-endian; every field behind them is not.
    fields.u32_be(MAGIC.len() as u32)?;
    fields.bytes(MAGIC)?;
    fields.u16_be(VERSION)?;
    fields.u32_be(model.len() as u32 + 1)?;
    fields.bytes(model.as_bytes())?;
    fields.u8(0)?;

    fields.u16_le(raster.width_px as u16)?;
    fields.u16_le(raster.height_px as u16)?;
    fields.f32_le(printer.build_volume.x)?;
    fields.f32_le(printer.build_volume.y)?;
    fields.f32_le(printer.build_volume.z)?;
    fields.f32_le(job.height_mm())?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.u32_le(material.bottom_layers)?;
    fields.u32_le(offsets.small_preview)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(job.layer_count())?;
    fields.u32_le(offsets.large_preview)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(printer.mirror_x || printer.mirror_y))?;
    fields.u32_le(offsets.print_parameters)?;
    fields.u32_le(PRINT_PARAMETERS_BYTES)?;

    // The masks carry eight bits of grey each, so nothing is stacked in passes.
    fields.u32_le(1)?;
    fields.u16_le(u16::from(material.light_pwm))?;
    fields.u16_le(u16::from(material.bottom_light_pwm))?;

    // Zero leaves the layer data in the clear; see docs/decisions/0046.
    fields.u32_le(0)?;
    fields.u32_le(offsets.slicer_info)?;
    fields.u32_le(SLICER_INFO_BYTES)
}

/// Writes the two previews, the print parameters and the slicer block, in that order, and
/// says where each landed. The caller stands the cursor at the end of the header first.
pub(crate) fn write_body(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<Offsets> {
    let small_preview = write_preview(fields, job, PREVIEW_SIZES_PX[0])?;
    let large_preview = write_preview(fields, job, PREVIEW_SIZES_PX[1])?;

    let print_parameters = fields.position()? as u32;
    write_print_parameters(fields, job)?;

    let slicer_info = fields.position()? as u32;
    write_slicer_info(fields, job)?;

    Ok(Offsets {
        small_preview,
        large_preview,
        print_parameters,
        slicer_info,
        layer_table: fields.position()? as u32,
    })
}

/// One preview record and its pixels, and where the record began.
///
/// The record is the Chitu family's, run-length RGB15 and all, which is what version 4
/// takes over from it wholesale.
fn write_preview(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    (width, height): (u32, u32),
) -> io::Result<u32> {
    let pixels = match job.thumbnail.as_ref() {
        Some(thumbnail) => thumbnail
            .fitted_to(width, height, PREVIEW_BACKGROUND)
            .pixels()
            .to_vec(),
        None => vec![PREVIEW_BACKGROUND; (width * height) as usize],
    };
    let data = encode_rgb15(&pixels);
    let record = fields.position()?;

    fields.u32_le(width)?;
    fields.u32_le(height)?;
    fields.u32_le((record + PREVIEW_HEADER_BYTES) as u32)?;
    fields.u32_le(data.len() as u32)?;
    fields.zeros(16)?;
    fields.bytes(&data)?;
    Ok(record as u32)
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
    fields.f32_le(job.cost().unwrap_or_default())?;
    fields.f32_le(material.light_off_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.u32_le(material.bottom_layers)?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.zeros(16)
}

/// The block the per-layer tables and the rests are announced in.
fn write_slicer_info(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    let [before_lift_s, after_lift_s, after_retract_s] = material.waits.rests_s();

    // The second lift and retract are off: a stack of ours moves in one stroke.
    fields.zeros(4 * 6)?;
    fields.f32_le(after_lift_s)?;
    fields.u32_le(u32::from(job.printer.firmware.per_layer_settings))?;
    fields.u32_le(job.created_minutes())?;
    fields.u32_le(1)?;

    // No slicer names itself here: the field is a capability stamp of the vendor's own.
    fields.u32_le(0)?;
    fields.f32_le(after_retract_s)?;
    fields.f32_le(before_lift_s)?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.bottom_exposure_s)?;

    // A second copy of the rest after lift, which a real file carries as well.
    fields.f32_le(after_lift_s)?;
    fields.u32_le(u32::from(material.transition_layers))?;
    fields.zeros(8)
}

/// One row of the layer table, filled in as its layer is written.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayerRow {
    pub z_mm: f32,
    pub exposure_s: f32,
    pub light_off_delay_s: f32,
    /// Where this layer's motion block begins, which the data follows.
    pub address: u32,
    /// That block and the run-length data behind it, together.
    pub size: u32,
    /// What the layer cures, square millimetres times a thousand.
    pub area: u32,
}

impl LayerRow {
    pub(crate) fn write(&self, fields: &mut Fields<'_>) -> io::Result<()> {
        fields.f32_le(self.z_mm)?;
        fields.f32_le(self.exposure_s)?;
        fields.f32_le(self.light_off_delay_s)?;
        fields.u32_le(self.address)?;
        fields.u32_le(self.size)?;

        // Data type zero is the only one these machines write: a layer of runs.
        fields.u32_le(0)?;
        fields.u32_le(0)?;
        fields.u32_le(self.area)?;
        fields.zeros(8)
    }
}

/// Writes the motion block that sits in front of a layer's run-length data.
pub(crate) fn write_layer_block(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    index: u32,
) -> io::Result<()> {
    let material = &job.material;
    let bottom = material.is_bottom_layer(index);
    let (lift_distance, lift_speed) = if bottom {
        (
            material.bottom_lift_distance_mm,
            material.bottom_lift_speed_mm_min,
        )
    } else {
        (material.lift_distance_mm, material.lift_speed_mm_min)
    };
    let retract_speed = if bottom {
        material.bottom_retract_speed_mm_min
    } else {
        material.retract_speed_mm_min
    };
    let light_pwm = if bottom {
        material.bottom_light_pwm
    } else {
        material.light_pwm
    };
    let [before_lift_s, after_lift_s, after_retract_s] = material.waits.rests_s();

    fields.f32_le(lift_distance)?;
    fields.f32_le(lift_speed)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(retract_speed)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(before_lift_s)?;
    fields.f32_le(after_lift_s)?;
    fields.f32_le(after_retract_s)?;
    fields.f32_le(f32::from(light_pwm))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    fn written(write: impl FnOnce(&mut Fields<'_>) -> io::Result<()>) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut fields = Fields::new(&mut buffer);
            write(&mut fields).expect("in memory");
        }
        buffer.into_inner()
    }

    #[test]
    fn the_header_is_as_long_as_it_says_it_is() {
        let job = sample_job(3);
        let bytes = written(|fields| write_header(fields, &job, "CL-103L", Offsets::default()));
        assert_eq!(bytes.len() as u64, header_bytes("CL-103L"));
    }

    #[test]
    fn the_two_settings_blocks_are_the_sizes_the_header_states() {
        let job = sample_job(3);
        assert_eq!(
            written(|fields| write_print_parameters(fields, &job)).len() as u32,
            PRINT_PARAMETERS_BYTES
        );
        assert_eq!(
            written(|fields| write_slicer_info(fields, &job)).len() as u32,
            SLICER_INFO_BYTES
        );
    }

    #[test]
    fn a_layer_row_and_its_motion_block_are_the_widths_the_table_steps_by() {
        let job = sample_job(3);
        let row = LayerRow {
            z_mm: 0.05,
            exposure_s: 2.0,
            light_off_delay_s: 1.0,
            address: 0,
            size: 0,
            area: 0,
        };
        assert_eq!(
            written(|fields| row.write(fields)).len() as u64,
            LAYER_ROW_BYTES
        );
        assert_eq!(
            written(|fields| write_layer_block(fields, &job, 0)).len() as u32,
            LAYER_BLOCK_BYTES
        );
    }

    #[test]
    fn a_layer_of_the_bottom_block_carries_its_own_motion() {
        let mut job = sample_job(10);
        job.material.bottom_layers = 2;
        job.material.bottom_lift_distance_mm = 7.0;
        job.material.lift_distance_mm = 6.0;
        let lift = |index| {
            let bytes = written(|fields| write_layer_block(fields, &job, index));
            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        };
        assert!(
            (lift(0) - 7.0).abs() < 1e-6,
            "a bottom layer lifts {}",
            lift(0)
        );
        assert!(
            (lift(5) - 6.0).abs() < 1e-6,
            "a normal layer lifts {}",
            lift(5)
        );
    }
}
