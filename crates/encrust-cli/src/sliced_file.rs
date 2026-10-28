use std::path::Path;

use anyhow::{Context, Result};
use core_engine::{Plate, Run};
use core_format::PrintJob;
use core_pipeline::CtbVersion;
use core_pipeline::SlicedFormat;
use printer_profiles::{LimitBreach, MaterialProfile, OutputFormat, PrinterProfile, Stray};

use crate::exit::Cancelled;
use crate::pipeline::{Watch, report_of};
use crate::progress::{Watching, bar};
use crate::raster_report::RasterReport;
use crate::slice_report::SliceReport;

/// Which `.ctb` revision `--ctb-version` asked for, over the one the machine's profile
/// names; see `docs/formats/chitu.md`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CtbRevision {
    #[value(name = "4")]
    Four,
    #[value(name = "5")]
    Five,
    #[value(name = "encrypted")]
    Encrypted,
}

impl CtbRevision {
    fn format(self) -> SlicedFormat {
        match self {
            Self::Four => SlicedFormat::Ctb(CtbVersion::V4),
            Self::Five => SlicedFormat::Ctb(CtbVersion::V5),
            Self::Encrypted => SlicedFormat::CtbEncrypted,
        }
    }
}

/// The format `path` names, or `None` when it names a directory for a PNG stack. A `.ctb`
/// is version 5 until the machine's profile or `--ctb-version` says otherwise.
pub fn format_of(path: &Path) -> Option<SlicedFormat> {
    SlicedFormat::of(path, CtbVersion::V5)
}

/// `format` at the `.ctb` revision `--ctb-version` asked for, where it asked and the
/// format is a `.ctb`.
pub fn asked_revision(format: SlicedFormat, revision: Option<CtbRevision>) -> SlicedFormat {
    match (format, revision) {
        (SlicedFormat::Ctb(_) | SlicedFormat::CtbEncrypted, Some(revision)) => revision.format(),
        _ => format,
    }
}

/// Slices and rasterises `plate` straight into a sliced file at `path`, one window in memory
/// (ADR 0010). A run `watch.stop` cancels leaves no file.
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
    let mut slice = report_of(run.solid(), run.windows(), drainage);
    warn_if_exposure_was_measured_elsewhere(run.job(), &plate.material);
    warn_of_limits(run.job());

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

/// Why the container is dear on this machine's panel, or `None` where the panel is usual for it.
/// A `.cbddlp` (ADR 0146) and an `.svgx` (ADR 0169) cost by panel, not model.
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

/// A setting past what the machine takes, or past what its maker advises, is written as
/// asked; the window would not have let the first be typed (ADR 0369).
pub(crate) fn warn_of_limits(job: &PrintJob) {
    for breach in job.limit_breaches() {
        tracing::warn!("{}", limit_warning(&job.printer.name, &breach));
    }
}

fn limit_warning(printer: &str, breach: &LimitBreach) -> String {
    let span = match breach.span {
        (Some(low), Some(high)) => format!("{low} to {high}"),
        (Some(low), None) => format!("at least {low}"),
        (None, Some(high)) => format!("at most {high}"),
        (None, None) => String::new(),
    };
    let whose = match breach.stray {
        Stray::Refused => "takes",
        Stray::Advised => "is advised",
    };
    format!(
        "{} is {}, and {printer} {whose} {span}",
        breach.field, breach.value
    )
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
        let of = |name: &str| format_of(Path::new(name));
        assert!(matches!(of("cube.goo"), Some(SlicedFormat::Goo)));
        assert!(matches!(
            of("cube.ctb"),
            Some(SlicedFormat::Ctb(CtbVersion::V5))
        ));
        assert!(
            of("out").is_none(),
            "a name without an extension is a PNG directory"
        );
    }

    #[test]
    fn an_asked_revision_holds_over_the_machines_own_and_touches_no_other_format() {
        let machines = SlicedFormat::CtbEncrypted;
        assert!(matches!(
            asked_revision(machines, Some(CtbRevision::Four)),
            SlicedFormat::Ctb(CtbVersion::V4)
        ));
        assert!(matches!(
            asked_revision(machines, None),
            SlicedFormat::CtbEncrypted
        ));
        assert!(matches!(
            asked_revision(SlicedFormat::Goo, Some(CtbRevision::Five)),
            SlicedFormat::Goo
        ));
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
