use std::io;

use core_format::{Fields, PrintJob, Rgb, rgb565};

use crate::blocks::{
    self, COLOUR_TABLE_BYTES, EXTRA_BYTES, MACHINE_BYTES, MODEL_BYTES, SOFTWARE_BYTES,
    sub_layer_table_bytes,
};
use crate::rle::GREY_STEPS;
use crate::writer::AnycubicVersion;

/// Bytes of a table's name, nul-padded.
pub(crate) const NAME_BYTES: usize = 12;

/// Bytes every named table spends on its own name and length.
pub(crate) const TABLE_BASE_BYTES: u32 = NAME_BYTES as u32 + 4;

/// The mark at the front of every file.
const FILE_MARK: &str = "ANYCUBIC";

/// Bytes of the file mark: the mark, the revision, the table count and eleven addresses.
pub(crate) const FILE_MARK_BYTES: u32 = NAME_BYTES as u32 + 8 + 4 * 11;

/// How many tables the file mark claims, whatever the revision it states.
const TABLE_COUNT: u32 = 11;

/// Fields of the header table, which is what its length states.
const HEADER_FIELD_BYTES: u32 = 96;

/// The one preview the container holds.
pub(crate) const PREVIEW_PX: (u32, u32) = (224, 168);

/// Colour a preview is padded with where the square thumbnail does not reach.
const PREVIEW_BACKGROUND: Rgb = [0, 0, 0];

/// Bytes of one row of the layer table.
pub(crate) const LAYER_DEF_BYTES: u64 = 32;

/// Bytes the layer table spends in front of its rows: its name, its length and the count.
pub(crate) const LAYER_TABLE_HEAD_BYTES: u64 = TABLE_BASE_BYTES as u64 + 4;

/// Where each block ended up. The file mark states all of them up front, so they are
/// computed before anything is written.
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
    pub sub_layers: u32,
    pub layer_data: u32,
}

impl Offsets {
    /// Every block's place, derived from the layer count alone: the preview is a fixed size
    /// and every other block is a fixed width.
    pub(crate) fn of(layer_count: u32) -> Self {
        let header = FILE_MARK_BYTES;
        let preview = header + TABLE_BASE_BYTES + HEADER_FIELD_BYTES;
        let colour = preview + preview_table_bytes();
        let layer_table = colour + COLOUR_TABLE_BYTES;
        let extra = layer_table + TABLE_BASE_BYTES + 4 + LAYER_DEF_BYTES as u32 * layer_count;
        let machine = extra + EXTRA_BYTES;
        let software = machine + MACHINE_BYTES;
        let model = software + SOFTWARE_BYTES;
        let sub_layers = model + MODEL_BYTES;
        Self {
            header,
            preview,
            colour,
            layer_table,
            extra,
            machine,
            software,
            model,
            sub_layers,
            layer_data: sub_layers + sub_layer_table_bytes(layer_count),
        }
    }
}

/// What the preview table states its length as: its shape and its pixels, not its base.
fn preview_stated_bytes() -> u32 {
    12 + PREVIEW_PX.0 * PREVIEW_PX.1 * 2
}

/// The whole preview table.
fn preview_table_bytes() -> u32 {
    TABLE_BASE_BYTES + preview_stated_bytes()
}

/// Writes a table's name and the length it states. Some tables count their own base into
/// that length and some do not, which is why it is the caller's number.
pub(crate) fn write_table_head(fields: &mut Fields<'_>, name: &str, length: u32) -> io::Result<()> {
    fields.text(name, NAME_BYTES)?;
    fields.u32_le(length)
}

/// Writes the mark at the front of the file: what the container is, its revision, and
/// where each of its blocks begins.
pub(crate) fn write_file_mark(
    fields: &mut Fields<'_>,
    offsets: Offsets,
    version: AnycubicVersion,
) -> io::Result<()> {
    fields.text(FILE_MARK, NAME_BYTES)?;
    fields.u32_le(version.number())?;
    fields.u32_le(TABLE_COUNT)?;
    for address in [
        offsets.header,
        offsets.software,
        offsets.preview,
        offsets.colour,
        offsets.layer_table,
        offsets.extra,
        offsets.machine,
        offsets.layer_data,
        offsets.model,
        offsets.sub_layers,
    ] {
        fields.u32_le(address)?;
    }
    // The second preview's address: left out, as in every file of this revision.
    fields.u32_le(0)
}

/// Writes every block in front of the layer table.
pub(crate) fn write_front(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    write_header(fields, job)?;
    write_preview(fields, job)?;
    blocks::write_colour_table(fields)?;
    write_layer_table_head(fields, job.layer_count())
}

/// Writes every block that sits between the layer table and the layer data.
pub(crate) fn write_back(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: AnycubicVersion,
) -> io::Result<()> {
    blocks::write_extra(fields, job)?;
    blocks::write_machine(fields, job, version)?;
    blocks::write_software(fields)?;
    blocks::write_model(fields, job, None)?;
    blocks::write_sub_layer_head(fields, job.layer_count())
}

/// Writes the header table. Layout is in `docs/formats/anycubic.md`.
pub(crate) fn write_header(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (material, raster) = (&job.material, &job.raster);
    write_table_head(fields, "HEADER", HEADER_FIELD_BYTES)?;

    fields.f32_le(header_pixel_um(job))?;
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

    fields.u32_le(u32::from(raster.anti_alias_level(GREY_STEPS)))?;
    fields.u32_le(raster.width_px)?;
    fields.u32_le(raster.height_px)?;
    fields.f32_le(job.weight_g())?;
    fields.f32_le(job.cost().unwrap_or(0.0))?;
    write_currency(fields, &material.details.currency)?;

    // The per-layer flag, left clear even when the rows differ; the rows carry it.
    fields.u32_le(0)?;
    fields.u32_le(job.print_time_s())?;
    fields.u32_le(u32::from(material.transition_layers))?;

    // Transition layer type: zero is the linear ramp, which is what the exposure plan
    // already carries layer by layer.
    fields.u32_le(0)?;
    write_header_tail(fields, job)
}

/// The resin type the format states for every resin.
const RESIN_TYPE: u32 = 10;

/// The pixel the header states: the coarser of the two axes, rounded to a whole micron.
fn header_pixel_um(job: &PrintJob) -> f32 {
    let (pitch_x_mm, pitch_y_mm) = job.printer.display.pixel_pitch_mm();
    (pitch_x_mm * 1000.0)
        .round()
        .max((pitch_y_mm * 1000.0).round())
}

/// The fields behind the transition type: advanced mode, on when the resin peels in two
/// stages; the grey level, which repeats the anti-alias level; the edge blur in pixels; the
/// resin type; and intelligent mode off, so the machine keeps the exposure it is given.
fn write_header_tail(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let raster = &job.raster;
    fields.u32_le(u32::from(job.material.second_stage.is_used()))?;
    fields.u16_le(raster.anti_alias_level(GREY_STEPS))?;
    fields.u16_le(u16::from(raster.stated_blur_px()))?;
    fields.u32_le(RESIN_TYPE)?;
    fields.u32_le(0)
}

/// Writes the currency of the price beside it: its first four bytes of UTF-8,
/// cut at a character and nul-padded. A `$` is `24 00 00 00`.
fn write_currency(fields: &mut Fields<'_>, currency: &str) -> io::Result<()> {
    let mut end = currency.len().min(CURRENCY_BYTES);
    while !currency.is_char_boundary(end) {
        end -= 1;
    }
    fields.text(&currency[..end], CURRENCY_BYTES)
}

const CURRENCY_BYTES: usize = 4;

/// Writes the preview table: the job's thumbnail as raw five-six-five colour.
fn write_preview(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (width, height) = PREVIEW_PX;
    write_table_head(fields, "PREVIEW", preview_stated_bytes())?;
    fields.u32_le(width)?;
    // A word between the two sides that every file states as 42.
    fields.u32_le(42)?;
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
    /// Millimetres a minute here, where the header states a second.
    pub lift_speed_mm_min: f32,
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
        fields.f32_le(self.lift_speed_mm_min)?;
        fields.f32_le(self.exposure_s)?;
        fields.f32_le(self.layer_height_mm)?;
        fields.u32_le(self.lit_pixels)?;
        fields.u32_le(0)
    }

    /// This layer's row of the second layer table, which revision 518 adds.
    pub(crate) fn write_sub_layer(&self, fields: &mut Fields<'_>) -> io::Result<()> {
        fields.u32_le(self.data_address)?;
        fields.u32_le(self.data_size)?;
        fields.u32_le(self.lit_pixels)?;
        fields.zeros(8 * 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_currency_is_its_first_four_bytes_of_utf8() {
        let written = |currency: &str| {
            let mut buffer = std::io::Cursor::new(Vec::new());
            write_currency(&mut Fields::new(&mut buffer), currency).expect("in memory");
            buffer.into_inner()
        };
        assert_eq!(written("$"), [0x24, 0, 0, 0]);
        assert_eq!(written("\u{20AC}"), [0xE2, 0x82, 0xAC, 0]);
        assert_eq!(
            written("CHF\u{20AC}"),
            [b'C', b'H', b'F', 0],
            "a character is never split"
        );
    }

    #[test]
    fn every_block_sits_behind_the_one_before_it() {
        let offsets = Offsets::of(10);
        assert_eq!(offsets.header, 64, "a mark of eleven addresses");
        assert_eq!(offsets.preview, 64 + 16 + 96);
        assert_eq!(offsets.colour, offsets.preview + 16 + 12 + 224 * 168 * 2);
        assert_eq!(offsets.layer_table, offsets.colour + 8);
        assert_eq!(offsets.extra, offsets.layer_table + 20 + 32 * 10);
        assert_eq!(offsets.machine, offsets.extra + 72);
        assert_eq!(offsets.software, offsets.machine + 224);
        assert_eq!(offsets.model, offsets.software + 164);
        assert_eq!(offsets.sub_layers, offsets.model + 44);
        assert_eq!(offsets.layer_data, offsets.sub_layers + 24 + 44 * 10);
    }
}
