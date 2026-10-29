//! The `.fdg` container: the Chitu layer records and previews under a header of its own and
//! a one-bit layer codec. Layout is in `docs/formats/fdg.md`; `.phz` is this layout under
//! another header.

use std::io;

use core_format::{
    Fields, FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, check_layer_pushed,
    check_layers_finished, validate, write_chitu_previews,
};
use core_raster::LayerRuns;

use crate::layer::{self, LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES, LayerDef};
use crate::writer::encryption_key;

mod header;
mod reader;
mod rle;
#[cfg(test)]
mod tests;

pub(crate) use header::{HEADER_BYTES, Offsets, Totals, anti_alias_level, write_tail};
pub use reader::{FdgReader, OpenFdg};
pub(crate) use reader::{Head, MotionAt, expect_magic, open_layout};
pub use rle::FdgLayer;

/// Two heights closer than this are one height, as the header counts them.
const SAME_HEIGHT_MM: f32 = 0.001;

/// Writes the header of a container of this layout, with what only the end of the stack
/// tells; it is written over the reserved bytes once every layer is in.
pub(crate) type WriteHeader = fn(&mut Fields<'_>, &PrintJob, Offsets, Totals) -> io::Result<()>;

/// Covers a layer's bytes with the container's keyed exclusive or, from its key and index.
pub(crate) type Crypt = fn(u32, u32, &mut [u8]);

/// Writes the `.fdg` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct FdgWriter;

impl SlicedFileWriter for FdgWriter {
    type Sink<'w> = FdgSink<'w>;

    fn extension(&self) -> &'static str {
        "fdg"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        FdgSink::open(job, sink, header::write_header, rle::crypt)
    }
}

/// An `.fdg` or `.phz` file with everything in front of its layers written.
pub struct FdgSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    write_header: WriteHeader,
    crypt: Crypt,
    offsets: Offsets,
    totals: Totals,
    previous_z_mm: f32,
    defs: Vec<LayerDef>,
}

impl<'w> FdgSink<'w> {
    /// Writes the previews, the machine name and room for the header and the table.
    pub(crate) fn open(
        job: &PrintJob,
        sink: &'w mut dyn ReadWriteSeek,
        write_header: WriteHeader,
        crypt: Crypt,
    ) -> Result<Self, FormatError> {
        validate(job)?;

        let mut fields = Fields::new(sink);
        fields.seek_to(HEADER_BYTES)?;
        let mut offsets = Offsets::default();
        (offsets.large_preview, offsets.small_preview) = write_chitu_previews(&mut fields, job)?;

        let name = job.printer.machine_name().as_bytes();
        offsets.machine_name = fields.position()? as u32;
        offsets.machine_name_bytes = name.len() as u32;
        fields.bytes(name)?;

        // The table's rows are only known once their layers are compressed.
        offsets.layer_table = fields.position()? as u32;
        fields.zeros((LAYER_DEF_BYTES * u64::from(job.layer_count())) as usize)?;

        Ok(Self {
            fields,
            job: job.clone(),
            write_header,
            crypt,
            offsets,
            totals: Totals::default(),
            previous_z_mm: f32::MIN_POSITIVE,
            defs: Vec::with_capacity(job.layer_count() as usize),
        })
    }
}

impl LayerSink for FdgSink<'_> {
    type Encoded = FdgLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        FdgLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.defs.len() as u32;
        check_layer_pushed(&self.job, index, (encoded.width(), encoded.height()))?;

        let block = self.fields.position()?;
        let def = LayerDef {
            z_mm: self.job.layer_z_mm(index),
            exposure_s: self.job.exposure_of_layer_s(index),
            light_off_delay_s: self.job.material.light_off_of_layer_s(index),
            data_address: block as u32 + LAYER_DEF_EX_BYTES,
            data_size: encoded.data().len() as u32,
        };
        let mut data = encoded.data().to_vec();
        (self.crypt)(encryption_key(&self.job), index, &mut data);
        layer::write_extended(&mut self.fields, &self.job, index, &def)?;
        self.fields.bytes(&data)?;
        self.tally(index, &def);
        self.defs.push(def);
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        self.job.volume_mm3 = volume_mm3;
        let written = self.defs.len() as u32;
        check_layers_finished(&self.job, written)?;

        self.fields.seek_to(u64::from(self.offsets.layer_table))?;
        for def in &self.defs {
            def.write(&mut self.fields, LAYER_DEF_EX_BYTES)?;
        }
        self.fields.seek_to(0)?;
        (self.write_header)(&mut self.fields, &self.job, self.offsets, self.totals)?;
        debug_assert_eq!(self.fields.position()?, HEADER_BYTES);
        self.fields.flush()?;
        Ok(())
    }
}

impl FdgSink<'_> {
    /// Adds a written layer to what the header sums over the stack.
    fn tally(&mut self, index: u32, def: &LayerDef) {
        let totals = &mut self.totals;
        totals.stack.add(&self.job, index, def.data_size);

        let bottom_mm = self.job.nominal_height_mm() * self.job.material.bottom_layers as f32;
        if def.z_mm < bottom_mm {
            totals.last_bottom_index = index;
        }
        if (def.z_mm - self.previous_z_mm).abs() >= SAME_HEIGHT_MM {
            totals.distinct_heights += 1;
            self.previous_z_mm = def.z_mm;
        }
    }
}
