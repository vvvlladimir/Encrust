use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range,
};
use core_raster::Run;

use crate::blocks::MACHINE_NAME_BYTES;
use crate::rle;
use crate::tables::{LAYER_DEF_BYTES, LAYER_TABLE_HEAD_BYTES, TABLE_BASE_BYTES};
use crate::writer::AnycubicVersion;

/// The mark at the front of the file, without the nul padding that fills its field.
const FILE_MARK: &[u8] = b"ANYCUBIC";

/// Where the mark states each table is, by its own offset. The machine address is there
/// from revision 516 on; see `docs/formats/anycubic.md`.
const HEADER_ADDRESS: u64 = 0x14;
const LAYER_TABLE_ADDRESS: u64 = 0x24;
const MACHINE_ADDRESS: u64 = 0x2C;

/// Grey the four-bit runs decode to: sixteen steps of seventeen.
const GREY_STEPS: u16 = 16;

/// The revisions this reader knows, which are the ones the writer produces.
const READ_VERSIONS: [AnycubicVersion; 3] = [
    AnycubicVersion::V1,
    AnycubicVersion::V516,
    AnycubicVersion::V517,
];

/// Reads the Anycubic containers at the revisions this crate writes. Layout is in
/// `docs/formats/anycubic.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnycubicReader;

impl SlicedFileReader for AnycubicReader {
    type Open<S: ReadSeek> = OpenAnycubic<S>;

    fn extension(&self) -> &'static str {
        "pwmx"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);

        let mark = reads.array::<8>()?;
        if mark != FILE_MARK {
            reads.seek_to(0)?;
            return Err(FormatError::NotThisFormat {
                format: "pwmx",
                found: reads.u32_le()?,
            });
        }
        reads.seek_to(0x0C)?;
        let version = reads.u32_le()?;
        // Every revision puts the header's own address in the same slot and its first
        // twenty fields in the same order, so one pass reads all three.
        let Some(revision) = READ_VERSIONS
            .iter()
            .copied()
            .find(|known| known.number() == version)
        else {
            return Err(FormatError::UnsupportedVersion {
                format: "pwmx",
                version,
            });
        };

        reads.seek_to(HEADER_ADDRESS)?;
        let header = u64::from(reads.u32_le()?);
        reads.seek_to(LAYER_TABLE_ADDRESS)?;
        let table = u64::from(reads.u32_le()?);

        // The fields begin behind the table's own name and stated length.
        reads.seek_to(header + LAYER_TABLE_HEAD_BYTES - 4)?;
        let pitch_um = reads.f32_le()?;
        let layer_height_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let bottom_exposure_s = reads.f32_le()?;
        let bottom_layers = reads.f32_le()? as u32;
        reads.skip(12)?;
        let volume_ml = reads.f32_le()?;
        reads.skip(4)?;
        let width_px = reads.u32_le()?;
        let height_px = reads.u32_le()?;
        panel_in_range(width_px, height_px)?;

        // Weight, price, the currency and the per-layer flag sit between the panel and the
        // print time; see docs/formats/anycubic.md.
        reads.skip(16)?;
        let print_time_s = reads.u32_le()?;

        reads.seek_to(table + LAYER_TABLE_HEAD_BYTES - 4)?;
        let layer_count = reads.u32_le()?;
        let layers = read_table(&mut reads, table, layer_count)?;
        let machine = read_machine_name(&mut reads, revision)?;

        let display_mm = (
            width_px as f32 * pitch_um / 1000.0,
            height_px as f32 * pitch_um / 1000.0,
        );
        Ok(OpenAnycubic {
            reads,
            facts: SlicedFile {
                format: "pwmx",
                version: Some(version),
                machine,
                slicer: None,
                resin: None,
                width_px,
                height_px,
                display_mm: Some(display_mm),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: Some(print_time_s),
                volume_mm3: Some(volume_ml * 1000.0),
                grey_steps: GREY_STEPS,
                layers,
            },
        })
    }
}

/// The name the machine block carries, which is what the firmware matches its own against.
///
/// The header states no machine at all, and revisions below 516 carry no such block, so a
/// file written for an older machine names none.
fn read_machine_name<S: ReadSeek>(
    reads: &mut Reads<S>,
    revision: AnycubicVersion,
) -> Result<Option<String>, FormatError> {
    if !revision.has_machine_block() {
        return Ok(None);
    }
    reads.seek_to(MACHINE_ADDRESS)?;
    let block = u64::from(reads.u32_le()?);
    if block == 0 {
        return Ok(None);
    }
    reads.seek_to(block + u64::from(TABLE_BASE_BYTES))?;
    let name = reads.text(MACHINE_NAME_BYTES)?;
    Ok((!name.is_empty()).then_some(name))
}

/// The layer table, whose rows carry a thickness rather than a height above the plate, so
/// the Z every other container states has to be added up as the rows are read.
fn read_table<S: ReadSeek>(
    reads: &mut Reads<S>,
    table: u64,
    layer_count: u32,
) -> Result<Vec<LayerEntry>, FormatError> {
    let claimed = reads.claim("layer table rows", u64::from(layer_count), LAYER_DEF_BYTES)?;
    let mut layers = Vec::with_capacity(claimed);
    let mut z_mm = 0.0;

    for row in 0..u64::from(layer_count) {
        reads.seek_to(table + LAYER_TABLE_HEAD_BYTES + row * LAYER_DEF_BYTES)?;
        let offset = u64::from(reads.u32_le()?);
        let size = reads.u32_le()?;
        reads.skip(8)?;
        let exposure_s = reads.f32_le()?;
        z_mm += reads.f32_le()?;
        layers.push(LayerEntry {
            z_mm,
            exposure_s,
            offset,
            size,
        });
    }
    Ok(layers)
}

/// An Anycubic file with its tables read and its masks still in it.
pub struct OpenAnycubic<S> {
    reads: Reads<S>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenAnycubic<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        rle::decode(&data)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn a_file_without_the_mark_is_refused() {
        let mut source = Cursor::new(vec![0x11; 64]);
        let err = AnycubicReader
            .open(&mut source)
            .err()
            .expect("the mark does not match");
        assert!(matches!(
            err,
            FormatError::NotThisFormat {
                format: "pwmx",
                found: 0x1111_1111
            }
        ));
    }

    #[test]
    fn a_revision_we_do_not_read_is_named_rather_than_guessed_at() {
        let mut bytes = vec![0u8; AnycubicVersion::V1.file_mark_bytes() as usize];
        bytes[..8].copy_from_slice(FILE_MARK);
        bytes[0x0C..0x10].copy_from_slice(&515u32.to_le_bytes());

        let mut source = Cursor::new(bytes);
        let err = AnycubicReader
            .open(&mut source)
            .err()
            .expect("515 is a revision nothing here writes");
        assert!(matches!(
            err,
            FormatError::UnsupportedVersion {
                format: "pwmx",
                version: 515
            }
        ));
    }
}
