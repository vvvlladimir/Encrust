//! The `.cxline` of the D158 and D160: the `.cxdlp` version 3 header and settings,
//! with each layer held as polygons rather than lines. Layout is in `docs/formats/cxline.md`.

use core_format::{
    Fields, FormatError, LayerSink, PrintJob, ReadWriteSeek, SlicedFileWriter, check_layer_pushed,
    check_layers_finished, validate,
};
use core_raster::{LayerRuns, Ring};

use crate::family::{block, block_at};
use crate::v3::blocks::{PAGE_BREAK, write_previews};
use core_format::Crc32;

mod blocks;
mod reader;
mod rings;
#[cfg(test)]
mod tests;

pub use blocks::cxline_model_code;
pub use reader::{CxlineReader, OpenCxline};

/// Writes the `.cxline`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CxlineWriter;

impl SlicedFileWriter for CxlineWriter {
    type Sink<'w> = CxlineSink<'w>;

    fn extension(&self) -> &'static str {
        "cxline"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn ReadWriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;
        let (layers, width, height) =
            (job.layer_count(), job.raster.width_px, job.raster.height_px);
        if layers > u32::from(u16::MAX)
            || width > u32::from(u16::MAX)
            || height > u32::from(u16::MAX)
        {
            return Err(FormatError::Encoding {
                what: "the header",
                reason: format!(
                    "{layers} layers of {width} x {height} px is past the 65535 its fields hold"
                ),
            });
        }

        // Built in memory, as for the `.cxdlp`, because the checksum covers it and the sink
        // cannot be read back; it is kept because the volume in it is known only at the end.
        let (head, volume_at) = block_at(0, |fields| {
            let volume_at = blocks::write_header(fields, job)?;
            write_previews(fields, job.thumbnail.as_ref())?;
            blocks::write_settings(fields, job)?;
            Ok(volume_at)
        })?;
        let corrections = block(blocks::write_corrections)?;

        let mut fields = Fields::new(sink);
        fields.bytes(&head)?;
        let area_table = fields.position()?;
        fields.zeros(4 * layers as usize)?;
        fields.bytes(&PAGE_BREAK)?;
        fields.bytes(&corrections)?;
        let after = Crc32::of(&corrections);

        Ok(CxlineSink {
            fields,
            job: job.clone(),
            head,
            volume_at: volume_at as usize,
            areas: Vec::with_capacity(layers as usize),
            area_table,
            after,
        })
    }
}

/// A `.cxline` with its header written and its layers still to come.
pub struct CxlineSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    /// Everything in front of the area table, written again once the volume is known.
    head: Vec<u8>,
    volume_at: usize,
    /// Each layer's area field, which the table in front of the layers repeats.
    areas: Vec<u32>,
    area_table: u64,
    /// The sum over everything behind the area table.
    after: Crc32,
}

impl LayerSink for CxlineSink<'_> {
    /// The panel the layer covers and its outline, traced off the main thread.
    type Encoded = (u32, u32, Vec<Ring>);

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        (layer.width(), layer.height(), core_raster::rings_of(layer))
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let (width, height, rings) = encoded;
        let index = self.areas.len() as u32;
        check_layer_pushed(&self.job, index, (width, height))?;

        let pitch = (self.job.raster.pitch.x, self.job.raster.pitch.y);
        let area = rings::area_field(&rings, pitch);
        self.areas.push(area);
        let body = rings::layer_bytes(area, &rings, pitch);
        self.after.update(&body);
        self.fields.bytes(&body)?;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        check_layers_finished(&self.job, self.areas.len() as u32)?;
        let footer = block(blocks::write_footer)?;
        self.after.update(&footer);
        self.fields.bytes(&footer)?;

        let mut table = Vec::with_capacity(4 * self.areas.len() + PAGE_BREAK.len());
        for area in &self.areas {
            table.extend_from_slice(&area.to_be_bytes());
        }
        table.extend_from_slice(&PAGE_BREAK);
        self.head[self.volume_at..self.volume_at + 4]
            .copy_from_slice(&(volume_mm3 / 1000.0).to_be_bytes());

        let end = self.fields.position()?;
        self.fields.seek_to(0)?;
        self.fields.bytes(&self.head)?;
        debug_assert_eq!(self.fields.position()?, self.area_table);
        self.fields.bytes(&table)?;
        self.fields.seek_to(end)?;

        let checksum = Crc32::of(&self.head)
            .followed_by(Crc32::of(&table))
            .followed_by(self.after)
            .value();
        self.fields.u32_be(checksum)?;
        self.fields.flush()?;
        Ok(())
    }
}
