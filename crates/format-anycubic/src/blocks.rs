//! The blocks the Photon Workshop container gained after version 1: the grey table, the
//! two-stage motion block, the machine, the slicer and the model's bounding box. Layout is
//! in `docs/formats/anycubic.md`.

use std::io;

use core_format::{Fields, PrintJob};

use crate::rle::GREY_STEPS;
use crate::tables::{NAME_BYTES, write_table_head};
use crate::writer::{AnycubicFlavour, AnycubicVersion};

/// The grey table: how many levels the layer data's nibbles stand for, and the eight-bit
/// grey each one lights. Unnamed, so it carries no table head.
pub(crate) const COLOUR_TABLE_BYTES: u32 = 4 + 4 + GREY_STEPS as u32 + 4;

/// Fields of the two-stage motion block, which states a length of its own that does not
/// match them.
const EXTRA_FIELD_BYTES: u32 = 14 * 4;

/// What the block says its length is. The reference writer states this and writes the
/// fields above it, and a reader that believed the number would stop short.
const EXTRA_STATED_BYTES: u32 = 24;

pub(crate) const EXTRA_BYTES: u32 = NAME_BYTES as u32 + 4 + EXTRA_FIELD_BYTES;

/// Bytes of the machine block, which counts its own head in.
pub(crate) const MACHINE_BYTES: u32 = 156;

/// Bytes of the machine's name and of the name of its layer-data codec.
const MACHINE_NAME_BYTES: usize = 96;
const CODEC_NAME_BYTES: usize = 16;

/// Bytes of the slicer block. Unnamed, and it states its own length in the middle.
pub(crate) const SOFTWARE_BYTES: u32 = 32 + 4 + 32 + 64 + 32;

/// Bytes of the model block, which counts its own head in.
pub(crate) const MODEL_BYTES: u32 = NAME_BYTES as u32 + 4 + 8 * 4;

/// Writes the grey table.
///
/// With one anti-aliasing level every entry is full brightness, which is what the
/// reference writer emits; the nibble in the layer data is the grey either way. See
/// ADR 0147.
pub(crate) fn write_colour_table(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.u32_le(0)?;
    fields.u32_le(u32::from(GREY_STEPS))?;
    for step in 0..GREY_STEPS {
        let level = u32::from(step) + 1;
        fields.u8((level * 255).min(u32::from(u8::MAX)) as u8)?;
    }
    fields.u32_le(0)
}

/// Writes the two-stage motion block: one stage, because this is what the slicer plans.
pub(crate) fn write_extra(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let material = &job.material;
    write_table_head(fields, "EXTRA", EXTRA_STATED_BYTES)?;

    fields.u32_le(1)?;
    fields.f32_le(material.bottom_lift_distance_mm)?;
    fields.f32_le(material.bottom_lift_speed_mm_min / 60.0)?;
    fields.f32_le(material.bottom_retract_speed_mm_min / 60.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)?;

    fields.u32_le(1)?;
    fields.f32_le(material.lift_distance_mm)?;
    fields.f32_le(material.lift_speed_mm_min / 60.0)?;
    fields.f32_le(material.retract_speed_mm_min / 60.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(0.0)
}

/// Writes the machine block: what the firmware matches its own name against, and the
/// panel it expects.
pub(crate) fn write_machine(
    fields: &mut Fields<'_>,
    job: &PrintJob,
    flavour: AnycubicFlavour,
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
    fields.u32_le(flavour.newest_version().number())?;

    // The colour the machine paints behind a preview; the reference writer states this one
    // whatever the file holds.
    fields.u32_le(6_506_241)
}

/// Writes the slicer block: which program wrote the file.
pub(crate) fn write_software(fields: &mut Fields<'_>) -> io::Result<()> {
    fields.text("Encrust", 32)?;
    fields.u32_le(SOFTWARE_BYTES)?;
    fields.text(env!("CARGO_PKG_VERSION"), 32)?;
    fields.text(std::env::consts::OS, 64)?;

    // The field holds the slicer's OpenGL version. Nothing here renders through OpenGL.
    fields.text("", 32)
}

/// Writes the model block: the box the print stands in, measured from the panel's centre.
///
/// The stack's own extent in X and Y is not carried this far down the pipeline, so the box
/// is the whole panel, which is an over-estimate the machine only draws with.
pub(crate) fn write_model(fields: &mut Fields<'_>, job: &PrintJob) -> io::Result<()> {
    let display = &job.printer.display;
    write_table_head(fields, "MODEL", MODEL_BYTES)?;

    fields.f32_le(display.width_mm / -2.0)?;
    fields.f32_le(display.height_mm / -2.0)?;
    fields.f32_le(0.0)?;
    fields.f32_le(display.width_mm / 2.0)?;
    fields.f32_le(display.height_mm / 2.0)?;
    fields.f32_le(job.height_mm())?;

    // Whether supports were generated, and how dense. Supports reach this writer as part
    // of the mesh, so nothing here can tell.
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
        assert_eq!(
            written(|fields| write_machine(
                fields,
                &sample_job(1),
                AnycubicFlavour::Pwmx,
                AnycubicVersion::V516
            ))
            .len(),
            MACHINE_BYTES as usize
        );
        assert_eq!(written(write_software).len(), SOFTWARE_BYTES as usize);
        assert_eq!(
            written(|fields| write_model(fields, &sample_job(1))).len(),
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
    fn the_grey_table_lights_every_level_at_one_pass() {
        let bytes = written(write_colour_table);
        assert_eq!(&bytes[8..8 + GREY_STEPS as usize], &[255; 16]);
    }
}
