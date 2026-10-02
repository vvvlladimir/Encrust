use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, Reads, SlicedFile, SlicedFileReader,
    layer_in_range, panel_in_range,
};
use core_raster::Run;

use crate::family::MAGIC;

use super::blocks::{PAGE_BREAK, PREVIEW_SIZES_PX};
use super::lines::{LINE_BYTES, decode};

/// Reads the Creality `.cxdlp`. Layout is in `docs/formats/creality.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxdlpReader;

impl SlicedFileReader for CxdlpReader {
    type Open<S: ReadSeek> = OpenCxdlp<S>;

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
        if !(2..=3).contains(&version) {
            return Err(FormatError::UnsupportedVersion {
                format: "cxdlp",
                version,
            });
        }
        let machine = read_text(&mut reads)?;
        let layer_count = u32::from(reads.u16_be()?);
        let width_px = u32::from(reads.u16_be()?);
        let height_px = u32::from(reads.u16_be()?);
        panel_in_range(width_px, height_px)?;
        reads.skip(64)?;

        // The previews are raw and of a fixed size, so they are stepped over rather than
        // decoded: a reader of ours shows the layers, not the picture somebody rendered.
        for (width, height) in PREVIEW_SIZES_PX {
            reads.skip(u64::from(width * height * 2) + PAGE_BREAK.len() as u64)?;
        }

        let display_mm_x = read_utf16_number(&mut reads)?;
        let display_mm_y = read_utf16_number(&mut reads)?;
        let layer_height_mm = read_utf16_number(&mut reads)?.unwrap_or_default();
        let exposure_s = f32::from(reads.u16_be()?) / 10.0;
        let wait_s = f32::from(reads.u16_be()?);
        let bottom_exposure_s = f32::from(reads.u16_be()?);
        let bottom_layers = u32::from(reads.u16_be()?);
        reads.skip(2 * 5)?;
        let bottom_light_pwm = reads.u16_be()?;
        let light_pwm = reads.u16_be()?;
        let _ = (wait_s, bottom_light_pwm, light_pwm);

        // The area table, then the block version 3 added: a reader takes the resin and the
        // slicer's name out of it and steps over the corrections behind them.
        reads.skip(u64::from(layer_count) * 4 + PAGE_BREAK.len() as u64)?;
        let (slicer, resin) = if version >= 3 {
            let slicer = read_text(&mut reads)?;
            let resin = read_text(&mut reads)?;
            reads.skip(1 + 4 + 4 + 1 + 2 + 1 + 2 + 5 + PAGE_BREAK.len() as u64)?;
            (slicer, resin)
        } else {
            (None, None)
        };

        // There is no table of offsets: a layer states its own length, so finding the last
        // one means walking all of them. Each walk costs eight bytes of a read.
        let mut layers = Vec::with_capacity(layer_count as usize);
        for index in 0..layer_count {
            reads.skip(4)?;
            let lines = reads.u32_be()?;
            let size = u64::from(lines) * LINE_BYTES as u64;
            let offset = reads.position()?;
            if offset + size > end {
                return Err(FormatError::AddressPastEnd { offset, end });
            }
            layers.push(LayerEntry {
                z_mm: layer_height_mm * (index + 1) as f32,
                exposure_s: if index < bottom_layers {
                    bottom_exposure_s
                } else {
                    exposure_s
                },
                offset,
                size: size as u32,
            });
            reads.skip(size + PAGE_BREAK.len() as u64)?;
        }

        Ok(OpenCxdlp {
            reads,
            facts: SlicedFile {
                format: "cxdlp",
                version: Some(version),
                machine,
                slicer,
                resin,
                width_px,
                height_px,
                display_mm: display_mm_x.zip(display_mm_y),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s,
                bottom_layers,
                print_time_s: None,
                volume_mm3: None,
                grey_steps: 256,
                layers,
            },
        })
    }
}

/// A length-prefixed, nul-terminated string, or `None` where it is empty.
fn read_text<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Option<String>, FormatError> {
    let length = reads.u32_be()?;
    if length == 0 {
        return Ok(None);
    }
    let bytes = reads.bytes(length as usize)?;
    let text = String::from_utf8_lossy(&bytes[..bytes.len() - 1]).into_owned();
    Ok(Some(text).filter(|text| !text.is_empty()))
}

/// One of the three settings the container states as UTF-16 text rather than a number.
fn read_utf16_number<S: ReadSeek>(reads: &mut Reads<S>) -> Result<Option<f32>, FormatError> {
    let length = reads.u32_be()?;
    let bytes = reads.bytes(length as usize)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_be_bytes(pair))
        .collect();
    Ok(String::from_utf16_lossy(&units).trim().parse().ok())
}

/// A `.cxdlp` with its tables read and its layers still in it.
pub struct OpenCxdlp<S> {
    reads: Reads<S>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenCxdlp<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let entry = self.facts.layers[index as usize];
        self.reads.seek_to(entry.offset)?;
        let data = self.reads.bytes(entry.size as usize)?;
        decode(&data, self.facts.width_px, self.facts.height_px)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn something_that_is_not_one_of_these_is_refused() {
        let mut source = Cursor::new(vec![0x42; 256]);
        let err = CxdlpReader
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
    fn a_revision_we_do_not_read_names_itself() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(MAGIC.len() as u32).to_be_bytes());
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&9u16.to_be_bytes());
        let mut source = Cursor::new(bytes);
        let err = CxdlpReader
            .open(&mut source)
            .err()
            .expect("a revision we do not read is refused");
        assert!(matches!(
            err,
            FormatError::UnsupportedVersion {
                format: "cxdlp",
                version: 9
            }
        ));
    }
}
