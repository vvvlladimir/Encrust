use std::io::Write;

use core_format::{FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate};
use core_raster::LayerRuns;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::conf::{self, CONF};
use crate::gcode;

/// What the images and the program are named after. The container numbers its images, and
/// a stem ending in a digit would run into that number.
pub(crate) const STEM: &str = "encrust";

/// Fewest digits an image's number is written in, as a vendor file writes it.
pub(crate) const DIGITS: usize = 4;

/// Writes the `.cws` archive. Layout is in `docs/formats/cws.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CwsWriter;

impl SlicedFileWriter for CwsWriter {
    type Sink<'w> = CwsSink<'w>;

    fn extension(&self) -> &'static str {
        "cws"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut zip = ZipWriter::new(sink);
        entry(&mut zip, CONF, conf::conf(job).as_bytes())?;
        tracing::debug!(layers = job.layer_count(), "cws settings written");

        Ok(CwsSink {
            zip,
            job: job.clone(),
            blocks: String::new(),
            written: 0,
        })
    }
}

/// The archive with its settings in, its images still to come and the program growing a
/// block at a time beside them.
pub struct CwsSink<'w> {
    zip: ZipWriter<&'w mut dyn WriteSeek>,
    job: PrintJob,
    /// One block per layer that has landed, in print order.
    blocks: String,
    written: u32,
}

impl LayerSink for CwsSink<'_> {
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
        let name = image_name(index, self.job.layer_count());
        entry(&mut self.zip, &name, &png)?;
        self.blocks.push_str(&gcode::layer_block(&self.job, index));
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

        // The program goes in last because it is the layer blocks, which are only known
        // once their layers have been written. A reader looks entries up by name.
        let mut program = gcode::preamble(&self.job);
        program.push_str(&self.blocks);
        program.push_str(&gcode::epilogue(&self.job));
        entry(&mut self.zip, &format!("{STEM}.gcode"), program.as_bytes())?;

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

/// What one layer's image is called: the stem and the layer's number, from zero, in as many
/// digits as the stack needs.
pub(crate) fn image_name(index: u32, layer_count: u32) -> String {
    let digits = DIGITS.max(layer_count.saturating_sub(1).to_string().len());
    format!("{STEM}{index:0digits$}.png")
}

fn entry(
    zip: &mut ZipWriter<&mut dyn WriteSeek>,
    name: &str,
    bytes: &[u8],
) -> Result<(), FormatError> {
    // A PNG is already deflated, and so is stored rather than deflated a second time; the
    // settings and the program are small enough that it makes no difference either way.
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

    fn layer() -> LayerRuns {
        LayerRuns::builder(8, 4).finish()
    }

    #[test]
    fn an_image_is_numbered_from_zero_in_four_digits_or_more() {
        assert_eq!(image_name(0, 10), "encrust0000.png");
        assert_eq!(image_name(7, 10), "encrust0007.png");
        assert_eq!(image_name(12_345, 20_000), "encrust12345.png");
    }

    #[test]
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = CwsWriter
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CwsWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(CwsSink::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CwsWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(CwsSink::encode(&layer())).expect("the first");
        let err = sink.push(CwsSink::encode(&layer())).unwrap_err();
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
        let sink = CwsWriter
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
