//! The PWSZ archive the newer Anycubic machines read: settings as JSON, the model's extent
//! and each layer's in two binary entries, and every layer in the PW0 codec of the table
//! container. Layout is in `docs/formats/pwsz.md`.

mod reader;
mod scene;
mod settings;
#[cfg(test)]
mod tests;

use std::io::{Cursor, Write};

use core_format::{FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate};
use core_raster::LayerRuns;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::rle::EncodedLayer;
pub use reader::{OpenPwsz, PwszReader, claims};
use scene::{Outline, SceneRow};

/// Which extension the archive goes under: the one the machine's firmware looks for.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PwszFlavour {
    /// Photon Mono M7 Pro.
    #[default]
    Pwsz,
    /// Photon Mono M7.
    Pm7,
    /// Photon Mono M7 Max.
    Pm7m,
    /// Photon Mono 4 Ultra.
    Pm4u,
    /// Photon P1 Max.
    Pp1m,
}

impl PwszFlavour {
    /// Every extension this writer offers, in the order a front end offers them.
    pub const ALL: [Self; 5] = [Self::Pwsz, Self::Pm7, Self::Pm7m, Self::Pm4u, Self::Pp1m];

    /// What the extension is called where a user would see it named.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pwsz => ".pwsz",
            Self::Pm7 => ".pm7",
            Self::Pm7m => ".pm7m",
            Self::Pm4u => ".pm4u",
            Self::Pp1m => ".pp1m",
        }
    }

    /// The extension itself, without the dot.
    pub fn extension(self) -> &'static str {
        &self.label()[1..]
    }

    /// The flavour `extension` names, whatever its case, or `None` for any other.
    pub fn of_extension(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|flavour| flavour.extension().eq_ignore_ascii_case(extension))
    }
}

/// Writes the PWSZ archive under one machine's extension.
#[derive(Debug, Default, Clone, Copy)]
pub struct PwszWriter {
    flavour: PwszFlavour,
}

impl PwszWriter {
    pub fn new(flavour: PwszFlavour) -> Self {
        Self { flavour }
    }
}

impl SlicedFileWriter for PwszWriter {
    type Sink<'w> = PwszSink<'w>;

    fn extension(&self) -> &'static str {
        self.flavour.extension()
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut zip = ZipWriter::new(sink);
        for (index, png) in settings::previews(job)?.iter().enumerate() {
            entry(
                &mut zip,
                &format!("preview_images/preview_{index}.png"),
                png,
            )?;
        }
        // The vendor's order, as far as a stream allows: what the layers add up to waits for them.
        let ahead = [
            ("software_info.conf", settings::software_info()?),
            ("layers_controller.conf", settings::layers_controller(job)?),
            (
                "anycubic_photon_resins.pwsp",
                settings::machine(job, self.flavour)?,
            ),
        ];
        for (name, bytes) in &ahead {
            entry(&mut zip, name, bytes)?;
        }
        tracing::debug!(layers = job.layer_count(), "pwsz previews written");

        Ok(PwszSink {
            zip,
            job: job.clone(),
            rows: Vec::with_capacity(job.layer_count() as usize),
        })
    }
}

/// The archive with its previews in and its layers going in one entry at a time; what the
/// layers add up to is written once they are all in.
pub struct PwszSink<'w> {
    zip: ZipWriter<&'w mut dyn WriteSeek>,
    job: PrintJob,
    rows: Vec<SceneRow>,
}

/// One layer in the PW0 codec, and the extent the scene entry records for it.
pub struct PwszLayer {
    runs: EncodedLayer,
    outline: Outline,
}

impl LayerSink for PwszSink<'_> {
    type Encoded = PwszLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        PwszLayer {
            runs: EncodedLayer::encode(layer),
            outline: Outline::of(layer),
        }
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.rows.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        let (width, height) = (encoded.runs.width(), encoded.runs.height());
        if width != self.job.raster.width_px || height != self.job.raster.height_px {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width,
                height,
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        entry(&mut self.zip, &layer_name(index), encoded.runs.data())?;
        self.rows
            .push(SceneRow::of(&self.job, index, &encoded.outline));
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.rows.len() as u32;
        if written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written,
            });
        }

        // These entries state what the layers add up to, so they go in behind them, where
        // the vendor puts them in front; a reader looks an entry up by name, not by place.
        let job = &self.job;
        let entries = [
            ("print_info.json", settings::print_info(job)?),
            ("scene.slice", scene::scene_slice(job, &self.rows)?),
            (
                "calc_layer_volumes.data",
                scene::layer_volumes(job, &self.rows)?,
            ),
        ];
        for (name, bytes) in &entries {
            entry(&mut self.zip, name, bytes)?;
        }

        let sink = self.zip.finish().map_err(container)?;
        sink.flush()?;
        Ok(())
    }
}

/// The entry a layer is in, counted from 0.
pub(crate) fn layer_name(index: u32) -> String {
    format!("layer_images/layer_{index}.pw0Img")
}

/// A binary entry built in memory before it goes into the archive.
pub(crate) fn built(
    write: impl FnOnce(&mut core_format::Fields<'_>) -> std::io::Result<()>,
) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Cursor::new(Vec::new());
    write(&mut core_format::Fields::new(&mut bytes))?;
    Ok(bytes.into_inner())
}

fn entry(
    zip: &mut ZipWriter<&mut dyn WriteSeek>,
    name: &str,
    bytes: &[u8],
) -> Result<(), FormatError> {
    // Deflated as the vendor writes it: a run-length layer still shrinks to a fiftieth.
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, options).map_err(container)?;
    zip.write_all(bytes)?;
    Ok(())
}

fn container(source: zip::result::ZipError) -> FormatError {
    match source {
        zip::result::ZipError::Io(source) => FormatError::Io(source),
        other => FormatError::Encoding {
            what: "the archive",
            reason: other.to_string(),
        },
    }
}
