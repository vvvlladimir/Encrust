use core_format::{
    Fields, FormatError, LayerSink, PrintJob, Rle7Layer, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::crc::Crc32;
use crate::family::{block, block_at, model_code};

use super::blocks::{self, LAYER_BLOCK_BYTES, LAYER_ROW_BYTES, LayerRow, Offsets};

/// Writes the Creality `.cxdlp` at version 4. Layout is in `docs/formats/creality.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxdlpV4Writer;

impl SlicedFileWriter for CxdlpV4Writer {
    type Sink<'w> = CxdlpV4Sink<'w>;

    fn extension(&self) -> &'static str {
        "cxdlp"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let (width, height) = (job.raster.width_px, job.raster.height_px);
        if width > u32::from(u16::MAX) || height > u32::from(u16::MAX) {
            return Err(FormatError::Encoding {
                what: "the header",
                reason: format!("a {width}x{height} panel is past the 65535 the panel fields hold"),
            });
        }

        let model = model_code(job).ok_or_else(|| FormatError::Missing {
            what: "a CL or CT model code in the machine name".to_owned(),
        })?;

        // The blocks in front of the layer table are built in memory because the header
        // addresses them and the checksum covers them, and the sink cannot be read back.
        let header_bytes = blocks::header_bytes(&model);
        let (body, offsets) = block_at(header_bytes, |fields| blocks::write_body(fields, job))?;

        let mut fields = Fields::new(sink);
        fields.seek_to(header_bytes)?;
        fields.bytes(&body)?;
        fields.zeros((LAYER_ROW_BYTES * u64::from(job.layer_count())) as usize)?;

        tracing::debug!(
            layers = job.layer_count(),
            model = %model,
            table = offsets.layer_table,
            "cxdlp version 4 header written"
        );
        Ok(CxdlpV4Sink {
            fields,
            job: job.clone(),
            model,
            header_bytes,
            offsets,
            rows: Vec::with_capacity(job.layer_count() as usize),
            layers: Crc32::default(),
        })
    }
}

/// A version 4 `.cxdlp` with its tables reserved and its layers still to come.
pub struct CxdlpV4Sink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    model: String,
    header_bytes: u64,
    offsets: Offsets,
    rows: Vec<LayerRow>,
    /// The sum over the layers as they land. The three blocks either side of them are only
    /// final once the stack is, so each is summed in `finish` and the four are joined.
    layers: Crc32,
}

impl LayerSink for CxdlpV4Sink<'_> {
    /// The compressed layer and what it lights, which the table states and the codec does
    /// not keep.
    type Encoded = (Rle7Layer, u64);

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        let lit_px = layer
            .runs()
            .iter()
            .filter(|run| run.value > 0)
            .map(|run| u64::from(run.length))
            .sum();
        (Rle7Layer::encode(layer), lit_px)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let (encoded, lit_px) = encoded;
        let index = self.rows.len() as u32;
        if index == self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written: index + 1,
            });
        }
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

        let address = self.fields.position()?;
        let mut body = block(|fields| blocks::write_layer_block(fields, &self.job, index))?;
        body.extend_from_slice(encoded.data());

        self.layers.update(&body);
        self.fields.bytes(&body)?;
        self.rows.push(LayerRow {
            z_mm: self.job.layer_z_mm(index),
            exposure_s: self.job.exposure_of_layer_s(index),
            light_off_delay_s: self.job.material.light_off_s(),
            address: address as u32,
            size: LAYER_BLOCK_BYTES + encoded.data().len() as u32,
            area: area_field(&self.job, lit_px),
        });
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
        let end = self.fields.position()?;

        // The resin the print takes is only known now, so the blocks that state it are
        // written again over the same bytes; nothing in them varies in length.
        let (body, _) = block_at(self.header_bytes, |fields| {
            blocks::write_body(fields, &self.job)
        })?;
        self.fields.seek_to(self.header_bytes)?;
        self.fields.bytes(&body)?;

        let table = block(|fields| {
            for row in &self.rows {
                row.write(fields)?;
            }
            Ok(())
        })?;
        debug_assert_eq!(
            self.header_bytes + body.len() as u64,
            u64::from(self.offsets.layer_table)
        );
        self.fields.bytes(&table)?;

        let header =
            block(|fields| blocks::write_header(fields, &self.job, &self.model, self.offsets))?;
        debug_assert_eq!(header.len() as u64, self.header_bytes);
        self.fields.seek_to(0)?;
        self.fields.bytes(&header)?;

        let checksum = sum(&[&header, &body, &table])
            .followed_by(self.layers)
            .value();
        self.fields.seek_to(end)?;
        self.fields.u32_be(checksum)?;
        self.fields.flush()?;
        Ok(())
    }
}

/// What the layer cures in square millimetres times a thousand, which is the unit the
/// row's area field is in. It carries everything the layer lights rather than the largest
/// island a vendor's slicer measures; see `docs/formats/creality.md`.
fn area_field(job: &PrintJob, lit_px: u64) -> u32 {
    let pitch = &job.raster.pitch;
    let area = lit_px as f64 * f64::from(pitch.x * pitch.y) * 1000.0;
    area.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// The sum over several blocks written one after another.
fn sum(blocks: &[&[u8]]) -> Crc32 {
    blocks.iter().fold(Crc32::default(), |total, block| {
        let mut next = Crc32::default();
        next.update(block);
        total.followed_by(next)
    })
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
        let err = CxdlpV4Writer
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
    }

    #[test]
    fn a_machine_whose_name_carries_no_model_code_is_refused() {
        let mut job = sample_job(1);
        job.printer.name = "Some Printer".to_owned();
        job.printer.machine_name = None;
        let mut buffer = Cursor::new(Vec::new());
        let err = CxdlpV4Writer
            .begin(&job, &mut buffer)
            .err()
            .expect("a machine with no model code cannot be written");
        assert!(matches!(err, FormatError::Missing { what } if what.contains("model code")));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CxdlpV4Writer
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(CxdlpV4Sink::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CxdlpV4Writer
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(CxdlpV4Sink::encode(&layer())).expect("the first");
        let err = sink.push(CxdlpV4Sink::encode(&layer())).unwrap_err();
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
        let sink = CxdlpV4Writer
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
    fn a_panel_wider_than_the_sixteen_bit_field_is_refused() {
        let mut job = sample_job(1);
        job.printer.display.width_px = 70_000;
        job.raster.width_px = 70_000;
        let mut buffer = Cursor::new(Vec::new());
        let err = CxdlpV4Writer
            .begin(&job, &mut buffer)
            .err()
            .expect("a panel the header cannot describe is refused");
        assert!(matches!(err, FormatError::Encoding { what, .. } if what == "the header"));
    }
}
