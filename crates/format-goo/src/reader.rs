use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range,
};
use core_raster::Run;

use crate::header::HEADER_SIZE;
use crate::rle;

/// The tag behind the version string, which is what says this is a `.goo` at all.
const MAGIC_TAG: [u8; 8] = [0x07, 0x00, 0x00, 0x00, 0x44, 0x4C, 0x50, 0x00];

/// Fields of the header we read, by their offset from the start of the file. The rest are
/// motion the reader has no place for; the whole list is in `docs/formats/goo.md`.
const SOFTWARE: u64 = 12;
const MACHINE: u64 = 92;
const RESIN: u64 = 156;
const GEOMETRY: u64 = 195_310;
const LAYER_HEIGHT: u64 = 195_332;
const BOTTOM_EXPOSURE: u64 = 195_369;
const TOTALS: u64 = 195_446;

/// Bytes of one layer's definition, up to the data size field behind it.
const DEFINITION_BYTES: u64 = 66;

/// Reads the Elegoo `.goo` container. Layout is in `docs/formats/goo.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GooReader;

impl SlicedFileReader for GooReader {
    type Open<S: ReadSeek> = OpenGoo<S>;

    fn extension(&self) -> &'static str {
        "goo"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut reads = Reads::new(source);

        let version = reads.text(4)?;
        if reads.array::<8>()? != MAGIC_TAG {
            // The tag is eight bytes where every other container has a word, so the first
            // four of it are what a mismatch can report.
            reads.seek_to(4)?;
            return Err(FormatError::NotThisFormat {
                format: "goo",
                found: reads.u32_be()?,
            });
        }

        reads.seek_to(SOFTWARE)?;
        let slicer = format!("{} {}", reads.text(32)?, reads.text(24)?);
        reads.seek_to(MACHINE)?;
        let machine = reads.text(32)?;
        reads.seek_to(RESIN)?;
        let resin = reads.text(32)?;

        reads.seek_to(GEOMETRY)?;
        let layer_count = reads.u32_be()?;
        let width_px = u32::from(reads.u16_be()?);
        let height_px = u32::from(reads.u16_be()?);
        panel_in_range(width_px, height_px)?;
        reads.skip(2)?;
        let display_mm = (reads.f32_be()?, reads.f32_be()?);

        reads.seek_to(LAYER_HEIGHT)?;
        let layer_height_mm = reads.f32_be()?;
        let exposure_s = reads.f32_be()?;
        reads.seek_to(BOTTOM_EXPOSURE)?;
        let bottom_exposure_s = reads.f32_be()?;
        let bottom_layers = reads.u32_be()?;

        reads.seek_to(TOTALS)?;
        let print_time_s = reads.u32_be()?;
        let volume_mm3 = reads.f32_be()?;

        let layers = walk_layers(&mut reads, layer_count)?;
        Ok(OpenGoo {
            reads,
            facts: SlicedFile {
                format: "goo",
                version: version.strip_prefix('V').and_then(parse_version),
                machine: Some(machine),
                slicer: Some(slicer.trim().to_owned()),
                resin: Some(resin),
                width_px,
                height_px,
                display_mm: Some(display_mm),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: Some(print_time_s),
                volume_mm3: Some(volume_mm3),
                grey_steps: 256,
                layers,
            },
        })
    }
}

/// `V3.0` as 30, the number the header's own field would hold.
fn parse_version(text: &str) -> Option<u32> {
    let (major, minor) = text.split_once('.')?;
    Some(major.parse::<u32>().ok()? * 10 + minor.parse::<u32>().ok()?)
}

/// Walks the layers from the front, because the container has no table: each layer's
/// definition states its own size and so says where the next one begins.
fn walk_layers<S: ReadSeek>(
    reads: &mut Reads<S>,
    layer_count: u32,
) -> Result<Vec<LayerEntry>, FormatError> {
    // The shortest a layer can be is its definition, the size field and the two bytes the
    // size counts beside the runs, which is what says a count is impossible.
    let claimed = reads.claim("layers", u64::from(layer_count), DEFINITION_BYTES + 6)?;
    let mut layers = Vec::with_capacity(claimed);
    let mut at = u64::from(HEADER_SIZE);

    for _ in 0..layer_count {
        reads.seek_to(at + 6)?;
        let z_mm = reads.f32_be()?;
        let exposure_s = reads.f32_be()?;

        reads.seek_to(at + DEFINITION_BYTES)?;
        let data_size = reads.u32_be()?;

        // The size counts the magic byte in front of the runs and the checksum behind
        // them, so the runs themselves are two bytes shorter.
        let runs = data_size.checked_sub(2).ok_or(FormatError::MalformedRun {
            offset: (at + DEFINITION_BYTES) as usize,
        })?;
        layers.push(LayerEntry {
            z_mm,
            exposure_s,
            offset: at + DEFINITION_BYTES + 5,
            size: runs,
        });
        at += DEFINITION_BYTES + 4 + u64::from(data_size) + 2;
    }
    Ok(layers)
}

/// A `.goo` with its layer offsets walked and its masks still in it.
pub struct OpenGoo<S> {
    reads: Reads<S>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenGoo<S> {
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
    fn a_file_that_is_not_a_goo_is_refused_by_its_tag() {
        let mut source = Cursor::new(vec![0x11; 64]);
        let err = GooReader
            .open(&mut source)
            .err()
            .expect("the tag does not match");
        assert!(matches!(
            err,
            FormatError::NotThisFormat {
                format: "goo",
                found: 0x1111_1111
            }
        ));
    }

    #[test]
    fn the_version_string_becomes_the_number_the_field_would_hold() {
        assert_eq!(parse_version("3.0"), Some(30));
        assert_eq!(parse_version("3"), None);
    }
}
