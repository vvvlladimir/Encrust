//! The blocks the Photon Workshop container gained after version 1: the grey table, the
//! two-stage motion block, the machine, the slicer, the model's bounding box and the second
//! layer table. Layout is in `docs/formats/anycubic.md`.

use std::io;

use core_format::{Fields, PrintJob};

use crate::rle::GREY_STEPS;
use crate::tables::{NAME_BYTES, write_table_head};
use crate::writer::AnycubicVersion;

/// The grey table: whether the full greyscale is in use and how many levels the nibbles
/// stand for. Unnamed, so it carries no table head.
pub(crate) const COLOUR_TABLE_BYTES: u32 = 4 + 4;

/// Fields of the two-stage motion block, which states a length of its own that does not
/// match them.
const EXTRA_FIELD_BYTES: u32 = 14 * 4;

/// What the block says its length is. The reference writer states this and writes the
/// fields above it, and a reader that believed the number would stop short.
const EXTRA_STATED_BYTES: u32 = 24;

pub(crate) const EXTRA_BYTES: u32 = NAME_BYTES as u32 + 4 + EXTRA_FIELD_BYTES;

/// Bytes of the machine block, which counts its own head in.
pub(crate) const MACHINE_BYTES: u32 = 224;

/// Bytes of the machine's name and of the name of its layer-data codec.
pub(crate) const MACHINE_NAME_BYTES: usize = 96;
const CODEC_NAME_BYTES: usize = 16;

/// Bytes of the slicer block. Unnamed, and it states a length of its own in the middle.
pub(crate) const SOFTWARE_BYTES: u32 = 32 + 4 + 32 + 64 + 32;

/// What the slicer block states its length as: the vendor's slicer states 128.
const SOFTWARE_STATED_BYTES: u32 = 128;

/// Bytes of the model block: a name and no length, then the box and two words.
pub(crate) const MODEL_BYTES: u32 = NAME_BYTES as u32 + 8 * 4;

/// Bytes the second layer table spends in front of its rows: name, length, count, index.
pub(crate) const SUB_LAYER_HEAD_BYTES: u32 = NAME_BYTES as u32 + 4 + 4 + 4;

/// Bytes of one row of the second layer table: where a layer is, how long, how lit, and
/// eight floats nobody has identified.
pub(crate) const SUB_LAYER_ROW_BYTES: u32 = 3 * 4 + 8 * 4;

/// Bytes the second layer table takes in the file, head and rows.
pub(crate) fn sub_layer_table_bytes(layer_count: u32) -> u32 {
    SUB_LAYER_HEAD_BYTES + SUB_LAYER_ROW_BYTES * layer_count
}

/// Writes the grey table: two zero words, as the vendor's slicer writes it. The nibble in
/// the layer data is the grey either way; see ADR 0147.
pub(crate) fn write_colour_table(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.zeros(COLOUR_TABLE_BYTES as usize)
}

/// Writes the two-stage motion block. The slicer plans one stage, so the second climbs
/// nothing at no speed and retracts at the first's; see docs/formats/anycubic.md.
pub(crate) fn write_extra(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    write_table_head(fields, "EXTRA", EXTRA_STATED_BYTES)?;
    write_stages(
        fields,
        material.bottom_lift_distance_mm,
        material.bottom_lift_speed_mm_min / 60.0,
        material.bottom_retract_speed_mm_min / 60.0,
    )?;
    write_stages(
        fields,
        material.lift_distance_mm,
        material.lift_speed_mm_min / 60.0,
        material.retract_speed_mm_min / 60.0,
    )
}

/// Two stages of one lift: a count, then height, speed up and speed down for each.
fn write_stages(
    fields: &mut Fields<'_>,
    height_mm: f32,
    up_mm_s: f32,
    down_mm_s: f32,
) -> io::Result<()> {
    fields.u32_le(2)?;
    for value in [height_mm, up_mm_s, down_mm_s, 0.0, 0.0, down_mm_s] {
        fields.f32_le(value)?;
    }
    Ok(())
}

/// Writes the machine block: what the firmware matches its own name against, and the
/// panel it expects.
pub(crate) fn write_machine(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    version: AnycubicVersion,
) -> io::Result<()> {
    let printer = &job.printer;
    write_table_head(fields, "MACHINE", MACHINE_BYTES)?;

    fields.text(printer.machine_name(), MACHINE_NAME_BYTES)?;
    fields.text("pw0Img", CODEC_NAME_BYTES)?;
    fields.u32_le(u32::from(GREY_STEPS))?;
    fields.u32_le(version.machine_property_fields())?;
    fields.f32_le(printer.display.width_mm)?;
    fields.f32_le(printer.display.height_mm)?;
    fields.f32_le(printer.build_volume.z)?;
    fields.u32_le(version.number())?;

    // The colour the machine paints behind a preview; the reference writer states this one
    // whatever the file holds.
    fields.u32_le(6_506_241)?;
    write_machine_panel(fields, job)
}

/// The machine block's panel fields: the pixel along each axis, how many panels there
/// are, and the panel's resolution.
fn write_machine_panel(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let (pitch_x_mm, pitch_y_mm) = job.printer.display.pixel_pitch_mm();
    fields.f32_le(pitch_x_mm * 1000.0)?;
    fields.f32_le(pitch_y_mm * 1000.0)?;
    fields.zeros(8 * 4)?;
    fields.u32_le(1)?;
    fields.zeros(4)?;
    fields.u16_le(panel_px(job.raster.width_px)?)?;
    fields.u16_le(panel_px(job.raster.height_px)?)?;
    fields.zeros(4 * 4)
}

/// One side of the panel in the sixteen bits the machine block gives it.
fn panel_px(side_px: u32) -> io::Result<u16> {
    u16::try_from(side_px).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a {side_px} px panel side does not fit the machine block's 16-bit field"),
        )
    })
}

/// Writes the second layer table's head and leaves its rows blank; they are filled in once
/// the layers are written, like the first table's.
pub(crate) fn write_sub_layer_head(fields: &mut Fields<'_>, layer_count: u32) -> io::Result<()> {
    // The vendor's slicer states one row's length here, whatever the layer count.
    write_table_head(fields, "SUBIMGS", SUB_LAYER_ROW_BYTES)?;
    fields.u32_le(layer_count)?;
    fields.u32_le(1)?;
    fields.zeros((SUB_LAYER_ROW_BYTES * layer_count) as usize)
}

/// Writes the slicer block: which program wrote the file.
pub(crate) fn write_software(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.text("Encrust", 32)?;
    fields.u32_le(SOFTWARE_STATED_BYTES)?;
    fields.text(env!("CARGO_PKG_VERSION"), 32)?;
    fields.text(std::env::consts::OS, 64)?;

    // The OpenGL profile the vendor's slicer states; nothing here renders through it.
    fields.text("3.3-CoreProfile", 32)
}

/// Writes the model block: the box the print stands in, measured from the panel's centre,
/// from the plate to the top of the stack. The vendor's slicer states the mesh's own box;
/// ours is what the layers light, `None` until they are written.
pub(crate) fn write_model(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    box_mm: Option<[f32; 4]>,
) -> io::Result<()> {
    // The vendor's slicer names this block in mixed case and gives it no length.
    fields.text("Model", NAME_BYTES)?;
    let [left, top, right, bottom] = box_mm.unwrap_or_default();
    for value in [left, top, 0.0, right, bottom, job.height_mm()] {
        fields.f32_le(value)?;
    }

    // TODO(step-B7): whether supports were generated, and how dense; needs `PrintJob` to
    // carry the support settings, as supports reach the writers only inside the mesh.
    fields.u32_le(0)?;
    fields.f32_le(0.0)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;

    fn written(block: impl Fn(&mut Fields<'_>) -> io::Result<()>) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut fields = Fields::new(&mut buffer);
        block(&mut fields).expect("a cursor does not fail");
        buffer.into_inner()
    }

    #[test]
    fn every_block_is_the_length_its_offsets_assume() {
        assert_eq!(
            written(write_colour_table).len(),
            COLOUR_TABLE_BYTES as usize
        );
        assert_eq!(
            written(|fields| write_extra(fields, &sample_job(1))).len(),
            EXTRA_BYTES as usize
        );
        for version in [
            AnycubicVersion::V1,
            AnycubicVersion::V516,
            AnycubicVersion::V518,
        ] {
            assert_eq!(
                written(|fields| write_machine(fields, &sample_job(1), version)).len(),
                MACHINE_BYTES as usize,
                "{version:?}"
            );
        }
        assert_eq!(
            written(|fields| write_sub_layer_head(fields, 3)).len(),
            sub_layer_table_bytes(3) as usize
        );
        assert_eq!(written(write_software).len(), SOFTWARE_BYTES as usize);
        assert_eq!(
            written(|fields| write_model(fields, &sample_job(1), None)).len(),
            MODEL_BYTES as usize
        );
    }

    #[test]
    fn the_motion_block_states_a_length_shorter_than_it_writes() {
        let bytes = written(|fields| write_extra(fields, &sample_job(1)));
        let stated = u32::from_le_bytes([
            bytes[NAME_BYTES],
            bytes[NAME_BYTES + 1],
            bytes[NAME_BYTES + 2],
            bytes[NAME_BYTES + 3],
        ]);
        assert_eq!(stated, EXTRA_STATED_BYTES);
        assert!(
            bytes.len() > stated as usize + NAME_BYTES + 4,
            "the reference writer states 24 and writes 56 fields"
        );
    }

    #[test]
    fn the_grey_table_states_no_levels() {
        assert_eq!(written(write_colour_table), vec![0; 8]);
    }
}
