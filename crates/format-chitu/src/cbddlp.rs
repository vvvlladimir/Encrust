use core_format::{
    Fields, FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, check_layer_pushed,
    check_layers_finished, validate,
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
        sink: &'w mut dyn ReadWriteSeek,
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
        check_layer_pushed(&self.job, index, (encoded.width(), encoded.height()))?;

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
        check_layers_finished(&self.job, written)?;

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
        encryption_key: 0,
        states_height: true,
    }
}

#[cfg(test)]
mod tests;
