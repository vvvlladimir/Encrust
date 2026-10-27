//! The `.fdg` container: the Chitu layer records and previews under a header of its own and
//! a one-bit layer codec. Layout is in `docs/formats/fdg.md`; `.phz` is this layout under
//! another header.

use std::io;

use core_format::{
    Fields, FormatError, LayerSink, PREVIEW_SIZES_PX, PrintJob, SlicedFileWriter, WriteSeek,
    validate, write_preview,
};
use core_raster::LayerRuns;

use crate::blocks::preview_pixels;
use crate::layer::{self, LAYER_DEF_BYTES, LAYER_DEF_EX_BYTES, LayerDef};

mod header;
mod reader;
mod rle;
#[cfg(test)]
mod tests;

pub(crate) use header::{HEADER_BYTES, Offsets, Totals, anti_alias_level, write_tail};
pub use reader::{FdgReader, OpenFdg};
pub(crate) use reader::{MotionAt, read_machine_name, read_motion, read_table};
pub use rle::FdgLayer;
pub(crate) use rle::{cover, decode};

/// Two heights closer than this are one height, as the header counts them.
const SAME_HEIGHT_MM: f32 = 0.001;

/// Writes the header of a container of this layout, with what only the end of the stack
/// tells; it is written over the reserved bytes once every layer is in.
pub(crate) type WriteHeader = fn(&mut Fields<'_>, &PrintJob, Offsets, Totals) -> io::Result<()>;

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
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        FdgSink::open(job, sink, header::write_header)
    }
}

/// An `.fdg` or `.phz` file with everything in front of its layers written.
pub struct FdgSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    write_header: WriteHeader,
    offsets: Offsets,
    totals: Totals,
    previous_z_mm: f32,
    defs: Vec<LayerDef>,
}

impl<'w> FdgSink<'w> {
    /// Writes the previews, the machine name and room for the header and the table.
    pub(crate) fn open(
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
        write_header: WriteHeader,
    ) -> Result<Self, FormatError> {
        validate(job)?;

        let mut fields = Fields::new(sink);
        fields.seek_to(HEADER_BYTES)?;
        let mut offsets = Offsets::default();
        let [(large_w, large_h), (small_w, small_h)] = PREVIEW_SIZES_PX;
        offsets.large_preview = write_preview(
            &mut fields,
            large_w,
            large_h,
            &preview_pixels(job, (large_w, large_h)),
        )?;
        offsets.small_preview = write_preview(
            &mut fields,
            small_w,
            small_h,
            &preview_pixels(job, (small_w, small_h)),
        )?;

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
        self.tally(index, &def);
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
        let rests_s: f32 = self.job.material.waits.rests_s().iter().sum();
        let totals = &mut self.totals;
        totals.layer_time_s += def.exposure_s + def.light_off_delay_s + rests_s;
        totals.largest_layer_bytes = totals.largest_layer_bytes.max(def.data_size);

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
