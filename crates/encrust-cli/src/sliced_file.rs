use std::path::Path;

use anyhow::{Context, Result};
use core_engine::{Plate, Run};
use core_format::PrintJob;
use core_pipeline::CtbVersion;
use core_pipeline::SlicedFormat;
use printer_profiles::{MaterialProfile, OutputFormat, PrinterProfile};

use crate::exit::Cancelled;
use crate::pipeline::{Watch, report_of};
use crate::progress::{Watching, bar};
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

/// Slices and rasterises `plate` straight into a printable sliced file at `path`.
///
/// Nothing but the window being worked on is ever in memory; see `core_engine::Run` and
/// ADR 0010. A run `watch.stop` cancels leaves no file behind.
pub fn write_plate(
    plate: Plate,
    path: &Path,
    drainage: bool,
    watch: &Watch,
) -> Result<(SliceReport, RasterReport)> {
    warn_if_the_machine_reads_another_container(&plate.printer, plate.format);
    if let Some(cost) = panel_too_big_for(plate.format, &plate.printer) {
        tracing::warn!("{cost}");
    }
    let run = Run::of(&plate)?;
    // The run holds the baked plate; the models it was baked from are not needed again.
    drop(plate.models);
    let mut slice = report_of(run.mesh(), run.windows(), drainage);
    warn_if_exposure_was_measured_elsewhere(run.job(), &plate.material);

    let mut watching = Watching {
        report: &mut slice,
        bar: watch.progress.then(|| bar("layers")),
        stop: watch.stop,
    };
    let written = run
        .write_file(path, &mut watching)
        .with_context(|| format!("cannot write {}", path.display()))?;
    drop(watching);
    let written = written.ok_or(Cancelled)?;

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

/// Why the container the output names is dear on this machine's panel, or `None` where the
/// panel is one its machines have.
///
/// A `.cbddlp` carries grey in eight one-bit passes over the whole panel (ADR 0146) and an
/// `.svgx` traces every lit run into polygons (ADR 0169): both cost by the panel and not by
/// the model, so on a panel ten times the one they were made for a plate takes minutes and
/// gigabytes.
fn panel_too_big_for(format: SlicedFormat, printer: &PrinterProfile) -> Option<String> {
    let made_for: (u32, u32) = match format {
        // The Photon, at 1440 x 2560, is the largest panel of the machines reading it.
        SlicedFormat::Cbddlp(_) => (1440, 2560),
        // The Foto 8.9S, at 3840 x 2400.
        SlicedFormat::Svgx => (3840, 2400),
        _ => return None,
    };
    let display = &printer.display;
    let panel = u64::from(display.width_px) * u64::from(display.height_px);
    if panel <= u64::from(made_for.0) * u64::from(made_for.1) {
        return None;
    }
    Some(format!(
        "no machine reading a {} has a panel over {} x {} px and {} has {} x {} px, so \
         this file costs minutes and gigabytes where the {} it reads does not",
        OutputFormat::from(format).label(),
        made_for.0,
        made_for.1,
        printer.name,
        display.width_px,
        display.height_px,
        printer.output.label()
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

/// The clock the file is stamped with. One that reads before the epoch stamps the epoch:
/// the field is informational and no printer refuses a file over it.
pub fn now_unix_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use core_pipeline::CbddlpFlavour;
    use printer_profiles::Display;

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

    fn panel(width_px: u32, height_px: u32) -> PrinterProfile {
        let blank = PrinterProfile::default();
        PrinterProfile {
            name: "Test".to_owned(),
            display: Display {
                width_px,
                height_px,
                ..blank.display
            },
            ..blank
        }
    }

    #[test]
    fn a_container_made_for_a_small_panel_is_named_as_dear_on_a_large_one() {
        let big = panel(11520, 5120);
        let cost = panel_too_big_for(SlicedFormat::Cbddlp(CbddlpFlavour::Photon), &big)
            .expect("a 59 megapixel panel is far past the 1440 x 2560 its machines have");
        assert!(cost.contains("1440 x 2560 px"), "{cost}");
        assert!(cost.contains("11520 x 5120 px"), "{cost}");
        assert!(panel_too_big_for(SlicedFormat::Svgx, &big).is_some());

        assert!(
            panel_too_big_for(SlicedFormat::Goo, &big).is_none(),
            "the container every one of those machines reads costs by the model"
        );
        assert!(
            panel_too_big_for(
                SlicedFormat::Cbddlp(CbddlpFlavour::Cbddlp),
                &panel(1440, 2560)
            )
            .is_none(),
            "the panel it was made for is no warning"
        );
    }
}
