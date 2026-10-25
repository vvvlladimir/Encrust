use core_raster::LayerRuns;

use crate::{FormatError, PrintJob, WriteSeek};

/// Bytes to hold before they go to the file. A layer is tens of kilobytes; the default 8 KB
/// `BufWriter` makes tens of thousands of writes, 1 MB a few per layer.
pub const WRITE_BUFFER_BYTES: usize = 1 << 20;

/// Opens a sliced file and hands back the sink its layers go into, one at a time.
/// Streaming and a seekable sink: `docs/decisions/0012`, `docs/decisions/0045`.
pub trait SlicedFileWriter {
    /// What this writer's layers go into.
    type Sink<'w>: LayerSink
    where
        Self: 'w;

    /// Extension of the produced file, without the dot.
    fn extension(&self) -> &'static str;

    /// Writes the header and returns the sink for exactly `job.layer_count` layers.
    fn begin<'w>(
        &self,
        job: &PrintJob,
        sink: &'w mut dyn WriteSeek,
    ) -> Result<Self::Sink<'w>, FormatError>;
}

/// Takes rasterised layers in print order and closes the file when the last one lands.
pub trait LayerSink {
    /// One layer already compressed. Producing it is the expensive part and carries no
    /// borrow of the sink, so it can be done on another thread.
    type Encoded: Send;

    fn encode(layer: &LayerRuns) -> Self::Encoded
    where
        Self: Sized;

    fn push(&mut self, encoded: Self::Encoded) -> Result<(), FormatError>;

    /// Writes whatever closes the file, with the resin volume the stack came to. The header is
    /// rewritten with the volume (ADR 0067); fails when layers are missing.
    fn finish(self, volume_mm3: f32) -> Result<(), FormatError>
    where
        Self: Sized;
}

/// Rejects a job no format can write: no layers, masks that are not the panel, or an
/// exposure by height on a machine that reads the header alone.
pub fn validate(job: &PrintJob) -> Result<(), FormatError> {
    if job.layer_count() == 0 {
        return Err(FormatError::EmptyJob);
    }
    if !job.exposure.is_empty() {
        if !job.printer.firmware.per_layer_settings {
            return Err(FormatError::PerLayerUnsupported {
                printer: job.printer.name.clone(),
            });
        }
        if let Some(range) = job
            .exposure
            .ranges()
            .iter()
            .find(|range| range.exposure_s <= 0.0)
        {
            return Err(FormatError::NonPositiveExposure {
                from_mm: range.from_mm,
                exposure_s: range.exposure_s,
            });
        }
    }
    if !job.is_uniform() && !job.printer.firmware.variable_layer_height {
        return Err(FormatError::VariableHeightUnsupported {
            printer: job.printer.name.clone(),
        });
    }
    let display = &job.printer.display;
    if display.width_px != job.raster.width_px || display.height_px != job.raster.height_px {
        return Err(FormatError::PanelMismatch {
            printer_width: display.width_px,
            printer_height: display.height_px,
            raster_width: job.raster.width_px,
            raster_height: job.raster.height_px,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::sample_job;
    use crate::{ExposurePlan, ExposureRange, LayerPlan};

    #[test]
    fn a_job_without_bands_validates() {
        assert!(validate(&sample_job(4)).is_ok());
    }

    #[test]
    fn a_job_with_no_layers_is_rejected() {
        assert!(matches!(
            validate(&sample_job(0)).unwrap_err(),
            FormatError::EmptyJob
        ));
    }

    #[test]
    fn bands_are_refused_on_a_machine_that_reads_the_header_alone() {
        let mut job = sample_job(4);
        job.printer.firmware.per_layer_settings = false;
        job.exposure = ExposurePlan::new(vec![ExposureRange::new(0.0, 1.0, 3.0)]);

        assert!(matches!(
            validate(&job).unwrap_err(),
            FormatError::PerLayerUnsupported { .. }
        ));
    }

    #[test]
    fn a_band_asking_for_no_exposure_is_rejected() {
        let mut job = sample_job(4);
        job.exposure = ExposurePlan::new(vec![ExposureRange::new(2.0, 3.0, 0.0)]);

        assert!(matches!(
            validate(&job).unwrap_err(),
            FormatError::NonPositiveExposure { from_mm, .. } if (from_mm - 2.0).abs() < 1e-6
        ));
    }

    #[test]
    fn a_stack_of_mixed_thicknesses_is_refused_on_a_machine_that_steps_by_the_header() {
        let mut job = sample_job(3);
        job.plan = LayerPlan::from_bounds(vec![0.0, 0.05, 0.15, 0.2], 0.2);

        assert!(matches!(
            validate(&job).unwrap_err(),
            FormatError::VariableHeightUnsupported { .. }
        ));

        job.printer.firmware.variable_layer_height = true;
        assert!(validate(&job).is_ok());
    }

    #[test]
    fn masks_that_are_not_the_panel_are_rejected() {
        let mut job = sample_job(4);
        job.raster.width_px = 16;

        assert!(matches!(
            validate(&job).unwrap_err(),
            FormatError::PanelMismatch { .. }
        ));
    }
}
