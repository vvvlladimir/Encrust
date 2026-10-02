use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range,
};
use core_raster::Run;

use crate::crypt::layer_crypt;
use crate::layer::LAYER_DEF_BYTES;
use crate::rle1;

/// The magics of the family, and what each says the layers are coded as.
const MAGIC_CBDDLP: u32 = 0x12FD_0019;
const MAGIC_CTB: u32 = 0x12FD_0086;
const MAGIC_CTB_V4: u32 = 0x12FD_0106;

/// Fields of the header, by their offset. The whole list is in `docs/formats/chitu.md`.
const LAYER_HEIGHT: u64 = 0x20;
const RESOLUTION: u64 = 0x34;
const LAYER_TABLE: u64 = 0x40;
const PRINT_TIME: u64 = 0x4C;
const PRINT_PARAMETERS: u64 = 0x54;
const GREY_PASSES: u64 = 0x5C;
const ENCRYPTION_KEY: u64 = 0x64;
const SLICER_INFO: u64 = 0x68;

/// Which container a magic names, which is what decides the layer codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// `.cbddlp` and `.photon`: one bit a pixel, one pass per grey step.
    Cbddlp,
    /// `.ctb`: seven bits a pixel in one pass.
    Ctb,
}

/// Reads the Chitu container family. Layout is in `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChituReader;

impl SlicedFileReader for ChituReader {
    type Open<S: ReadSeek> = OpenChitu<S>;

    fn extension(&self) -> &'static str {
        "ctb"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);

        let magic = reads.u32_le()?;
        let family = match magic {
            MAGIC_CBDDLP => Family::Cbddlp,
            MAGIC_CTB | MAGIC_CTB_V4 => Family::Ctb,
            found => {
                return Err(FormatError::NotThisFormat {
                    format: "ctb",
                    found,
                });
            }
        };
        let version = reads.u32_le()?;

        reads.seek_to(LAYER_HEIGHT)?;
        let layer_height_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        let bottom_exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let bottom_layers = reads.u32_le()?;

        reads.seek_to(RESOLUTION)?;
        let width_px = reads.u32_le()?;
        let height_px = reads.u32_le()?;
        let pixel_count = panel_in_range(width_px, height_px)?;

        reads.seek_to(LAYER_TABLE)?;
        let table = u64::from(reads.u32_le()?);
        let layer_count = reads.u32_le()?;

        reads.seek_to(PRINT_TIME)?;
        let print_time_s = reads.u32_le()?;

        reads.seek_to(GREY_PASSES)?;
        let passes = reads.u32_le()?.max(1);

        // Other writers cover a `.ctb`'s runs with a cipher keyed here, so a
        // reader that ignored it would decode noise; see docs/formats/chitu.md.
        reads.seek_to(ENCRYPTION_KEY)?;
        let key = reads.u32_le()?;

        let volume_mm3 = read_volume(&mut reads)?;
        let machine = read_machine_name(&mut reads, family)?;

        // A `.cbddlp` holds one row per pass per layer, laid out pass-major, so the table
        // is that many times longer than the stack; see docs/formats/chitu.md.
        let passes = match family {
            Family::Cbddlp => passes,
            Family::Ctb => 1,
        };
        if passes > rle1::GREY_PASSES {
            return Err(FormatError::Encoding {
                what: "the layer passes",
                reason: format!(
                    "the header states {passes}, more than the {} the codec carries",
                    rle1::GREY_PASSES
                ),
            });
        }
        let rows = read_table(&mut reads, table, layer_count, passes)?;

        Ok(OpenChitu {
            reads,
            facts: SlicedFile {
                format: match family {
                    Family::Cbddlp => "cbddlp",
                    Family::Ctb => "ctb",
                },
                version: Some(version),
                machine,
                slicer: None,
                resin: None,
                width_px,
                height_px,
                display_mm: None,
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: Some(print_time_s),
                volume_mm3: Some(volume_mm3),
                grey_steps: match family {
                    Family::Cbddlp => (passes + 1) as u16,
                    Family::Ctb => 128,
                },
                layers: rows.iter().take(layer_count as usize).copied().collect(),
            },
            family,
            passes,
            key,
            rows,
            pixel_count,
        })
    }
}

/// The resin volume, which the print parameters block states in millilitres.
fn read_volume<S: ReadSeek>(reads: &mut Reads<S>) -> Result<f32, FormatError> {
    reads.seek_to(PRINT_PARAMETERS)?;
    let at = u64::from(reads.u32_le()?);
    if at == 0 {
        return Ok(0.0);
    }
    reads.seek_to(at + 0x14)?;
    Ok(reads.f32_le()? * 1000.0)
}

/// The machine name, which only the `.ctb` slicer info block carries and which it addresses
/// separately from itself.
fn read_machine_name<S: ReadSeek>(
    reads: &mut Reads<S>,
    family: Family,
) -> Result<Option<String>, FormatError> {
    if family != Family::Ctb {
        return Ok(None);
    }
    reads.seek_to(SLICER_INFO)?;
    let at = u64::from(reads.u32_le()?);
    if at == 0 {
        return Ok(None);
    }
    reads.seek_to(at + 0x1C)?;
    let name_at = u64::from(reads.u32_le()?);
    let length = reads.u32_le()? as usize;
    if name_at == 0 || length == 0 {
        return Ok(None);
    }
    reads.seek_to(name_at)?;
    Ok(Some(reads.text(length)?))
}

/// Every row of the layer table, which is how a reader reaches a layer at all.
fn read_table<S: ReadSeek>(
    reads: &mut Reads<S>,
    table: u64,
    layer_count: u32,
    passes: u32,
) -> Result<Vec<LayerEntry>, FormatError> {
    let rows = u64::from(layer_count) * u64::from(passes);
    let mut entries = Vec::with_capacity(reads.claim("layer table rows", rows, LAYER_DEF_BYTES)?);

    for row in 0..rows {
        reads.seek_to(table + row * LAYER_DEF_BYTES)?;
        let z_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let offset = u64::from(reads.u32_le()?);
        let size = reads.u32_le()?;
        entries.push(LayerEntry {
            z_mm,
            exposure_s,
            offset,
            size,
        });
    }
    Ok(entries)
}

/// A Chitu file with its table read and its masks still in it.
pub struct OpenChitu<S> {
    reads: Reads<S>,
    facts: SlicedFile,
    family: Family,
    passes: u32,
    /// Key the layer runs are covered with, or zero when they are in the clear.
    key: u32,
    /// Every row, which for a `.cbddlp` is more than one per layer.
    rows: Vec<LayerEntry>,
    pixel_count: u32,
}

impl<S: ReadSeek> OpenFile for OpenChitu<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        let layer_count = self.facts.layer_count();
        layer_in_range(index, layer_count)?;

        match self.family {
            Family::Ctb => {
                let entry = self.rows[index as usize];
                self.reads.seek_to(entry.offset)?;
                let mut data = self.reads.bytes(entry.size as usize)?;
                layer_crypt(self.key, index, &mut data);
                core_format::decode_rle7(&data)
            }
            Family::Cbddlp => {
                // The rows run pass-major, so a layer's passes are a stride apart.
                let mut passes = Vec::with_capacity(self.passes as usize);
                for pass in 0..self.passes as usize {
                    let entry = self.rows[pass * layer_count as usize + index as usize];
                    self.reads.seek_to(entry.offset)?;
                    passes.push(self.reads.bytes(entry.size as usize)?);
                }
                rle1::decode_passes(&passes, self.pixel_count)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn a_file_of_another_family_is_refused_by_its_magic() {
        let mut source = Cursor::new(0x1234_5678u32.to_le_bytes().to_vec());
        let err = ChituReader
            .open(&mut source)
            .err()
            .expect("the magic does not match");
        assert!(matches!(
            err,
            FormatError::NotThisFormat {
                format: "ctb",
                found: 0x1234_5678
            }
        ));
    }

    #[test]
    fn a_header_that_stops_short_is_an_error_rather_than_a_guess() {
        let mut source = Cursor::new(MAGIC_CTB_V4.to_le_bytes().to_vec());
        assert!(ChituReader.open(&mut source).is_err());
    }

    #[test]
    fn the_layer_table_is_not_past_the_end_of_the_file() {
        let mut header = vec![0u8; crate::HEADER_BYTES as usize];
        header[..4].copy_from_slice(&MAGIC_CTB_V4.to_le_bytes());
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        // A table address well past a file this size, and one layer to look for in it.
        header[0x40..0x44].copy_from_slice(&900_000u32.to_le_bytes());
        header[0x44..0x48].copy_from_slice(&1u32.to_le_bytes());

        let mut source = Cursor::new(header);
        let err = ChituReader
            .open(&mut source)
            .err()
            .expect("the table is not in the file");
        assert!(matches!(err, FormatError::AddressPastEnd { .. }));
    }

    #[test]
    fn more_table_rows_than_the_file_has_bytes_is_refused_before_they_are_reserved() {
        let mut header = vec![0u8; crate::HEADER_BYTES as usize];
        header[..4].copy_from_slice(&MAGIC_CTB_V4.to_le_bytes());
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        header[0x34..0x38].copy_from_slice(&16u32.to_le_bytes());
        header[0x38..0x3C].copy_from_slice(&8u32.to_le_bytes());
        // A table inside the file, and four billion rows to read out of it.
        header[0x40..0x44].copy_from_slice(&100u32.to_le_bytes());
        header[0x44..0x48].copy_from_slice(&u32::MAX.to_le_bytes());

        let mut source = Cursor::new(header);
        let err = ChituReader
            .open(&mut source)
            .err()
            .expect("a header-sized file holds no such table");
        assert!(matches!(err, FormatError::ImpossibleCount { .. }), "{err}");
    }
}
