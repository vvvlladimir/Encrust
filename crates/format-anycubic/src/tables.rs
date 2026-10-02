use std::io;

use core_format::{Fields, PrintJob, Rgb, rgb565};

use crate::blocks::{
    self, COLOUR_TABLE_BYTES, EXTRA_BYTES, MACHINE_BYTES, MODEL_BYTES, SOFTWARE_BYTES,
};
use crate::writer::{AnycubicFlavour, AnycubicVersion};

/// Bytes of a table's name, nul-padded.
pub(crate) const NAME_BYTES: usize = 12;

/// Bytes every named table spends on its own name and length.
const TABLE_BASE_BYTES: u32 = NAME_BYTES as u32 + 4;

/// The mark at the front of every file.
const FILE_MARK: &str = "ANYCUBIC";

/// The only preview the container holds below version 518.
pub(crate) const PREVIEW_PX: (u32, u32) = (224, 168);

/// Colour a preview is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Bytes of one row of the layer table.
pub(crate) const LAYER_DEF_BYTES: u64 = 32;

/// Bytes the layer table spends in front of its rows: its name, its length and the count.
pub(crate) const LAYER_TABLE_HEAD_BYTES: u64 = TABLE_BASE_BYTES as u64 + 4;

/// Where each block ended up. The file mark states all of them up front, so they are
/// computed before anything is written; a block a revision does not carry is zero, which
/// is how the mark says it is absent.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Offsets {
    pub header: u32,
    pub preview: u32,
    pub colour: u32,
    pub layer_table: u32,
    pub extra: u32,
    pub machine: u32,
    pub software: u32,
    pub model: u32,
    pub layer_data: u32,
}

impl Offsets {
    /// Every block's place, derived from the layer count and the revision alone: the
    /// preview is a fixed size and every other block is a fixed width.
    pub(crate) fn of(layer_count: u32, version: AnycubicVersion) -> Self {
        let header = version.file_mark_bytes();
        let preview = header + TABLE_BASE_BYTES + version.header_field_bytes();
        let after_preview = preview + preview_table_bytes();

        let (colour, layer_table) = if version.has_colour_table() {
            (after_preview, after_preview + COLOUR_TABLE_BYTES)
        } else {
            (0, after_preview)
        };
        let after_layers =
            layer_table + TABLE_BASE_BYTES + 4 + LAYER_DEF_BYTES as u32 * layer_count;

        let mut at = after_layers;
        let (extra, machine) = if version.has_machine_block() {
            let blocks = (at, at + EXTRA_BYTES);
            at += EXTRA_BYTES + MACHINE_BYTES;
            blocks
        } else {
            (0, 0)
        };
        let (software, model) = if version.has_model_block() {
            let blocks = (at, at + SOFTWARE_BYTES);
            at += SOFTWARE_BYTES + MODEL_BYTES;
            blocks
        } else {
            (0, 0)
        };

        Self {
            header,
            preview,
            colour,
            layer_table,
            extra,
            machine,
            software,
            model,
            layer_data: at,
        }
    }
}

/// The whole preview table, which states its own length including the base.
fn preview_table_bytes() -> u32 {
    TABLE_BASE_BYTES + 12 + PREVIEW_PX.0 * PREVIEW_PX.1 * 2
}

/// Writes a table's name and the length it states. Some tables count their own base into
/// that length and some do not, which is why it is the caller's number.
pub(crate) fn write_table_head(fields: &mut Fields<'_>, name: &str, length: u32) -> io::Result<()> {
    fields.text(name, NAME_BYTES)?;
    fields.u32_le(length)
}

/// Writes the mark at the front of the file: what the container is, its revision, and
/// where each of its blocks begins. A revision states only the addresses it has.
pub(crate) fn write_file_mark(
    fields: &mut Fields<'_>,
    offsets: Offsets,
    version: AnycubicVersion,
) -> io::Result<()> {
    fields.text(FILE_MARK, NAME_BYTES)?;
    fields.u32_le(version.number())?;
    fields.u32_le(version.table_count())?;
    fields.u32_le(offsets.header)?;
    fields.u32_le(offsets.software)?;
    fields.u32_le(offsets.preview)?;
    fields.u32_le(offsets.colour)?;
    fields.u32_le(offsets.layer_table)?;
    fields.u32_le(offsets.extra)?;
    if version.has_machine_block() {
        fields.u32_le(offsets.machine)?;
    }
    fields.u32_le(offsets.layer_data)?;
    if version.has_model_block() {
        fields.u32_le(offsets.model)?;
    }
    Ok(())
}

/// Writes every block in front of the layer table.
pub(crate) fn write_front(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: AnycubicVersion,
) -> io::Result<()> {
    write_header(fields, job, version)?;
    write_preview(fields, job)?;
    if version.has_colour_table() {
        blocks::write_colour_table(fields)?;
    }
    write_layer_table_head(fields, job.layer_count())
}

/// Writes every block that sits between the layer table and the layer data.
pub(crate) fn write_back(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    flavour: AnycubicFlavour,
    version: AnycubicVersion,
) -> io::Result<()> {
    if version.has_machine_block() {
        blocks::write_extra(fields, job)?;
        blocks::write_machine(fields, job, flavour, version)?;
    }
    if version.has_model_block() {
        blocks::write_software(fields)?;
        blocks::write_model(fields, job)?;
    }
    Ok(())
}

/// Writes the header table. Layout is in `docs/formats/anycubic.md`.
pub(crate) fn write_header(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: AnycubicVersion,
) -> io::Result<()> {
    let (printer, material, raster) = (&job.printer, &job.material, &job.raster);
    let (pitch_x_mm, _) = printer.display.pixel_pitch_mm();

    write_table_head(fields, "HEADER", version.header_field_bytes())?;

    fields.f32_le(pitch_x_mm * 1000.0)?;
    fields.f32_le(job.nominal_height_mm())?;
    fields.f32_le(job.header_exposure_s())?;
    fields.f32_le(material.light_off_s())?;
    fields.f32_le(material.bottom_exposure_s)?;
    fields.f32_le(material.bottom_layers as f32)?;
    fields.f32_le(material.lift_distance_mm)?;

    // Both speeds are millimetres a second here, not a minute.
    fields.f32_le(material.lift_speed_mm_min / 60.0)?;
    fields.f32_le(material.retract_speed_mm_min / 60.0)?;
    fields.f32_le(job.volume_mm3 / 1000.0)?;

    // One pass: the runs carry four bits of grey of their own, and a level above one asks
    // a machine to read that many one-bit passes it will not find. See ADR 0147.
    fields.u32_le(1)?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(job.cost().unwrap_or(0.0))?;
    write_currency(fields, &material.details.currency)?;
    fields.u32_le(u32::from(job.varies_by_layer()))?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(material.transition_layers))?;

    // Transition layer type: zero is the linear ramp, which is what the exposure plan
    // already carries layer by layer.
    fields.u32_le(0)?;
    write_header_tail(fields, version)
}

/// The fields revisions 516 and 517 append to the header.
fn write_header_tail(fields: &mut Fields<'_>, version: AnycubicVersion) -> io::Result<()> {
    if !version.has_machine_block() {
        return Ok(());
    }

    // Advanced mode is what lets a machine obey the two-stage lift; the slicer plans one
    // stage, so the basic mode is what the motion block beside it describes.
    fields.u32_le(0)?;
    if !version.has_model_block() {
        return Ok(());
    }

    // A grey level, a blur level and a resin type the vendor slicer offers and this one
    // does not: nothing here can state them, so they are left at zero.
    fields.u16_le(0)?;
    fields.u16_le(0)?;
    fields.u32_le(0)
}

/// Writes the currency of the price beside it.
///
/// The four-byte field holds one UTF-16 code unit and then padding, not a string: a `$` is
/// `24 00 00 00`. A resin with no currency leaves it blank.
fn write_currency(fields: &mut Fields<'_>, currency: &str) -> io::Result<()> {
    let unit = currency.encode_utf16().next().unwrap_or(0);
    fields.u16_le(unit)?;
    fields.zeros(2)
}

/// Writes the preview table: the job's thumbnail as raw five-six-five colour.
fn write_preview(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (width, height) = PREVIEW_PX;
    write_table_head(fields, "PREVIEW", preview_table_bytes())?;
    fields.u32_le(width)?;
    fields.text("x", 4)?;
    fields.u32_le(height)?;

    match job.thumbnail.as_ref() {
        Some(thumbnail) => {
            for pixel in thumbnail
                .fitted_to(width, height, PREVIEW_BACKGROUND)
                .pixels()
            {
                fields.u16_le(rgb565(*pixel))?;
            }
            Ok(())
        }
        None => fields.zeros((width * height * 2) as usize),
    }
}

/// Writes the layer table's own name, length and count. The rows are left blank and filled
/// in once their layers have been compressed.
fn write_layer_table_head(fields: &mut Fields<'_>, layer_count: u32) -> io::Result<()> {
    let rows = LAYER_DEF_BYTES as u32 * layer_count;
    write_table_head(fields, "LAYERDEF", 4 + rows)?;
    fields.u32_le(layer_count)
}

/// One row of the layer table, filled in as its layer is written.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayerDef {
    pub data_address: u32,
    pub data_size: u32,
    pub lift_distance_mm: f32,
    pub lift_speed_mm_s: f32,
    pub exposure_s: f32,
    /// This layer's own thickness, not its height above the plate.
    pub layer_height_mm: f32,
    pub lit_pixels: u32,
}

impl LayerDef {
    pub(crate) fn write(&self, fields: &mut Fields<'_>) -> io::Result<()> {
        fields.u32_le(self.data_address)?;
        fields.u32_le(self.data_size)?;
        fields.f32_le(self.lift_distance_mm)?;
        fields.f32_le(self.lift_speed_mm_s)?;
        fields.f32_le(self.exposure_s)?;
        fields.f32_le(self.layer_height_mm)?;
        fields.u32_le(self.lit_pixels)?;
        fields.u32_le(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_of_a_ten_layer_file_sit_where_the_reference_file_puts_them() {
        // A reference `.pwmx` of ten layers, read byte by byte; see
        // docs/formats/anycubic.md.
        let offsets = Offsets::of(10, AnycubicVersion::V1);
        assert_eq!(offsets.header, 0x30);
        assert_eq!(offsets.preview, 0x90);
        assert_eq!(offsets.layer_table, 0x126AC);
        assert_eq!(offsets.layer_data, 0x12800);
        assert_eq!(
            (
                offsets.colour,
                offsets.extra,
                offsets.machine,
                offsets.model
            ),
            (0, 0, 0, 0),
            "version 1 carries none of the later blocks and says so with a zero"
        );
    }

    #[test]
    fn a_later_revision_moves_the_layer_data_behind_the_blocks_it_added() {
        let one = Offsets::of(10, AnycubicVersion::V1);
        let five = Offsets::of(10, AnycubicVersion::V516);
        assert_eq!(five.header, one.header + 4, "one more address in the mark");
        assert_eq!(five.colour, five.layer_table - COLOUR_TABLE_BYTES);
        assert_eq!(five.extra, five.layer_table + 0x14 + 32 * 10);
        assert_eq!(five.machine, five.extra + EXTRA_BYTES);
        assert_eq!(five.layer_data, five.machine + MACHINE_BYTES);

        let seven = Offsets::of(10, AnycubicVersion::V517);
        assert_eq!(seven.software, seven.machine + MACHINE_BYTES);
        assert_eq!(seven.model, seven.software + SOFTWARE_BYTES);
        assert_eq!(seven.layer_data, seven.model + MODEL_BYTES);
    }
}
