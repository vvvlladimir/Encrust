use core_format::{
    FormatError, LayerEntry, OpenFile, PrinterTuning, ReadSeek, Reads, SlicedFile,
    SlicedFileReader, layer_in_range, panel_in_range, stated_distance_mm, stated_speed_mm_min,
    stated_wait_s,
};
use core_raster::Run;

use crate::blocks::MACHINE_NAME_BYTES;
use crate::tables::{LAYER_DEF_BYTES, LAYER_TABLE_HEAD_BYTES, TABLE_BASE_BYTES};
use crate::writer::AnycubicVersion;
use crate::{bit_planes, rle};

/// The mark at the front of the file, without the nul padding that fills its field.
const FILE_MARK: &[u8] = b"ANYCUBIC";

/// Where the mark states each table is, by its own offset. The machine address is there
/// when the mark claims eight tables or more; see `docs/formats/anycubic.md`.
const HEADER_ADDRESS: u64 = 0x14;
const LAYER_TABLE_ADDRESS: u64 = 0x24;
const MACHINE_ADDRESS: u64 = 0x2C;

/// Where the file mark states how many tables follow.
const TABLE_COUNT_ADDRESS: u64 = 0x10;

/// The fewest tables a file carrying the machine block states.
const MACHINE_TABLE_COUNT: u32 = 8;

/// Grey the four-bit runs decode to: sixteen steps of seventeen.
const GREY_STEPS: u16 = 16;

/// The revisions this reader knows, which are the ones the writer produces.
const READ_VERSIONS: [AnycubicVersion; 5] = [
    AnycubicVersion::V1,
    AnycubicVersion::V515,
    AnycubicVersion::V516,
    AnycubicVersion::V517,
    AnycubicVersion::V518,
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
        open_as(source, Codec::Nibbles)
    }
}

/// Reads the `.pws`: the same container, whose layers are one-bit passes summed into grey.
#[derive(Debug, Default, Clone, Copy)]
pub struct PwsReader;

impl SlicedFileReader for PwsReader {
    type Open<S: ReadSeek> = OpenAnycubic<S>;

    fn extension(&self) -> &'static str {
        "pws"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        open_as(source, Codec::BitPlanes { levels: 0 })
    }
}

/// How a layer's bytes encode its pixels.
#[derive(Debug, Clone, Copy)]
enum Codec {
    /// Runs of a four-bit grey.
    Nibbles,
    /// One one-bit pass per anti-alias level, which the header states.
    BitPlanes { levels: u32 },
}

impl Codec {
    /// This codec as the header's anti-alias level makes it.
    fn with_levels(self, levels: u32) -> Result<Self, FormatError> {
        match self {
            Self::Nibbles => Ok(self),
            Self::BitPlanes { .. } if (1..=bit_planes::MAX_LEVELS).contains(&levels) => {
                Ok(Self::BitPlanes { levels })
            }
            Self::BitPlanes { .. } => Err(FormatError::AntiAliasLevels {
                levels,
                max: bit_planes::MAX_LEVELS,
            }),
        }
    }

    fn format(self) -> &'static str {
        match self {
            Self::Nibbles => "pwmx",
            Self::BitPlanes { .. } => "pws",
        }
    }

    fn grey_steps(self) -> u16 {
        match self {
            Self::Nibbles => GREY_STEPS,
            Self::BitPlanes { levels } => u16::try_from(levels + 1).unwrap_or(u16::MAX),
        }
    }
}

fn open_as<S: ReadSeek>(source: S, codec: Codec) -> Result<OpenAnycubic<S>, FormatError> {
    let mut reads = Reads::new(source);

    let mark = reads.array::<8>()?;
    if mark != FILE_MARK {
        reads.seek_to(0)?;
        return Err(FormatError::NotThisFormat {
            format: codec.format(),
            found: reads.u32_le()?,
        });
    }
    reads.seek_to(0x0C)?;
    let version = reads.u32_le()?;
    // Every revision puts the header's own address in the same slot and its first
    // twenty fields in the same order, so one pass reads them all.
    if !READ_VERSIONS.iter().any(|known| known.number() == version) {
        return Err(FormatError::UnsupportedVersion {
            format: codec.format(),
            version,
        });
    }

    reads.seek_to(HEADER_ADDRESS)?;
    let header = u64::from(reads.u32_le()?);
    reads.seek_to(LAYER_TABLE_ADDRESS)?;
    let table = u64::from(reads.u32_le()?);

    // The fields begin behind the table's own name and stated length.
    reads.seek_to(header + LAYER_TABLE_HEAD_BYTES - 4)?;
    let pitch_um = reads.f32_le()?;
    let layer_height_mm = reads.f32_le()?;
    let exposure_s = reads.f32_le()?;
    let light_off_delay_s = stated_wait_s(reads.f32_le()?);
    let bottom_exposure_s = reads.f32_le()?;
    let bottom_layers = reads.f32_le()? as u32;
    let lift_mm = stated_distance_mm(reads.f32_le()?);
    // Both speeds are millimetres a second here, not a minute.
    let lift_speed_mm_min = stated_speed_mm_min(reads.f32_le()? * 60.0);
    let retract_speed_mm_min = stated_speed_mm_min(reads.f32_le()? * 60.0);
    let volume_ml = reads.f32_le()?;
    let codec = codec.with_levels(reads.u32_le()?)?;
    let width_px = reads.u32_le()?;
    let height_px = reads.u32_le()?;
    panel_in_range(width_px, height_px)?;

    // Weight, price, the currency and the per-layer flag sit between the panel and the
    // print time; see docs/formats/anycubic.md.
    reads.skip(16)?;
    let print_time_s = reads.u32_le()?;
    let stated = PrinterTuning {
        light_off_delay_s,
        lift_distance_mm: lift_mm,
        retract_distance_mm: lift_mm,
        lift_speed_mm_min,
        retract_speed_mm_min,
        transition_layers: u16::try_from(reads.u32_le()?).ok(),
        ..PrinterTuning::default()
    };

    reads.seek_to(table + LAYER_TABLE_HEAD_BYTES - 4)?;
    let layer_count = reads.u32_le()?;
    let layers = read_table(&mut reads, table, layer_count)?;
    let machine = read_machine_name(&mut reads)?;

    let display_mm = (
        width_px as f32 * pitch_um / 1000.0,
        height_px as f32 * pitch_um / 1000.0,
    );
    Ok(OpenAnycubic {
        reads,
        codec,
        facts: SlicedFile {
            format: codec.format(),
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
            grey_steps: codec.grey_steps(),
            stated,
            layers,
        },
    })
}

/// The name the machine block carries, which is what the firmware matches its own against.
///
/// The header states no machine at all, and Photon Workshop's files below 516 carry no
/// such block, so such a file names none.
fn read_machine_name<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Option<String>, FormatError> {
    // Photon Workshop's own files carry the machine block from eight tables on, and ours
    // carry it at every revision; the table count says which, the revision does not.
    reads.seek_to(TABLE_COUNT_ADDRESS)?;
    if reads.u32_le()? < MACHINE_TABLE_COUNT {
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
    codec: Codec,
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
        match self.codec {
            Codec::Nibbles => rle::decode(&data),
            Codec::BitPlanes { levels } => {
                let panel_px = u64::from(self.facts.width_px) * u64::from(self.facts.height_px);
                bit_planes::decode(&data, panel_px, levels)
            }
        }
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
        let mut bytes = vec![0u8; crate::tables::FILE_MARK_BYTES as usize];
        bytes[..8].copy_from_slice(FILE_MARK);
        bytes[0x0C..0x10].copy_from_slice(&514u32.to_le_bytes());

        let mut source = Cursor::new(bytes);
        let err = AnycubicReader
            .open(&mut source)
            .err()
            .expect("514 is a revision nothing here writes");
        assert!(matches!(
            err,
            FormatError::UnsupportedVersion {
                format: "pwmx",
                version: 514
            }
        ));
    }
}
