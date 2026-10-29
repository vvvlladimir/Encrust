use std::fmt::Write as _;

use md5::{Digest as _, Md5};

use core_format::{
    Fields, FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, check_layer_pushed,
    check_layers_finished, validate,
};
use core_raster::LayerRuns;

use crate::header::{self, DELIMITER, HEADER_SIZE};
use crate::layer::{self, DEFINITION_BYTES, LAYER_MAGIC};
use crate::rle5::EncodedHalves;
use crate::version::GooVersion;

/// Marks of the three tables behind the header: the definitions, the masks, the blocks.
const DEFINITIONS_MARK: u8 = 0xA1;
const MASKS_MARK: u8 = 0xA2;
const BLOCKS_MARK: u8 = 0xA3;

/// The block that ends the layers: a mark, the resin preset's name, a delimiter.
const NAME_MARK: u8 = 0x66;
const NAME_BYTES: usize = 139;
const NAME_BLOCK_BYTES: u32 = 1 + NAME_BYTES as u32 + 2;

/// Bytes of the revision's own header fields behind the shared ones.
const EXTENSION_BYTES: u32 = 15;

/// Writes the `.goo` revision 5.1 the Jupiter 2 reads. Layout in `docs/formats/goo.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Goo5Writer;

impl SlicedFileWriter for Goo5Writer {
    type Sink<'w> = Goo5Sink<'w>;

    fn extension(&self) -> &'static str {
        "goo"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;
        if !job.raster.width_px.is_multiple_of(2) {
            return Err(FormatError::Encoding {
                what: "a .goo 5.1 panel",
                reason: format!("{} px wide has no middle to split at", job.raster.width_px),
            });
        }

        let tables = Tables::of(job.layer_count());
        let mut fields = Fields::new(sink);
        header::write(&mut fields, job, GooVersion::V5)?;
        tables.write(&mut fields)?;
        for index in 0..job.layer_count() {
            layer::write_definition(&mut fields, job, index, GooVersion::V5)?;
        }
        let end = fields.position()?;
        Ok(Goo5Sink {
            fields,
            job: job.clone(),
            tables,
            written: 0,
            end,
        })
    }
}

/// Where the tables behind the header start, all fixed by the layer count.
#[derive(Debug, Clone, Copy)]
struct Tables {
    count: u32,
    definitions: u32,
    masks: u32,
    blocks: u32,
}

impl Tables {
    fn of(count: u32) -> Self {
        let definitions = HEADER_SIZE + EXTENSION_BYTES;
        let masks = definitions + 1 + 8 * count;
        Self {
            count,
            definitions,
            masks,
            blocks: masks + 1 + 16 * count,
        }
    }

    /// The first definition, behind the block table's mark, count and one entry.
    fn first_definition(self) -> u32 {
        self.blocks + 13
    }

    /// The extension and all three tables, the masks' entries and the block's offset
    /// left zero until they are known.
    fn write(self, fields: &mut Fields<'_>) -> std::io::Result<()> {
        fields.u8(2)?;
        for at in [self.definitions, self.masks, self.blocks] {
            fields.u32_le(at)?;
        }
        fields.u16_le(0x0300)?;

        fields.u8(DEFINITIONS_MARK)?;
        for index in 0..self.count {
            fields.u32_le(self.first_definition() + index * DEFINITION_BYTES)?;
            fields.u32_le(DEFINITION_BYTES)?;
        }
        fields.u8(MASKS_MARK)?;
        fields.zeros(16 * self.count as usize)?;
        fields.u8(BLOCKS_MARK)?;
        fields.u32_le(1)?;
        fields.u32_le(0)?;
        fields.u32_le(NAME_BLOCK_BYTES)
    }
}

/// A `.goo` 5.1 with its header, tables and definitions written, waiting for the masks.
pub struct Goo5Sink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    tables: Tables,
    written: u32,
    end: u64,
}

impl Goo5Sink<'_> {
    /// One half's mask at the end of the file: where it went and its length.
    fn write_half(&mut self, data: &[u8], checksum: u8) -> Result<[u32; 2], FormatError> {
        let at = self.end as u32;
        self.fields.u8(LAYER_MAGIC)?;
        self.fields.bytes(data)?;
        self.fields.u8(checksum)?;
        self.fields.bytes(&DELIMITER)?;
        self.end = self.fields.position()?;
        Ok([at, self.end as u32 - at])
    }
}

impl LayerSink for Goo5Sink<'_> {
    type Encoded = EncodedHalves;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedHalves::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        check_layer_pushed(&self.job, self.written, (encoded.width(), encoded.height()))?;

        let [left, right] = encoded.halves();
        let left = self.write_half(&left.data, left.checksum)?;
        let right = self.write_half(&right.data, right.checksum)?;
        self.fields
            .seek_to(u64::from(self.tables.masks + 1 + 16 * self.written))?;
        for value in left.into_iter().chain(right) {
            self.fields.u32_le(value)?;
        }
        self.fields.seek_to(self.end)?;
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        check_layers_finished(&self.job, self.written)?;
        self.fields.u8(NAME_MARK)?;
        self.fields.text(&self.job.material.name, NAME_BYTES)?;
        self.fields.bytes(&DELIMITER)?;
        self.fields.seek_to(u64::from(self.tables.blocks + 5))?;
        self.fields.u32_le(self.end as u32)?;

        // The header is laid down again for the volume, as in version 3.0.
        self.job.volume_mm3 = volume_mm3;
        self.fields.seek_to(0)?;
        header::write(&mut self.fields, &self.job, GooVersion::V5)?;

        // The file closes with the MD5 of every byte before it, in lower-case hex.
        let mut md5 = Md5::new();
        self.fields.read_back(|chunk| md5.update(chunk))?;
        self.fields.bytes(hex_of(&md5.finalize()).as_bytes())?;
        self.fields.flush()?;
        Ok(())
    }
}

/// `digest` as lower-case hex, two characters a byte.
fn hex_of(digest: &[u8]) -> String {
    digest.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

#[cfg(test)]
mod tests;
