use std::io::Write;

use core_format::{
    FormatError, LayerSink, PrintJob, SlicedFileWriter, Thumbnail, WriteSeek, validate,
};
use core_raster::LayerRuns;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::config;

/// Digits a layer's entry is numbered with. A reader finds layers by this shape alone, so
/// the count has to be exactly five.
const INDEX_DIGITS: usize = 5;

/// What a reader matches the tail of an entry name against.
pub(crate) fn index_digits() -> usize {
    INDEX_DIGITS
}

/// The previews the container holds, each under its own name.
const THUMBNAILS_PX: [(u32, u32); 2] = [(400, 400), (800, 480)];

/// Colour a preview is padded with where the square thumbnail does not reach.
const THUMBNAIL_BACKGROUND: [u8; 3] = [0, 0, 0];

/// Which of the two Prusa extensions to write.
///
/// The container is the same; each names its own machine and archive format, which is what
/// a reader takes the machine's tilt times from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sl1Flavour {
    /// Original SL1.
    #[default]
    Sl1,
    /// SL1S Speed.
    Sl1s,
}

impl Sl1Flavour {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Sl1 => "sl1",
            Self::Sl1s => "sl1s",
        }
    }

    /// The flavour `extension` names, without the dot and in any case.
    pub fn of_extension(extension: &str) -> Option<Self> {
        [Self::Sl1, Self::Sl1s]
            .into_iter()
            .find(|flavour| extension.eq_ignore_ascii_case(flavour.extension()))
    }

    pub(crate) fn printer_model(self) -> &'static str {
        match self {
            Self::Sl1 => "SL1",
            Self::Sl1s => "SL1S",
        }
    }

    /// What the archive format key states: `SL1` whichever extension it is.
    ///
    /// A genuine `.sl1s` from Prusa's own slicer carries `SL1` too: the key names the
    /// container rather than the machine, and `printer_model` names the machine.
    pub(crate) const ARCHIVE_FORMAT: &'static str = "SL1";
}

/// The sizes the preview key lists, in the order the entries are written.
pub(crate) fn thumbnail_sizes() -> String {
    THUMBNAILS_PX
        .iter()
        .map(|(width, height)| format!("{width}x{height}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Writes the Prusa `.sl1` container. Layout is in `docs/formats/sl1.md`.
#[derive(Debug, Default, Clone)]
pub struct Sl1Writer {
    pub flavour: Sl1Flavour,
    /// What the layer entries are named after, which is the output file's own stem.
    pub job_dir: String,
}

impl Sl1Writer {
    pub fn new(flavour: Sl1Flavour, job_dir: impl Into<String>) -> Self {
        Self {
            flavour,
            job_dir: job_dir.into(),
        }
    }
}

impl SlicedFileWriter for Sl1Writer {
    type Sink<'w> = Sl1Sink<'w>;

    fn extension(&self) -> &'static str {
        self.flavour.extension()
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        // The container states one exposure and one layer height for the whole stack, so a
        // stack that varies would be written as one that does not.
        if !job.exposure.is_empty() {
            return Err(FormatError::FixedForWholeStack {
                format: "sl1",
                field: "exposure",
            });
        }
        if !job.is_uniform() {
            return Err(FormatError::FixedForWholeStack {
                format: "sl1",
                field: "layer height",
            });
        }

        let mut zip = ZipWriter::new(sink);
        write_thumbnails(&mut zip, job.thumbnail.as_ref())?;
        tracing::debug!(
            layers = job.layer_count(),
            job_dir = self.job_dir,
            "sl1 previews written"
        );

        Ok(Sl1Sink {
            zip,
            job: job.clone(),
            flavour: self.flavour,
            job_dir: self.job_dir.clone(),
            written: 0,
        })
    }
}

/// An `.sl1` archive with its previews in and its layers still to come.
pub struct Sl1Sink<'w> {
    zip: ZipWriter<&'w mut dyn WriteSeek>,
    job: PrintJob,
    flavour: Sl1Flavour,
    job_dir: String,
    written: u32,
}

impl LayerSink for Sl1Sink<'_> {
    type Encoded = EncodedLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedLayer {
            width: layer.width(),
            height: layer.height(),
            png: core_format::encode_grey(layer),
        }
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.written;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        if encoded.width != self.job.raster.width_px || encoded.height != self.job.raster.height_px
        {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width: encoded.width,
                height: encoded.height,
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        let png = encoded.png?;
        let name = format!("{}{index:0width$}.png", self.job_dir, width = INDEX_DIGITS);
        entry(&mut self.zip, &name, &png)?;
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        if self.written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: self.written,
            });
        }

        // The two settings files go in last because one of their values is the resin the
        // stack came to, and an archive entry cannot be rewritten once it is closed. A
        // reader looks entries up by name, so the order does not reach it.
        let config = config::config_ini(&self.job, self.flavour, &self.job_dir);
        entry(&mut self.zip, "config.ini", config.as_bytes())?;
        let settings = config::prusaslicer_ini(&self.job, self.flavour);
        entry(&mut self.zip, "prusaslicer.ini", settings.as_bytes())?;

        let sink = self.zip.finish().map_err(container)?;
        sink.flush()?;
        Ok(())
    }
}

/// One layer already compressed, or why it could not be.
///
/// The encoding happens off the sink and so cannot report through it; the failure is
/// carried to `push` instead of being unwrapped on another thread.
pub struct EncodedLayer {
    width: u32,
    height: u32,
    png: Result<Vec<u8>, FormatError>,
}

fn write_thumbnails(
    zip: &mut ZipWriter<&mut dyn WriteSeek>,
    thumbnail: Option<&Thumbnail>,
) -> Result<(), FormatError> {
    for (width, height) in THUMBNAILS_PX {
        let image = match thumbnail {
            Some(thumbnail) => thumbnail.fitted_to(width, height, THUMBNAIL_BACKGROUND),
            None => Thumbnail::filled(width, height, THUMBNAIL_BACKGROUND),
        };
        let png = core_format::encode_colour(&image)?;
        entry(
            zip,
            &format!("thumbnail/thumbnail{width}x{height}.png"),
            &png,
        )?;
    }
    Ok(())
}

fn entry(
    zip: &mut ZipWriter<&mut dyn WriteSeek>,
    name: &str,
    bytes: &[u8],
) -> Result<(), FormatError> {
    // A PNG is already deflated, and so is stored rather than deflated a second time; the
    // two settings files are small enough that it makes no difference either way.
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;
    use core_format::{ExposurePlan, ExposureRange, LayerPlan};

    fn layer() -> LayerRuns {
        LayerRuns::builder(8, 4).finish()
    }

    #[test]
    fn each_extension_round_trips_through_its_own_name() {
        assert_eq!(Sl1Flavour::of_extension("sl1"), Some(Sl1Flavour::Sl1));
        assert_eq!(Sl1Flavour::of_extension("SL1S"), Some(Sl1Flavour::Sl1s));
        assert_eq!(Sl1Flavour::of_extension("goo"), None);
    }

    #[test]
    fn the_previews_are_named_after_the_sizes_the_settings_list() {
        assert_eq!(thumbnail_sizes(), "400x400,800x480");
    }

    #[test]
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = Sl1Writer::default()
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
    }

    #[test]
    fn a_stack_whose_exposure_varies_is_refused() {
        let mut job = sample_job(4);
        job.exposure = ExposurePlan::new(vec![ExposureRange::new(0.0, 1.0, 3.0)]);
        let mut buffer = Cursor::new(Vec::new());
        let err = Sl1Writer::default()
            .begin(&job, &mut buffer)
            .err()
            .expect("the container cannot carry it");
        assert!(matches!(
            err,
            FormatError::FixedForWholeStack {
                field: "exposure",
                ..
            }
        ));
    }

    #[test]
    fn a_stack_of_mixed_thicknesses_is_refused() {
        let mut job = sample_job(3);
        job.printer.firmware.variable_layer_height = true;
        job.plan = LayerPlan::from_bounds(vec![0.0, 0.05, 0.15, 0.2], 0.2);
        let mut buffer = Cursor::new(Vec::new());
        let err = Sl1Writer::default()
            .begin(&job, &mut buffer)
            .err()
            .expect("the container cannot carry it");
        assert!(matches!(
            err,
            FormatError::FixedForWholeStack {
                field: "layer height",
                ..
            }
        ));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = Sl1Writer::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(Sl1Sink::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = Sl1Writer::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(Sl1Sink::encode(&layer())).expect("the first");
        let err = sink.push(Sl1Sink::encode(&layer())).unwrap_err();
        assert!(matches!(
            err,
            FormatError::LayerCountMismatch {
                expected: 1,
                written: 2
            }
        ));
    }

    #[test]
    fn fewer_layers_than_promised_are_refused_at_the_end() {
        let mut buffer = Cursor::new(Vec::new());
        let sink = Sl1Writer::default()
            .begin(&sample_job(2), &mut buffer)
            .expect("the job is sound");
        let err = sink.finish(0.0).unwrap_err();
        assert!(matches!(
            err,
            FormatError::LayerCountMismatch {
                expected: 2,
                written: 0
            }
        ));
    }
}
