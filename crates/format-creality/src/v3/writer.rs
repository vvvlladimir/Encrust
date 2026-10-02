use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::crc::Crc32;
use crate::family::{block, model_code};

use super::blocks::{self, PAGE_BREAK};
use super::lines::EncodedLayer;

/// Writes the Creality `.cxdlp`. Layout is in `docs/formats/creality.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxdlpWriter;

impl SlicedFileWriter for CxdlpWriter {
    type Sink<'w> = CxdlpSink<'w>;

    fn extension(&self) -> &'static str {
        "cxdlp"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        // The header carries the layer count and the panel as sixteen-bit fields, so a
        // stack or a panel past that cannot be described at all.
        let layers = job.layer_count();
        if layers > u32::from(u16::MAX) {
            return Err(FormatError::Encoding {
                what: "the header",
                reason: format!("{layers} layers is past the 65535 the layer count holds"),
            });
        }

        let model = model_code(job).ok_or_else(|| FormatError::Missing {
            what: "a CL or CT model code in the machine name".to_owned(),
        })?;

        // The blocks in front of the layers are built once in memory — a few hundred
        // kilobytes, bounded by the preview sizes — because every byte of them has to go
        // through the checksum as well as to the sink, and the sink cannot be read back.
        let head = block(|fields| {
            blocks::write_header(fields, job, &model)?;
            blocks::write_previews(fields, job.thumbnail.as_ref())?;
            blocks::write_settings(fields, job)
        })?;
        let info = block(|fields| blocks::write_slicer_info(fields, job))?;

        let mut fields = Fields::new(sink);
        fields.bytes(&head)?;
        let area_table = fields.position()?;
        fields.zeros(4 * layers as usize)?;
        fields.bytes(&PAGE_BREAK)?;
        fields.bytes(&info)?;

        let mut before = Crc32::default();
        before.update(&head);
        let mut after = Crc32::default();
        after.update(&info);

        tracing::debug!(layers, model = %model, "cxdlp header written");
        Ok(CxdlpSink {
            fields,
            job: job.clone(),
            areas: Vec::with_capacity(layers as usize),
            area_table,
            before,
            after,
        })
    }
}

/// A `.cxdlp` with its header written and its layers still to come.
pub struct CxdlpSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    /// Each layer's cured area, which the table in front of the layers states.
    areas: Vec<u32>,
    /// Where that table begins, so it can be filled once the areas are known.
    area_table: u64,
    /// The sum over everything before the area table, and the one over everything after
    /// it: the table itself is only known once the last layer has landed.
    before: Crc32,
    after: Crc32,
}

impl LayerSink for CxdlpSink<'_> {
    type Encoded = Result<EncodedLayer, FormatError>;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.areas.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
        let encoded = encoded?;
        if encoded.width() != self.job.raster.width_px
            || encoded.height() != self.job.raster.height_px
        {
            return Err(FormatError::ResolutionMismatch {
                index: index as usize,
                width: encoded.width(),
                height: encoded.height(),
                expected_width: self.job.raster.width_px,
                expected_height: self.job.raster.height_px,
            });
        }

        let pitch = &self.job.raster.pitch;
        let area = encoded.area_field(pitch.x * pitch.y);
        self.areas.push(area);

        let mut body = Vec::with_capacity(8 + encoded.data().len() + PAGE_BREAK.len());
        body.extend_from_slice(&area.to_be_bytes());
        body.extend_from_slice(&encoded.lines().to_be_bytes());
        body.extend_from_slice(encoded.data());
        body.extend_from_slice(&PAGE_BREAK);

        self.after.update(&body);
        self.fields.bytes(&body)?;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        if self.areas.len() as u32 != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: self.areas.len() as u32,
            });
        }

        let footer = block(blocks::write_footer)?;
        self.after.update(&footer);
        self.fields.bytes(&footer)?;

        // The table in front of the layers is the one thing not known in order, so it is
        // filled now and its bytes are joined to the two sums either side of it.
        let mut table = Vec::with_capacity(4 * self.areas.len() + PAGE_BREAK.len());
        for area in &self.areas {
            table.extend_from_slice(&area.to_be_bytes());
        }
        table.extend_from_slice(&PAGE_BREAK);

        let end = self.fields.position()?;
        self.fields.seek_to(self.area_table)?;
        self.fields.bytes(&table)?;
        self.fields.seek_to(end)?;

        let mut middle = Crc32::default();
        middle.update(&table);
        let checksum = self
            .before
            .followed_by(middle)
            .followed_by(self.after)
            .value();
        self.fields.u32_be(checksum)?;
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
    fn a_machine_whose_name_carries_no_model_code_is_refused() {
        let mut job = sample_job(1);
        job.printer.name = "Some Printer".to_owned();
        job.printer.machine_name = None;
        let mut buffer = Cursor::new(Vec::new());
        let err = CxdlpWriter
            .begin(&job, &mut buffer)
            .err()
            .expect("a machine with no model code cannot be written");
        assert!(matches!(err, FormatError::Missing { what } if what.contains("model code")));
    }

    #[test]
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = CxdlpWriter
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CxdlpWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(CxdlpSink::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CxdlpWriter
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(CxdlpSink::encode(&layer())).expect("the first");
        let err = sink.push(CxdlpSink::encode(&layer())).unwrap_err();
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
        let sink = CxdlpWriter
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
