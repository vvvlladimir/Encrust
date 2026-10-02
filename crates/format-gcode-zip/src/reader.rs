use std::collections::BTreeMap;

use core_format::{
    FormatError, LayerEntry, OpenFile, ReadSeek, SlicedFile, SlicedFileReader, decode_grey,
    layer_in_range, panel_in_range, png_shape, read_entry,
};
use core_raster::Run;
use zip::ZipArchive;

use crate::gcode::GENERATED_BY;
use crate::writer::PROGRAM;

/// Reads the zip of greyscale PNGs a Chitu board runs as gcode. Layout is in
/// `docs/formats/gcode-zip.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GcodeZipReader;

impl SlicedFileReader for GcodeZipReader {
    type Open<S: ReadSeek> = OpenGcodeZip<S>;

    fn extension(&self) -> &'static str {
        "zip"
    }

    fn open<S: ReadSeek>(&self, source: S) -> Result<Self::Open<S>, FormatError> {
        let mut zip = ZipArchive::new(source).map_err(archive)?;
        let program = read_program(&mut zip)?;
        let (settings, slicer, blocks) = parse(&program);
        let names = layer_names(&zip);

        let number = |key: &str| {
            settings
                .get(key)
                .and_then(|value| value.parse::<f32>().ok())
        };
        let text = |key: &str| settings.get(key).cloned().filter(|value| !value.is_empty());
        let either = |first: &str, second: &str| number(first).or_else(|| number(second));

        let layer_height_mm = number("layerHeight").unwrap_or_default();
        let exposure_s = number("normalExposureTime").unwrap_or_default();

        // The program is what states the panel, where it states it at all; a file whose
        // header is short of it is measured off its first image instead.
        let (width_px, height_px) = match (number("resolutionX"), number("resolutionY")) {
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

        let entries = (0..names.len())
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

        Ok(OpenGcodeZip {
            zip,
            names,
            facts: SlicedFile {
                format: "zip",
                version: None,
                machine: text("machineType"),
                slicer,
                resin: text("resin"),
                width_px,
                height_px,
                display_mm: number("machineX").zip(number("machineY")),
                layer_height_mm,
                exposure_s,
                bottom_exposure_s: either("bottomLayerExposureTime", "bottomLayExposureTime")
                    .unwrap_or_default(),
                bottom_layers: either("bottomLayerCount", "bottomLayCount").unwrap_or_default()
                    as u32,
                print_time_s: number("estimatedPrintTime").map(|seconds| seconds as u32),
                volume_mm3: number("volume").map(|ml| ml * 1000.0),
                grey_steps: 256,
                layers: entries,
            },
        })
    }
}

/// Whether an archive is one of these, which is the presence of the program it runs.
///
/// Both archive containers we read start `PK`, so a file renamed on the way out of another
/// slicer is told apart by what is in it rather than by its first bytes.
pub fn claims<S: ReadSeek>(source: &mut S) -> bool {
    ZipArchive::new(source).is_ok_and(|zip| zip.file_names().any(|name| name == PROGRAM))
}

/// What one `;LAYER_START` block says about its layer.
struct Block {
    z_mm: f32,
    exposure_s: f32,
}

/// The program split into its header keys, what wrote it, and one record per layer block.
fn parse(program: &str) -> (BTreeMap<String, String>, Option<String>, Vec<Block>) {
    let mut settings = BTreeMap::new();
    let mut slicer = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut lit = false;

    for line in program.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix(GENERATED_BY) {
            slicer = Some(name.to_owned());
        } else if line.starts_with(";LAYER_START:") {
            blocks.push(Block {
                z_mm: 0.0,
                exposure_s: 0.0,
            });
        } else if let Some(z) = line.strip_prefix(";PositionZ:") {
            if let (Some(block), Ok(z_mm)) = (
                blocks.last_mut(),
                z.trim_end_matches("mm").trim().parse::<f32>(),
            ) {
                block.z_mm = z_mm;
            }
        } else if let Some((key, value)) = line.strip_prefix(';').and_then(|l| l.split_once(':')) {
            settings.insert(key.trim().to_owned(), value.trim().to_owned());
        } else if let Some(pwm) = command(line, "M106 S") {
            // The light is lit by one `M106` and put out by the next; the dwell between
            // the two is the exposure, wherever in the block it sits.
            lit = pwm.parse::<u16>().is_ok_and(|pwm| pwm > 0);
        } else if let Some(ms) = lit
            .then(|| command(line, "G4 P").and_then(|ms| ms.parse::<f32>().ok()))
            .flatten()
        {
            if let Some(block) = blocks.last_mut() {
                block.exposure_s = ms / 1000.0;
            }
            lit = false;
        }
    }
    (settings, slicer, blocks)
}

/// The argument of a command, with the trailing `;comment` a line may carry taken off.
fn command<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    Some(rest.split(';').next().unwrap_or(rest).trim())
}

fn read_program<S: ReadSeek>(zip: &mut ZipArchive<S>) -> Result<String, FormatError> {
    let bytes = entry_bytes(zip, PROGRAM)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The layer entries, found the way a reader finds them: a name of nothing but digits and
/// `.png`, numbered from one. The two previews escape it because they are named.
fn layer_names<S: ReadSeek>(zip: &ZipArchive<S>) -> Vec<String> {
    let mut found: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|name| {
            let stem = name.strip_suffix(".png")?;
            stem.parse().ok().map(|index| (index, name.to_owned()))
        })
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found.into_iter().map(|(_, name)| name).collect()
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

/// An archive with its program read and its layers still in it.
pub struct OpenGcodeZip<S> {
    zip: ZipArchive<S>,
    /// Entry name of each layer, in print order.
    names: Vec<String>,
    facts: SlicedFile,
}

impl<S: ReadSeek> OpenFile for OpenGcodeZip<S> {
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

    fn archive_of(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            for (name, content) in entries {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .expect("in memory");
                std::io::Write::write_all(&mut zip, content).expect("in memory");
            }
            zip.finish().expect("in memory");
        }
        Cursor::new(bytes.into_inner())
    }

    #[test]
    fn only_an_archive_carrying_the_program_is_claimed() {
        assert!(claims(&mut archive_of(&[(PROGRAM, b";totalLayer:1\n")])));
        assert!(
            !claims(&mut archive_of(&[("config.ini", b"action = print\n")])),
            "an archive of another slicer's settings is not one of these"
        );
        assert!(!claims(&mut Cursor::new(b"not a zip at all".to_vec())));
    }

    #[test]
    fn something_that_is_not_an_archive_is_refused() {
        let mut source = Cursor::new(b"not a zip at all".to_vec());
        assert!(GcodeZipReader.open(&mut source).is_err());
    }

    #[test]
    fn an_archive_without_the_program_names_what_is_missing() {
        let mut source = archive_of(&[("readme.txt", b"")]);
        let err = GcodeZipReader
            .open(&mut source)
            .err()
            .expect("a zip of something else is not one of these");
        assert!(matches!(err, FormatError::Missing { what } if what == "run.gcode"));
    }

    #[test]
    fn a_program_with_no_image_at_all_says_so() {
        let mut source = archive_of(&[(PROGRAM, b";totalLayer:1\n")]);
        let err = GcodeZipReader
            .open(&mut source)
            .err()
            .expect("nothing states the panel");
        assert!(matches!(err, FormatError::Missing { what } if what == "any layer image"));
    }

    #[test]
    fn a_block_states_its_own_z_and_exposure() {
        let program = ";resolutionX:4\n;resolutionY:2\n;layerHeight:0.05\n\
             ;LAYER_START:0\n;PositionZ:0.05mm\nM106 S255;Turn LED ON\nG4 P30000;Cure\n\
             M106 S0;Turn LED OFF\n\
             ;LAYER_START:1\n;PositionZ:0.1mm\nM106 S180;Turn LED ON\nG4 P2500;Cure\n\
             M106 S0;Turn LED OFF\n";
        let (settings, _, blocks) = parse(program);

        assert_eq!(
            settings.get("layerHeight").map(String::as_str),
            Some("0.05")
        );
        assert_eq!(blocks.len(), 2);
        assert!((blocks[0].z_mm - 0.05).abs() < 1e-6);
        assert!((blocks[0].exposure_s - 30.0).abs() < 1e-6);
        assert!((blocks[1].exposure_s - 2.5).abs() < 1e-6);
    }

    #[test]
    fn a_dwell_that_is_not_an_exposure_is_not_read_as_one() {
        let program = ";LAYER_START:0\n;PositionZ:0.05mm\nG4 P4000;Wait before cure\n\
             M106 S255;Turn LED ON\nG4 P2000;Cure\nM106 S0;Turn LED OFF\n\
             G4 P1000;Wait after cure\n";
        let (_, _, blocks) = parse(program);

        assert!(
            (blocks[0].exposure_s - 2.0).abs() < 1e-6,
            "only the dwell between the light going on and off is the exposure"
        );
    }
}
