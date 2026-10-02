use std::collections::BTreeMap;

use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, SlicedFile, SlicedFileReader, decode_grey,
    layer_in_range, png_shape, read_entry,
};
use core_raster::Run;
use zip::ZipArchive;

use crate::writer::{Sl1Flavour, index_digits};

/// Reads the Prusa `.sl1` archive. Layout is in `docs/formats/sl1.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sl1Reader;

impl SlicedFileReader for Sl1Reader {
    type Open<S: ReadSeek> = OpenSl1<S>;

    fn extension(&self) -> &'static str {
        "sl1"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut zip = ZipArchive::new(source).map_err(archive)?;

        let settings = read_settings(&mut zip)?;
        let mut layers = layer_names(&zip);
        let number = |key: &str| {
            settings
                .get(key)
                .and_then(|value| value.parse::<f32>().ok())
        };
        let text = |key: &str| settings.get(key).cloned().filter(|value| !value.is_empty());

        // The archive states no resolution; the layers are the only thing that does.
        let (width_px, height_px) = match layers.first() {
            Some((_, name)) => png_shape(&entry_bytes(&mut zip, name)?)?,
            None => {
                return Err(FormatError::Missing {
                    what: "any layer image".to_owned(),
                });
            }
        };

        let layer_height_mm = number("layerHeight").unwrap_or_default();
        let exposure_s = number("expTime").unwrap_or_default();
        let entries = layers
            .iter()
            .map(|(index, _)| LayerEntry {
                z_mm: layer_height_mm * (index + 1) as f32,
                exposure_s,
                offset: 0,
                size: 0,
            })
            .collect();

        layers.sort_by_key(|(index, _)| *index);
        Ok(OpenSl1 {
            zip,
            names: layers.into_iter().map(|(_, name)| name).collect(),
            facts: SlicedFile {
                // The machine is what tells the two apart: `sla_archive_format` reads
                // `SL1` in a real `.sl1s` as well.
                format: match text("printerModel").as_deref() {
                    Some("SL1S") => Sl1Flavour::Sl1s.extension(),
                    _ => Sl1Flavour::Sl1.extension(),
                },
                version: None,
                machine: text("printerModel"),
                slicer: text("prusaSlicerVersion"),
                resin: text("materialName"),
                width_px,
                height_px,
                display_mm: number("display_width").zip(number("display_height")),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s: number("expTimeFirst").unwrap_or_default(),
                bottom_layers: number("numFade").unwrap_or_default() as u32,
                print_time_s: number("printTime").map(|seconds| seconds as u32),
                volume_mm3: number("usedMaterial").map(|ml| ml * 1000.0),
                grey_steps: 256,
                layers: entries,
            },
        })
    }
}

/// Both settings files as one table of keys, which is what a reader needs of them: the two
/// do not share a key, and a reader looks each one up by name.
fn read_settings<S: ReadSeek>(
    zip: &mut ZipArchive<S>,
) -> Result<BTreeMap<String, String>, FormatError> {
    let mut settings = BTreeMap::new();

    for name in ["config.ini", "prusaslicer.ini"] {
        let text = String::from_utf8_lossy(&entry_bytes(zip, name)?).into_owned();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=') {
                settings.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
    Ok(settings)
}

/// The layer entries, found the way a reader finds them: by a name that ends in exactly
/// five digits and `.png`, whatever the prefix. The previews escape it because
/// `thumbnail400x400` does not end in five digits.
fn layer_names<S: ReadSeek>(zip: &ZipArchive<S>) -> Vec<(u32, String)> {
    let mut found: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|name| {
            let stem = name.strip_suffix(".png")?;
            let digits = stem.get(stem.len().checked_sub(index_digits())?..)?;
            digits
                .chars()
                .all(|c| c.is_ascii_digit())
                .then(|| digits.parse().ok())
                .flatten()
                .map(|index| (index, name.to_owned()))
        })
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found
}

fn entry_bytes<S: ReadSeek>(zip: &mut ZipArchive<S>, name: &str) -> Result<Vec<u8>, FormatError> {
    let mut entry = zip.by_name(name).map_err(|_| FormatError::Missing {
        what: name.to_owned(),
    })?;
    read_entry(&mut entry, name)
}

fn archive(source: zip::result::ZipError) -> FormatError {
    match source {
        zip::result::ZipError::Io(source) => FormatError::Io(source),
        other => FormatError::Encoding {
            what: "the archive",
            reason: other.to_string(),
        },
    }
}

/// An `.sl1` with its settings read and its layers still in it.
pub struct OpenSl1<S> {
    zip: ZipArchive<S>,
    /// Entry name of each layer, in print order.
    names: Vec<String>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenSl1<S> {
    fn facts(&self) -> &SlicedFile {
        &self.facts
    }

    fn layer(&mut self, index: u32) -> Result<Vec<Run>, FormatError> {
        layer_in_range(index, self.facts.layer_count())?;
        let name = self.names[index as usize].clone();
        let bytes = entry_bytes(&mut self.zip, &name)?;
        decode_grey(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn something_that_is_not_an_archive_is_refused() {
        let mut source = Cursor::new(b"not a zip at all".to_vec());
        assert!(Sl1Reader.open(&mut source).is_err());
    }

    #[test]
    fn an_archive_without_the_settings_names_what_is_missing() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            zip.start_file("readme.txt", zip::write::SimpleFileOptions::default())
                .expect("in memory");
            zip.finish().expect("in memory");
        }
        let mut source = Cursor::new(bytes.into_inner());
        let err = Sl1Reader
            .open(&mut source)
            .err()
            .expect("a zip of something else is not an .sl1");
        assert!(matches!(err, FormatError::Missing { what } if what == "config.ini"));
    }
}
