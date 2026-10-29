use std::io::Write;

use core_format::{
    FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, Thumbnail,
    check_layer_pushed, check_layers_finished, validate,
};
use core_raster::LayerRuns;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::gcode;

/// The entry the board runs, which is the whole of the file but its images.
pub(crate) const PROGRAM: &str = "run.gcode";

/// The previews the archive holds, in the order and at the sizes a printed file carries.
pub(crate) const THUMBNAILS: [(&str, u32, u32); 2] = [
    ("preview.png", 954, 850),
    ("preview_cropping.png", 168, 150),
];

/// Colour a preview is padded with where the square thumbnail does not reach.
const THUMBNAIL_BACKGROUND: [u8; 3] = [0, 0, 0];

/// Writes the zip of greyscale PNGs a Chitu board runs as gcode. Layout is in
/// `docs/formats/gcode-zip.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GcodeZipWriter;

impl SlicedFileWriter for GcodeZipWriter {
    type Sink<'w> = GcodeZipSink<'w>;

    fn extension(&self) -> &'static str {
        "zip"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut zip = ZipWriter::new(sink);
        write_thumbnails(&mut zip, job.thumbnail.as_ref())?;
        tracing::debug!(layers = job.layer_count(), "gcode zip previews written");

        Ok(GcodeZipSink {
            zip,
            job: job.clone(),
            blocks: String::new(),
            written: 0,
        })
    }
}

/// The archive with its previews in, its layers still to come and the program growing a
/// block at a time beside them.
pub struct GcodeZipSink<'w> {
    zip: ZipWriter<&'w mut dyn ReadWriteSeek>,
    job: PrintJob,
    /// One block per layer that has landed, in print order.
    blocks: String,
    written: u32,
}

impl LayerSink for GcodeZipSink<'_> {
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
        check_layer_pushed(&self.job, index, (encoded.width, encoded.height))?;

        let png = encoded.png?;
        entry(&mut self.zip, &gcode::image_name(index), &png)?;
        self.blocks.push_str(&gcode::layer_block(&self.job, index));
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        check_layers_finished(&self.job, self.written)?;

        // The program goes in last: its header states the resin the stack came to, which
        // is only known once the last layer is cut, and a zip entry cannot be rewritten.
        // A reader looks entries up by name, so the order does not reach it.
        let mut program = gcode::preamble(&self.job);
        program.push_str(&self.blocks);
        program.push_str(&gcode::epilogue(&self.job));
        entry(&mut self.zip, PROGRAM, program.as_bytes())?;

        let sink = self.zip.finish()?;
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
    zip: &mut ZipWriter<&mut dyn ReadWriteSeek>,
    thumbnail: Option<&Thumbnail>,
) -> Result<(), FormatError> {
    for (name, width, height) in THUMBNAILS {
        let image = Thumbnail::fitted_or_filled(thumbnail, width, height, THUMBNAIL_BACKGROUND);
        entry(zip, name, &core_format::encode_colour(&image)?)?;
    }
    Ok(())
}

fn entry(
    zip: &mut ZipWriter<&mut dyn ReadWriteSeek>,
    name: &str,
    bytes: &[u8],
) -> Result<(), FormatError> {
    // Deflated as known files are: a mask's PNG is mostly repeats a second pass folds away,
    // some fifteen times smaller for a fifth of a millisecond (docs/formats/gcode-zip.md).
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, options)?;
    zip.write_all(bytes)?;
    Ok(())
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
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = GcodeZipWriter
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = GcodeZipWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(GcodeZipSink::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = GcodeZipWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(GcodeZipSink::encode(&layer()))
            .expect("the first");
        let err = sink.push(GcodeZipSink::encode(&layer())).unwrap_err();
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
        let sink = GcodeZipWriter
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
