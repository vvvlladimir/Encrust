use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, validate,
};
use core_raster::LayerRuns;

use crate::blocks::{self, HEADER_BYTES, Head, Offsets};
use crate::layer::{LAYER_DEF_BYTES, LayerDef};
use crate::rle1::{EncodedPasses, GREY_PASSES};

/// The magic both `.cbddlp` and `.photon` carry.
const MAGIC: u32 = 0x12FD_0019;

/// The revision we write. Version 1 carries no print parameters and no grey at all.
const VERSION: u32 = 2;

/// Which extension the older Chitu container is written under.
///
/// The bytes are the same either way, so this is the caller's choice and not a property
/// of the file; it is the extension a machine's firmware looks for.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CbddlpFlavour {
    #[default]
    Cbddlp,
    Photon,
}

impl CbddlpFlavour {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Cbddlp => "cbddlp",
            Self::Photon => "photon",
        }
    }
}

/// Writes the older Chitu container, version 2. Layout is in `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CbddlpWriter {
    pub flavour: CbddlpFlavour,
}

impl CbddlpWriter {
    pub fn new(flavour: CbddlpFlavour) -> Self {
        Self { flavour }
    }
}

impl SlicedFileWriter for CbddlpWriter {
    type Sink<'w> = CbddlpSink<'w>;

    fn extension(&self) -> &'static str {
        self.flavour.extension()
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;

        let mut fields = Fields::new(sink);
        let offsets = blocks::write_cbddlp_body(&mut fields, job)?;

        let rows = u64::from(job.layer_count()) * u64::from(GREY_PASSES);
        fields.zeros((LAYER_DEF_BYTES * rows) as usize)?;
        tracing::debug!(
            layers = job.layer_count(),
            passes = GREY_PASSES,
            table = offsets.layer_table,
            "cbddlp header written"
        );

        Ok(CbddlpSink {
            fields,
            job: job.clone(),
            offsets,
            defs: Vec::with_capacity(rows as usize),
        })
    }
}

/// A `.cbddlp` file with everything in front of its layers written.
pub struct CbddlpSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    offsets: Offsets,
    /// Every pass of every layer, in the order the layers arrived. The table itself is
    /// written pass-major, which `finish` does by striding this.
    defs: Vec<LayerDef>,
}

impl CbddlpSink<'_> {
    fn layers_pushed(&self) -> u32 {
        self.defs.len() as u32 / GREY_PASSES
    }
}

impl LayerSink for CbddlpSink<'_> {
    type Encoded = EncodedPasses;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        EncodedPasses::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.layers_pushed();
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

        // The passes of one layer sit together, so the stack streams even though the
        // table that addresses them is ordered the other way round.
        for pass in encoded.passes() {
            let at = self.fields.position()? as u32;
            self.fields.bytes(pass)?;
            self.defs.push(LayerDef {
                z_mm: self.job.layer_z_mm(index),
                exposure_s: self.job.exposure_of_layer_s(index),
                light_off_delay_s: self.job.material.light_off_s(),
                data_address: at,
                data_size: pass.len() as u32,
            });
        }
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.layers_pushed();
        if written != self.job.layer_count() {
            return Err(FormatError::LayerCountMismatch {
                expected: self.job.layer_count(),
                written,
            });
        }

        self.fields.seek_to(u64::from(self.offsets.layer_table))?;
        for pass in 0..GREY_PASSES as usize {
            for layer in 0..written as usize {
                let def = self.defs[layer * GREY_PASSES as usize + pass];
                def.write(&mut self.fields, LAYER_DEF_BYTES as u32)?;
            }
        }

        blocks::rewrite_print_parameters(
            &mut self.fields,
            &self.job,
            self.offsets.print_parameters,
        )?;

        self.fields.seek_to(0)?;
        blocks::write_header(&mut self.fields, &self.job, head(), self.offsets)?;
        debug_assert_eq!(self.fields.position()?, HEADER_BYTES);

        self.fields.flush()?;
        Ok(())
    }
}

fn head() -> Head {
    Head {
        magic: MAGIC,
        version: VERSION,
        grey_passes: GREY_PASSES,

        // The container has no slicer info block, and says so with a zero size beside a
        // zero address.
        slicer_info_bytes: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fixtures::sample_job;
    use crate::rle1;

    fn layer() -> LayerRuns {
        LayerRuns::builder(8, 4).finish()
    }

    fn written(layers: u32) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut sink = CbddlpWriter::default()
                .begin(&sample_job(layers), &mut buffer)
                .expect("the job is sound");
            for _ in 0..layers {
                sink.push(CbddlpSink::encode(&layer())).expect("a layer");
            }
            sink.finish(0.0).expect("every promised layer arrived");
        }
        buffer.into_inner()
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    #[test]
    fn the_header_names_the_older_container_and_its_passes() {
        let bytes = written(2);
        assert_eq!(u32_at(&bytes, 0x00), MAGIC);
        assert_eq!(u32_at(&bytes, 0x04), VERSION);
        assert_eq!(
            u32_at(&bytes, 0x44),
            2,
            "the layer count is physical layers"
        );
        assert_eq!(u32_at(&bytes, 0x5C), GREY_PASSES);
        assert_eq!(u32_at(&bytes, 0x68), 0, "there is no slicer info block");
        assert_eq!(u32_at(&bytes, 0x6C), 0);
    }

    #[test]
    fn the_table_holds_one_row_per_pass_and_ends_where_the_data_starts() {
        let bytes = written(3);
        let table = u32_at(&bytes, 0x40) as usize;
        let rows = 3 * GREY_PASSES as usize;
        let first_data = u32_at(&bytes, table + 0x0C) as usize;
        assert_eq!(
            table + rows * LAYER_DEF_BYTES as usize,
            first_data,
            "the layer data begins at the end of the table"
        );
        for row in 0..rows {
            let at = table + row * LAYER_DEF_BYTES as usize;
            assert_eq!(
                u32_at(&bytes, at + 0x18),
                LAYER_DEF_BYTES as u32,
                "no extended block follows a row"
            );
        }
    }

    #[test]
    fn a_row_addresses_the_pass_of_its_own_layer() {
        let bytes = written(2);
        let table = u32_at(&bytes, 0x40) as usize;
        let row = |index: usize| {
            let at = table + index * LAYER_DEF_BYTES as usize;
            (u32_at(&bytes, at + 0x0C), u32_at(&bytes, at + 0x10))
        };

        // Rows run pass-major: row `pass * layers + layer`. Layer one's first pass sits
        // after all of layer zero's, so it is further into the file than row one is.
        let (first_of_layer_0, size) = row(0);
        let (first_of_layer_1, _) = row(1);
        let (second_of_layer_0, _) = row(2);
        assert_eq!(second_of_layer_0, first_of_layer_0 + size);
        assert!(first_of_layer_1 > second_of_layer_0);
    }

    #[test]
    fn the_greys_of_a_layer_come_back_out_of_the_file() {
        let mut job = sample_job(1);
        job.raster.width_px = 4;
        job.raster.height_px = 1;
        job.printer.display.width_px = 4;
        job.printer.display.height_px = 1;

        let mut runs = LayerRuns::builder(4, 1);
        runs.push(1, 255);
        runs.push(1, 100);
        runs.push(1, 40);
        let runs = runs.finish();

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut sink = CbddlpWriter::default()
                .begin(&job, &mut buffer)
                .expect("the job is sound");
            sink.push(CbddlpSink::encode(&runs)).expect("the layer");
            sink.finish(0.0).expect("the promised layer arrived");
        }
        let bytes = buffer.into_inner();
        let table = u32_at(&bytes, 0x40) as usize;
        let passes: Vec<Vec<u8>> = (0..GREY_PASSES as usize)
            .map(|pass| {
                let at = table + pass * LAYER_DEF_BYTES as usize;
                let from = u32_at(&bytes, at + 0x0C) as usize;
                let size = u32_at(&bytes, at + 0x10) as usize;
                bytes[from..from + size].to_vec()
            })
            .collect();

        let decoded = rle1::decode_passes(&passes, 4).expect("the file addresses its own passes");
        let values: Vec<u8> = decoded
            .iter()
            .flat_map(|run| std::iter::repeat_n(run.value, run.length as usize))
            .collect();
        assert_eq!(
            values,
            [255, 95, 31, 0],
            "each grey lands on the step below it, and the padding stays black"
        );
    }

    #[test]
    fn a_job_without_layers_is_rejected_before_anything_is_written() {
        let mut buffer = Cursor::new(Vec::new());
        let err = CbddlpWriter::default()
            .begin(&sample_job(0), &mut buffer)
            .err()
            .expect("an empty job cannot be written");
        assert!(matches!(err, FormatError::EmptyJob));
        assert!(buffer.into_inner().is_empty());
    }

    #[test]
    fn a_layer_of_the_wrong_size_is_rejected() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CbddlpWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        let err = sink
            .push(EncodedPasses::encode(&LayerRuns::builder(4, 4).finish()))
            .unwrap_err();
        assert!(matches!(
            err,
            FormatError::ResolutionMismatch { index: 0, .. }
        ));
    }

    #[test]
    fn more_layers_than_promised_are_refused() {
        let mut buffer = Cursor::new(Vec::new());
        let mut sink = CbddlpWriter::default()
            .begin(&sample_job(1), &mut buffer)
            .expect("the job is sound");
        sink.push(CbddlpSink::encode(&layer()))
            .expect("the first layer");
        let err = sink.push(CbddlpSink::encode(&layer())).unwrap_err();
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
        let sink = CbddlpWriter::default()
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
    fn the_two_extensions_are_the_same_file_under_different_names() {
        assert_eq!(CbddlpWriter::default().extension(), "cbddlp");
        assert_eq!(
            CbddlpWriter::new(CbddlpFlavour::Photon).extension(),
            "photon"
        );
    }
}
