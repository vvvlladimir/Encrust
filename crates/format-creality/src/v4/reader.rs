use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader, decode_rle7,
    layer_in_range, panel_in_range,
};
use core_raster::Run;

use crate::family::MAGIC;

use super::blocks::{LAYER_BLOCK_BYTES, LAYER_ROW_BYTES, VERSION};

/// Reads the Creality `.cxdlp` at version 4. Layout is in `docs/formats/creality.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxdlpV4Reader;

impl SlicedFileReader for CxdlpV4Reader {
    type Open<S: ReadSeek> = OpenCxdlpV4<S>;

    fn extension(&self) -> &'static str {
        "cxdlp"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);
        let end = reads.length()?;

        let magic_bytes = reads.u32_be()?;
        if magic_bytes as usize != MAGIC.len() {
            return Err(FormatError::NotThisFormat {
                format: "cxdlp",
                found: magic_bytes,
            });
        }
        let magic = reads.bytes(MAGIC.len())?;
        if !magic.starts_with(b"CXSW3D") {
            return Err(FormatError::NotThisFormat {
                format: "cxdlp",
                found: u32::from_be_bytes([magic[0], magic[1], magic[2], magic[3]]),
            });
        }

        let version = u32::from(reads.u16_be()?);
        if version != u32::from(VERSION) {
            return Err(FormatError::UnsupportedVersion {
                format: "cxdlp",
                version,
            });
        }

        let machine = read_text(&mut reads)?;
        let width_px = u32::from(reads.u16_le()?);
        let height_px = u32::from(reads.u16_le()?);
        panel_in_range(width_px, height_px)?;
        let display_mm = (reads.f32_le()?, reads.f32_le()?);
        reads.skip(4)?;
        reads.skip(4)?;
        let layer_height_mm = reads.f32_le()?;
        let bottom_layers = reads.u32_le()?;
        reads.skip(4)?;
        let table = u64::from(reads.u32_le()?);
        let layer_count = reads.u32_le()?;
        reads.skip(4)?;
        let print_time_s = reads.u32_le()?;
        reads.skip(4)?;
        let print_parameters = u64::from(reads.u32_le()?);

        // A layer covered by the vendor's cipher would decode to noise, so a file that
        // names a key is refused rather than read wrongly; see docs/formats/creality.md.
        reads.skip(4 + 4 + 2 + 2)?;
        let key = reads.u32_le()?;
        if key != 0 {
            return Err(FormatError::Encoding {
                what: "the layer data",
                reason: format!("it is encrypted under key {key}"),
            });
        }

        reads.seek_to(print_parameters)?;
        reads.skip(4 * 5)?;
        let volume_ml = reads.f32_le()?;
        reads.skip(4 * 2)?;
        reads.skip(4 * 3)?;
        let exposure_s = reads.f32_le()?;
        let bottom_exposure_s = reads.f32_le()?;

        let layers = read_table(&mut reads, table, layer_count, end)?;

        Ok(OpenCxdlpV4 {
            reads,
            facts: SlicedFile {
                format: "cxdlp",
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
                // The codec carries seven bits of grey, so a layer reads back in 128 steps.
                grey_steps: 128,
                layers,
            },
        })
    }
}

/// The layer table, where a row addresses the motion block the data follows.
fn read_table<S: ReadSeek>(
    reads: &mut Reads<S>,
    table: u64,
    layer_count: u32,
    end: u64,
) -> Result<Vec<LayerEntry>, FormatError> {
    let claimed = reads.claim("layer table rows", u64::from(layer_count), LAYER_ROW_BYTES)?;
    let mut layers = Vec::with_capacity(claimed);
    for index in 0..layer_count {
        reads.seek_to(table + u64::from(index) * LAYER_ROW_BYTES)?;
        let z_mm = reads.f32_le()?;
        let exposure_s = reads.f32_le()?;
        reads.skip(4)?;
        let address = u64::from(reads.u32_le()?);
        let size = reads.u32_le()?;

        if size < LAYER_BLOCK_BYTES {
            return Err(FormatError::Missing {
                what: format!("layer {index}'s motion block"),
            });
        }
        let offset = address + u64::from(LAYER_BLOCK_BYTES);
        let size = size - LAYER_BLOCK_BYTES;
        if offset + u64::from(size) > end {
            return Err(FormatError::AddressPastEnd { offset, end });
        }
        layers.push(LayerEntry {
            z_mm,
            exposure_s,
            offset,
            size,
        });
    }
    Ok(layers)
}

/// A length-prefixed, nul-terminated string, counted big-endian as in version 3.
fn read_text<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Option<String>, FormatError> {
    let length = reads.u32_be()?;
    if length == 0 {
        return Ok(None);
    }
    let bytes = reads.bytes(length as usize)?;
    let text = String::from_utf8_lossy(&bytes[..bytes.len() - 1]).into_owned();
    Ok(Some(text).filter(|text| !text.is_empty()))
}

/// A version 4 `.cxdlp` with its tables read and its layers still in it.
pub struct OpenCxdlpV4<S> {
    reads: Reads<S>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenCxdlpV4<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        decode_rle7(&data)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn something_that_is_not_one_of_these_is_refused() {
        let mut source = Cursor::new(vec![0x42; 256]);
        let err = CxdlpV4Reader
            .open(&mut source)
            .err()
            .expect("nothing but a cxdlp is read as one");
        assert!(matches!(
            err,
            FormatError::NotThisFormat {
                format: "cxdlp",
                ..
            }
        ));
    }

    #[test]
    fn version_three_is_not_this_readers_file() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(MAGIC.len() as u32).to_be_bytes());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&3u16.to_be_bytes());
        let mut source = Cursor::new(bytes);
        let err = CxdlpV4Reader
            .open(&mut source)
            .err()
            .expect("version 3 is read by the other reader");
        assert!(matches!(
            err,
            FormatError::UnsupportedVersion {
                format: "cxdlp",
                version: 3
            }
        ));
    }
}
