//! The `.photons` of the Photon S: a big-endian header of doubles, one raw preview and a
//! one-bit run-length layer codec. Layout is in `docs/formats/photons.md`.

use core_format::{
    Fields, FormatError, LayerSink, PrintJob, SlicedFileWriter, WriteSeek, check_layer_pushed,
    check_layers_finished, validate,
};
use core_raster::LayerRuns;

use crate::blocks::panel_in_sixteen_bits;

mod codec;
mod header;
mod reader;
#[cfg(test)]
mod tests;

pub use codec::PhotonsLayer;
pub use reader::{OpenPhotons, PhotonsReader};

/// Writes the `.photons` container.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhotonsWriter;

impl SlicedFileWriter for PhotonsWriter {
    type Sink<'w> = PhotonsSink<'w>;

    fn extension(&self) -> &'static str {
        "photons"
    }

    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError> {
        validate(job)?;
        // A layer record states the panel in sixteen bits as well as in thirty-two.
        panel_in_sixteen_bits(job, "a layer record")?;

        let mut fields = Fields::new(sink);
        header::write_header(&mut fields, job)?;
        header::write_preview(&mut fields, job.thumbnail.as_ref())?;
        fields.u32_be(job.layer_count())?;

        Ok(PhotonsSink {
            fields,
            job: job.clone(),
            written: 0,
        })
    }
}

/// A `.photons` with everything in front of its layers written.
pub struct PhotonsSink<'w> {
    fields: Fields<'w>,
    job: PrintJob,
    written: u32,
}

impl LayerSink for PhotonsSink<'_> {
    type Encoded = PhotonsLayer;

    fn encode(layer: &LayerRuns) -> Self::Encoded {
        PhotonsLayer::encode(layer)
    }

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError> {
        let index = self.written;
        check_layer_pushed(&self.job, index, (encoded.width(), encoded.height()))?;
        encoded.write(&mut self.fields)?;
        self.written += 1;
        Ok(())
    }

    fn finish(mut self, volume_mm3: f32) -> Result<(), FormatError> {
        check_layers_finished(&self.job, self.written)?;
        self.job.volume_mm3 = volume_mm3;
        self.fields.seek_to(0)?;
        header::write_header(&mut self.fields, &self.job)?;
        self.fields.flush()?;
        Ok(())
    }
}
