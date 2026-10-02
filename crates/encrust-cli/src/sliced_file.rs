use std::path::Path;

use anyhow::{Context, Result};
use core_engine::{Cutting, Model, Plate, Run};
use core_format::{ExposurePlan, PrintJob};
use core_pipeline::{Observer, SlicedFormat};
use core_slicer::Sliced;
use format_chitu::CtbVersion;
use printer_profiles::{MaterialProfile, OutputFormat, PrinterProfile};

use crate::Args;
use crate::pipeline::{overrides_of, raster_window, report_of};
use crate::raster_report::RasterReport;
use crate::slice_report::SliceReport;

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

/// Slices and rasterises `models` straight into a printable sliced file.
///
/// Nothing but the window being worked on is ever in memory; see `core_engine::Run` and
/// ADR 0010.
#[allow(clippy::too_many_arguments)]
pub fn write_sliced(
    models: &[Model],
    cutting: &Cutting,
    args: &Args,
    path: &Path,
    printer: &PrinterProfile,
    material: &MaterialProfile,
    format: SlicedFormat,
) -> Result<(SliceReport, RasterReport)> {
    let format = format.at_revision_of(printer.output);
    warn_if_the_machine_reads_another_container(printer, format);

    let run = Run::of(&Plate {
        models: models.to_vec(),
        printer: printer.clone(),
        material: material.clone(),
        panel: overrides_of(args),
        cutting: *cutting,
        exposure: ExposurePlan::new(args.exposure_at.clone()),
        remove_islands: args.remove_islands,
        format,
        raster_window: raster_window(args),
    })?;

    let mut slice = report_of(run.mesh(), run.windows(), args);
    warn_if_exposure_was_measured_elsewhere(run.job(), material);

    let written = run
        .write_file(path, &mut Absorbing(&mut slice))
        .with_context(|| format!("cannot write {}", path.display()))?
        .context("a command-line run is never cancelled")?;

    let raster = RasterReport::of_written(path.to_owned(), *run.panel(), written);
    Ok((slice, raster))
}

/// A file in a container the chosen machine does not read is written anyway — the output
/// name has the last word (ADR 0047) — but the run says so.
fn warn_if_the_machine_reads_another_container(printer: &PrinterProfile, format: SlicedFormat) {
    let family = OutputFormat::from(format);
    if family != printer.output {
        tracing::warn!(
            "{} reads {}, and this is a {} file",
            printer.name,
            printer.output.label(),
            family.label()
        );
    }
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
