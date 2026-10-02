use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, SlicedFile, SlicedFileReader, decode_grey,
    layer_in_range, panel_in_range, png_shape, read_entry,
};
use core_raster::Run;
use zip::ZipArchive;

use crate::conf::{self, CONF};
use crate::gcode;

/// Reads the `.cws` archive. Layout is in `docs/formats/cws.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CwsReader;

impl SlicedFileReader for CwsReader {
    type Open<S: ReadSeek> = OpenCws<S>;

    fn extension(&self) -> &'static str {
        "cws"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut zip = ZipArchive::new(source).map_err(archive)?;
        let conf_text = String::from_utf8_lossy(&entry_bytes(&mut zip, CONF)?).into_owned();
        let settings = conf::parse(&conf_text);
        let (header, blocks) = match program_name(&zip) {
            Some(name) => {
                let program = String::from_utf8_lossy(&entry_bytes(&mut zip, &name)?).into_owned();
                gcode::parse(&program)
            }
            None => (Vec::new(), Vec::new()),
        };
        let names = layer_names(&zip);

        let number = |key: &str| {
            settings
                .get(key)
                .and_then(|value| value.parse::<f32>().ok())
        };
        let comment = |key: &str| {
            header
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
                .filter(|value| !value.is_empty())
        };

        let layer_height_mm = number("thickness").unwrap_or_default();
        let exposure_s = number("layers_expo_ms").unwrap_or_default() / 1000.0;
        let bottom_exposure_s =
            number("head_layers_expo_ms").unwrap_or(exposure_s * 1000.0) / 1000.0;
        let bottom_layers = number("head_layers_num").unwrap_or_default() as u32;

        // The settings state the panel; a file short of them is measured off its first
        // image instead, which is the one thing that cannot be missing.
        let (width_px, height_px) = match (number("xres"), number("yres")) {
            (Some(width), Some(height)) => (width as u32, height as u32),
            _ => match names.first() {
                Some(name) => png_shape(&entry_bytes(&mut zip, name)?)?,
                None => {
                    return Err(FormatError::Missing {
                        what: "any layer image".to_owned(),
                    });
                }
            },
        };
        panel_in_range(width_px, height_px)?;

        let layers = (0..names.len())
            .map(|index| {
                let block = blocks.get(index);
                LayerEntry {
                    z_mm: block.map_or(layer_height_mm * (index + 1) as f32, |block| block.z_mm),
                    exposure_s: block.map_or(exposure_s, |block| block.exposure_s),
                    offset: 0,
                    size: 0,
                }
            })
            .collect();

        let display_mm = number("xppm")
            .zip(number("yppm"))
            .filter(|(x, y)| *x > 0.0 && *y > 0.0)
            .map(|(x, y)| (width_px as f32 / x, height_px as f32 / y));

        Ok(OpenCws {
            zip,
            names,
            facts: SlicedFile {
                format: "cws",
                version: None,
                machine: comment("Machine Type"),
                slicer: conf::slicer(&conf_text).or_else(|| comment("Slicer")),
                resin: comment("Resin"),
                width_px,
                height_px,
                display_mm,
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

/// Whether an archive is one of these, which is the presence of the settings entry.
///
/// Three containers we read start `PK`, so a file renamed on the way out of another slicer
/// is told apart by what is in it rather than by its first bytes.
pub fn claims<S: ReadSeek>(source: &mut S) -> bool {
    ZipArchive::new(source).is_ok_and(|zip| zip.file_names().any(|name| name == CONF))
}

/// The images of the archive, in the order their numbers put them.
///
/// The firmware shows them in that order whatever they are called, so the stem a writer
/// chose does not reach the reader.
fn layer_names<S: ReadSeek>(zip: &ZipArchive<S>) -> Vec<String> {
    let mut names: Vec<String> = zip
        .file_names()
        .filter(|name| name.to_ascii_lowercase().ends_with(".png") && !name.contains('/'))
        .map(str::to_owned)
        .collect();
    names.sort_by_key(|name| natural(name));
    names
}

/// The entry holding the program, where the archive holds one.
fn program_name<S: ReadSeek>(zip: &ZipArchive<S>) -> Option<String> {
    zip.file_names()
        .find(|name| name.to_ascii_lowercase().ends_with(".gcode"))
        .map(str::to_owned)
}

/// A name as its digits order it, so `a9` comes before `a10`.
fn natural(name: &str) -> (usize, String) {
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    (digits.len(), digits)
}

/// A `.cws` with its settings read and its images still in it.
pub struct OpenCws<S> {
    zip: ZipArchive<S>,
    names: Vec<String>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenCws<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let name = self.names[index as usize].clone();
        let png = entry_bytes(&mut self.zip, &name)?;
        decode_grey(&png)
    }
}

fn entry_bytes<S: ReadSeek>(zip: &mut ZipArchive<S>, name: &str) -> Result<Vec<u8>, FormatError> {
    let mut entry = zip.by_name(name).map_err(|source| match source {
        zip::result::ZipError::FileNotFound => FormatError::Missing {
            what: format!("the archive entry {name}"),
        },
        other => archive(other),
    })?;
    read_entry(&mut entry, name)
}

fn archive(source: zip::result::ZipError) -> FormatError {
    match source {
        zip::result::ZipError::Io(source) => FormatError::Io(source),
        _ => FormatError::NotThisFormat {
            format: "cws",
            found: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn something_that_is_not_an_archive_is_refused() {
        let mut source = Cursor::new(vec![0x42; 256]);
        assert!(!claims(&mut source));
        let err = CwsReader
            .open(&mut source)
            .err()
            .expect("nothing but a cws is read as one");
        assert!(matches!(
            err,
            FormatError::NotThisFormat { format: "cws", .. }
        ));
    }

    #[test]
    fn images_are_ordered_by_their_numbers_and_not_by_their_names() {
        let mut names = ["a9.png", "a10.png", "a1.png"];
        names.sort_by_key(|name| natural(name));
        assert_eq!(names, ["a1.png", "a9.png", "a10.png"]);
    }
}
