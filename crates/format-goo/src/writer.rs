use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::header::{self, ENDING_STRING};
use crate::layer;
use crate::rle::EncodedLayer;

/// Writes the Elegoo `.goo` container. Layout is in `docs/formats/goo.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct GooWriter;

impl SlicedFileWriter for GooWriter {
    type Sink<'w> = GooSink<'w>;

    fn extension(&self) -> &'static str {
        "goo"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut fields = Fields::new(sink);
        header::write(&mut fields, job)?;
        tracing::debug!(
            layers = job.layer_count(),
            bytes = fields.written(),
            "goo header written"
        );

        Ok(GooSink {
            fields,
            job: job.clone(),
            written: 0,
        })
    }
}

/// A `.goo` file with its header written, waiting for its layers.
pub struct GooSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    written: u32,
}

impl LayerSink for GooSink<'_> {
    type Encoded = EncodedLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        if self.written == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: self.written + 1,
            });
        }
        if encoded.width() != self.job.raster.width_px
            || encoded.height() != self.job.raster.height_px
        {
            return Err(FormatError::ResolutionMismatch {
                index: self.written as usize,
                width: encoded.width(),
                height: encoded.height(),
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        layer::write(&mut self.fields, &self.job, self.written, &encoded)?;
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        if self.written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: self.written,
            });
        }
        self.fields.bytes(&ENDING_STRING)?;

        // The header is a fixed layout of a fixed length, so the volume the stack came to
        // is written by laying it down again over the one written at the start.
        self.job.volume_mm3 = volume_mm3;
        self.fields.seek_to(0)?;
        header::write(&mut self.fields, &self.job)?;

        self.fields.flush()?;
        Ok(())
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
    fn a_job_without_layers_is_rejected_before_the_header() {
        let mut buffer = Cursor::new(Vec::new());
        let err = GooWriter
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
        assert!(buffer.into_inner().is_empty());
    }

    #[test]
    fn a_panel_that_does_not_match_the_masks_is_rejected() {
        let mut job = sample_job(1);
        job.raster.width_px = 16;
        let mut buffer = Cursor::new(Vec::new());
        let err = GooWriter
            .begin(&job, &mut buffer)
            .err()
            .expect("a mismatched panel cannot be written");
        assert!(matches!(err, FormatError::PanelMismatch { .. }));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = GooWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(EncodedLayer::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = GooWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(GooSink::encode(&layer()))
            .expect("the first layer");
        let err = sink.push(GooSink::encode(&layer())).unwrap_err();
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
        let sink = GooWriter
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

    #[test]
    fn a_finished_file_ends_with_the_ending_string() {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut sink = GooWriter
                .begin(&sample_job(2), &mut buffer)
                .expect("the job is sound");
            sink.push(GooSink::encode(&layer()))
                .expect("the first layer");
            sink.push(GooSink::encode(&layer()))
                .expect("the second layer");
            sink.finish(0.0).expect("both layers arrived");
        }
        let bytes = buffer.into_inner();
        assert_eq!(&bytes[bytes.len() - 11..], &ENDING_STRING);
        assert_eq!(GooWriter.extension(), "goo");
    }
}
