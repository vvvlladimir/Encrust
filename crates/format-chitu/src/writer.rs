use core_format::{
    Fields, FormatError, LayerSink, PrintJob, Rle7Layer, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::blocks::{self, HEADER_BYTES, Head, Offsets, SLICER_INFO_BYTES};
use crate::layer::{self, LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES, LayerDef};

/// Every `.ctb` from version 4 onwards carries this magic, version 5 included.
const MAGIC: u32 = 0x12FD_0106;

/// Which `.ctb` revision to write.
///
/// Both carry the same magic and differ in what the machine is told it may vary per
/// layer; version 5 adds the resin block. See `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CtbVersion {
    #[default]
    V4,
    V5,
}

impl CtbVersion {
    pub(crate) fn number(self) -> u32 {
        match self {
            Self::V4 => 4,
            Self::V5 => 5,
        }
    }

    /// The byte that tells the firmware per-layer settings are present and may be obeyed.
    pub(crate) fn per_layer_settings(self) -> u8 {
        match self {
            Self::V4 => 0x40,
            Self::V5 => 0x50,
        }
    }
    /// capability stamp, so a version has to name one that shipped with it.
    pub(crate) fn software_version(self) -> u32 {
        match self {
            Self::V4 => 0x0109_0000,
            Self::V5 => 0x0200_0000,
        }
    }

    fn head(self) -> Head {
        Head {
            magic: MAGIC,
            version: self.number(),

            // The masks already carry eight bits of grey each, so nothing is gained by
            // stacking several passes of a one-bit image, which is what a level above one
            // means here.
            grey_passes: 1,
            slicer_info_bytes: SLICER_INFO_BYTES,
        }
    }
}

/// Writes the Chitu `.ctb` container. Layout is in `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CtbWriter {
    pub version: CtbVersion,
}

impl CtbWriter {
    pub fn new(version: CtbVersion) -> Self {
        Self { version }
    }
}

impl SlicedFileWriter for CtbWriter {
    type Sink<'w> = CtbSink<'w>;

    fn extension(&self) -> &'static str {
        "ctb"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut fields = Fields::new(sink);
        let offsets = blocks::write_body(&mut fields, job, self.version)?;

        // The table is fixed-width and its rows are only known once their layers have
        // been compressed, so the space is reserved here and filled in by `finish`.
        fields.zeros((LAYER_DEF_BYTES * u64::from(job.layer_count())) as usize)?;
        tracing::debug!(
            layers = job.layer_count(),
            table = offsets.layer_table,
            "ctb header written"
        );

        Ok(CtbSink {
            fields,
            job: job.clone(),
            version: self.version,
            offsets,
            defs: Vec::with_capacity(job.layer_count() as usize),
        })
    }
}

/// A `.ctb` file with everything in front of its layers written.
pub struct CtbSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    version: CtbVersion,
    offsets: Offsets,
    defs: Vec<LayerDef>,
}

impl LayerSink for CtbSink<'_> {
    type Encoded = Rle7Layer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        Rle7Layer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.defs.len() as u32;
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

        let block = self.fields.position()?;
        let def = LayerDef {
            z_mm: self.job.layer_z_mm(index),
            exposure_s: self.job.exposure_of_layer_s(index),
            light_off_delay_s: self.job.material.light_off_s(),
            data_address: block as u32 + LAYER_DEF_EX_BYTES,
            data_size: encoded.data().len() as u32,
        };

        layer::write_extended(&mut self.fields, &self.job, index, &def)?;
        self.fields.bytes(encoded.data())?;
        self.defs.push(def);
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.defs.len() as u32;
        if written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written,
            });
        }

        self.fields.seek_to(u64::from(self.offsets.layer_table))?;
        for def in &self.defs {
            def.write(&mut self.fields, LAYER_DEF_EX_BYTES)?;
        }

        blocks::rewrite_print_parameters(
            &mut self.fields,
            &self.job,
            self.offsets.print_parameters,
        )?;

        self.fields.seek_to(0)?;
        blocks::write_header(
            &mut self.fields,
            &self.job,
            self.version.head(),
            self.offsets,
        )?;
        debug_assert_eq!(self.fields.position()?, HEADER_BYTES);

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
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = CtbWriter::default()
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
        let err = CtbWriter::default()
            .begin(&job, &mut buffer)
            .err()
            .expect("a mismatched panel cannot be written");
        assert!(matches!(err, FormatError::PanelMismatch { .. }));
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CtbWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(Rle7Layer::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CtbWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(CtbSink::encode(&layer()))
            .expect("the first layer");
        let err = sink.push(CtbSink::encode(&layer())).unwrap_err();
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
        let sink = CtbWriter::default()
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
    fn both_versions_announce_themselves_in_the_header() {
        for (version, number, per_layer) in [(CtbVersion::V4, 4, 0x40), (CtbVersion::V5, 5, 0x50)] {
            let mut buffer = Cursor::new(Vec::new());
            {
                let mut sink = CtbWriter::new(version)
                    .begin(&sample_job(1), &mut buffer)
                    .expect("the job is sound");
                sink.push(CtbSink::encode(&layer())).expect("the layer");
                sink.finish(0.0).expect("the promised layer arrived");
            }
            let bytes = buffer.into_inner();
            assert_eq!(
                u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                number
            );
            assert_eq!(version.per_layer_settings(), per_layer);
        }
        assert_eq!(CtbWriter::default().extension(), "ctb");
    }
}
