use std::io::Cursor;

use core_format::{
    CHITU_SLICER_FLAGS, ChituLayer, ChituTotals, Crc32, Fields, FormatError, LayerSink, PrintJob,
    ReadWriteSeek, SlicedFileWriter, check_layer_pushed, check_layers_finished, validate,
};
use core_raster::LayerRuns;

use crate::blocks::{self, HEADER_BYTES, Head, Offsets, SLICER_INFO_BYTES};
use crate::layer::{self, LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES, LAYER_TAIL_BYTES, LayerDef};

/// Every `.ctb` from version 4 onwards carries this magic, version 5 included.
const MAGIC: u32 = 0x12FD_0106;

/// Which `.ctb` revision to write.
///
/// Both carry the same magic and differ in what the machine is told it may vary per
/// layer; version 5 adds the resin block. See `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CtbVersion {
    V4,
    #[default]
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

    /// The slicer info's flag word: in its high byte whether the machine is to obey the
    /// per-layer blocks, which the vendor leaves clear while every layer is the header's.
    pub(crate) fn flags(self, job: &PrintJob) -> u32 {
        let per_layer = if job.varies_by_layer() {
            self.per_layer_settings()
        } else {
            0
        };
        CHITU_SLICER_FLAGS | u32::from(per_layer) << 24
    }

    fn head(self, job: &PrintJob) -> Head {
        Head {
            magic: MAGIC,
            version: self.number(),

            // The masks already carry seven bits of grey each, so nothing is gained by
            // stacking several passes of a one-bit image, which is what a level above one
            // means here.
            grey_passes: 1,
            slicer_info_bytes: SLICER_INFO_BYTES,
            encryption_key: encryption_key(job),
            states_height: false,
        }
    }
}

/// The key the layers are covered with: any but zero reads the same, and the vendor's
/// slicer never writes zero in a `.ctb`, `.fdg` or `.phz`; see `docs/decisions/0365`.
pub(crate) fn encryption_key(job: &PrintJob) -> u32 {
    job.created_unix_s as u32 | 1
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
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        // Everything in front of the layer table is kept, to be patched and summed at the end.
        let mut front = Cursor::new(Vec::new());
        let offsets = blocks::write_body(&mut Fields::new(&mut front), job, self.version)?;
        let front = front.into_inner();

        let mut fields = Fields::new(sink);
        fields.bytes(&front)?;
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
            front,
            defs: Vec::with_capacity(job.layer_count() as usize),
            totals: ChituTotals::default(),
            layers_crc: Crc32::default(),
        })
    }
}

/// A `.ctb` file with everything in front of its layers written.
pub struct CtbSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    version: CtbVersion,
    offsets: Offsets,
    front: Vec<u8>,
    defs: Vec<LayerDef>,
    totals: ChituTotals,
    layers_crc: Crc32,
}

impl LayerSink for CtbSink<'_> {
    type Encoded = ChituLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        ChituLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.defs.len() as u32;
        let runs = encoded.runs();
        check_layer_pushed(&self.job, index, (runs.width(), runs.height()))?;

        let block = self.fields.position()?;
        let def = LayerDef {
            z_mm: self.job.layer_z_mm(index),
            exposure_s: self.job.exposure_of_layer_s(index),
            light_off_delay_s: self.job.material.light_off_of_layer_s(index),
            data_address: block as u32 + LAYER_DEF_EX_BYTES,
            data_size: runs.data().len() as u32,
        };

        let job = &self.job;
        let bytes = encoded.block(job, encryption_key(job), index, |fields| {
            layer::write_extended(fields, job, index, &def)
        })?;
        self.layers_crc.update(&bytes);
        self.fields.bytes(&bytes)?;
        self.totals.add(job, index, def.data_size);
        self.defs.push(def);
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.defs.len() as u32;
        check_layers_finished(&self.job, written)?;

        let mut front = Cursor::new(std::mem::take(&mut self.front));
        {
            let mut patch = Fields::new(&mut front);
            blocks::rewrite_print_parameters(&mut patch, &self.job, self.offsets.print_parameters)?;
            blocks::rewrite_v4_parameters(&mut patch, &self.job, self.offsets, self.totals)?;
            patch.seek_to(0)?;
            blocks::write_header(
                &mut patch,
                &self.job,
                self.version.head(&self.job),
                self.offsets,
            )?;
            debug_assert_eq!(patch.position()?, HEADER_BYTES);
        }
        let front = front.into_inner();

        let mut table = Cursor::new(Vec::with_capacity(
            self.defs.len() * LAYER_DEF_BYTES as usize,
        ));
        {
            let mut rows = Fields::new(&mut table);
            for def in &self.defs {
                def.write(&mut rows, LAYER_DEF_EX_BYTES)?;
            }
        }
        let table = table.into_inner();

        let crc = Crc32::of(&front)
            .followed_by(Crc32::of(&table))
            .followed_by(self.layers_crc);

        self.fields.seek_to(0)?;
        self.fields.bytes(&front)?;
        self.fields.bytes(&table)?;
        self.fields.seek_to(
            u64::from(self.offsets.layer_table) + table.len() as u64 + self.layers_crc_len(),
        )?;
        // The file ends with the CRC of everything before it, as the vendor's slicer ends it.
        self.fields.u32_le(crc.zlib())?;
        self.fields.flush()?;
        Ok(())
    }
}

impl CtbSink<'_> {
    /// Bytes of every layer written, extended blocks and tails included.
    fn layers_crc_len(&self) -> u64 {
        self.defs
            .iter()
            .map(|def| u64::from(LAYER_DEF_EX_BYTES + def.data_size + LAYER_TAIL_BYTES))
            .sum()
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
            .push(CtbSink::encode(&LayerRuns::builder(4, 4).finish()))
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
