use std::path::Path;

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_format::{ExposurePlan, PrintJob, Thumbnail};
use core_pipeline::{Observer, SlicedFormat, Writing};
use core_raster::RasterSettings;
use core_slicer::Sliced;
use format_chitu::CtbVersion;
use printer_profiles::{MaterialProfile, PrinterProfile};

use crate::raster_report::RasterReport;
use crate::slice_report::SliceReport;
use crate::slicing::Plan;

/// Which `.ctb` revision `--ctb-version` asked for.
///
/// Version 4 is the default because every machine that reads version 5 reads it too; see
/// `docs/formats/chitu.md`.
#[derive(Debug, Default, Clone, Copy, clap::ValueEnum)]
pub enum CtbRevision {
    #[default]
    #[value(name = "4")]
    Four,
    #[value(name = "5")]
    Five,
}

impl From<CtbRevision> for CtbVersion {
    fn from(revision: CtbRevision) -> Self {
        match revision {
            CtbRevision::Four => Self::V4,
            CtbRevision::Five => Self::V5,
        }
    }
}

/// The format `path` names, or `None` when it names a directory for a PNG stack.
pub fn format_of(path: &Path, revision: CtbRevision) -> Option<SlicedFormat> {
    SlicedFormat::of(path, revision.into())
}

/// Every window of the stack, counted into the slicing report as it goes past.
struct Absorbing<'a>(&'a mut SliceReport);

impl Observer for Absorbing<'_> {
    fn window(&mut self, sliced: &Sliced) {
        self.0.absorb(sliced);
    }
}

/// Slices and rasterises straight into a printable sliced file.
///
/// Nothing but the window being worked on is ever in memory; see
/// `core_pipeline::write` and ADR 0010.
#[allow(clippy::too_many_arguments)]
pub fn write_sliced(
    plan: &Plan,
    slice: &mut SliceReport,
    settings: &RasterSettings,
    path: &Path,
    window: usize,
    printer: &PrinterProfile,
    material: &MaterialProfile,
    exposure: ExposurePlan,
    format: SlicedFormat,
    thumbnail: Option<Thumbnail>,
    fold: Measured,
) -> Result<RasterReport> {
    let job = PrintJob {
        printer: printer.clone(),
        material: material.clone(),
        raster: *settings,
        plan: plan.layers().clone(),
        // What the stack comes to is only known once it has been cut, and the header is
        // written before the first layer; `finish` lays it down again. See ADR 0067.
        volume_mm3: 0.0,
        exposure,
        thumbnail,
    };
    warn_if_exposure_was_measured_elsewhere(&job, material);

    let written = core_pipeline::write(
        &Writing {
            format,
            path,
            job: &job,
            mesh: plan.mesh(),
            windows: plan.windows(),
            settings,
            window,
            fold,
        },
        &mut Absorbing(slice),
    )
    .with_context(|| format!("cannot write {}", path.display()))?
    .context("a command-line run is never cancelled")?;

    Ok(RasterReport::of_written(
        path.to_owned(),
        *settings,
        written,
    ))
}

/// Exposure follows the layer height, so a stack cut off the height the resin was
/// measured at is not printed at the seconds the resin profile states.
fn warn_if_exposure_was_measured_elsewhere(job: &PrintJob, material: &MaterialProfile) {
    let normal_s = job.header_exposure_s();
    if (normal_s - material.exposure_s).abs() > 1e-4 {
        tracing::warn!(
            "{} was measured at {:.3} mm for {:.2} s; at {:.3} mm a normal layer is \
             exposed for {:.2} s",
            material.name,
            material.layer_height_mm,
            material.exposure_s,
            job.nominal_height_mm(),
            normal_s,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_decides_what_is_written() {
        let of = |name: &str| format_of(Path::new(name), CtbRevision::default());
        assert!(matches!(of("cube.goo"), Some(SlicedFormat::Goo)));
        assert!(matches!(
            of("cube.ctb"),
            Some(SlicedFormat::Ctb(CtbVersion::V4))
        ));
        assert!(matches!(
            format_of(Path::new("cube.ctb"), CtbRevision::Five),
            Some(SlicedFormat::Ctb(CtbVersion::V5))
        ));
        assert!(
            of("out").is_none(),
            "a name without an extension is a PNG directory"
        );
    }
}
