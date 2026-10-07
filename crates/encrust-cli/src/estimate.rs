//! What a plate would take to print — layers, time, resin, weight, price and the layers
//! likely to fail — worked out without writing anything.

use std::fmt;

use anyhow::{Context, Result};
use core_analysis::Measured;
use core_engine::{Run, bake, cut};
use core_format::PrintJob;

use crate::exit::Cancelled;
use crate::pipeline::{Watch, raster_window, report_of};
use crate::progress::{Watching, bar};
use crate::raster_report::write_risks;
use crate::slice_report::SliceReport;
use crate::stage::Staged;

/// The stack as cut, and with a printer to draw it for, the print it makes.
pub struct Estimate {
    pub slice: SliceReport,
    pub print: Option<Print>,
}

/// What the masks cure and what that costs in time, resin and money.
pub struct Print {
    /// The header the file would carry, its resin volume taken from the masks.
    pub job: PrintJob,
    pub measured: Measured,
}

impl Print {
    pub fn print_time_s(&self) -> u32 {
        self.job.print_time_s()
    }

    pub fn weight_g(&self) -> f32 {
        self.job.weight_g()
    }

    pub fn cost(&self) -> Option<f32> {
        self.job.cost()
    }
}

/// Cuts `staged`, and with a printer rasterises it too, writing nothing.
pub fn estimate(
    staged: &mut Staged,
    raster: &crate::args::RasterArgs,
    watch: &Watch,
) -> Result<Estimate> {
    let mut estimate = match staged.take_plate(None, raster_window(raster)) {
        Some(plate) => {
            let run = Run::of(&plate)?;
            drop(plate);
            let mut slice = report_of(run.mesh(), run.windows(), staged.drainage);
            let mut watching = Watching {
                report: &mut slice,
                bar: watch.progress.then(|| bar("layers")),
                stop: watch.stop,
            };
            let measured = run.measure(&mut watching)?.ok_or(Cancelled)?;
            drop(watching);
            let mut job = run.job().clone();
            job.volume_mm3 = measured.volume_mm3();
            Estimate {
                slice,
                print: Some(Print { job, measured }),
            }
        }
        None => {
            tracing::warn!("without a printer only the stack is counted: no time, weight or risks");
            let baked =
                bake(&staged.models, &staged.cutting.compensation).context("nothing to slice")?;
            let windows = cut(&baked, &staged.cutting)?;
            let mesh = &baked.mesh;
            let mut slice = report_of(mesh, &windows, staged.drainage);
            windows
                .stream(mesh, |sliced| {
                    watch.stop.check()?;
                    slice.absorb(sliced);
                    Ok::<(), anyhow::Error>(())
                })
                .context("cannot slice the model")?;
            Estimate { slice, print: None }
        }
    };
    estimate.slice.finish_drainage();
    Ok(estimate)
}

impl Estimate {
    /// Nothing found would stop the stack printing correctly.
    pub fn is_clean(&self) -> bool {
        self.slice.is_clean()
    }
}

impl fmt::Display for Estimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.slice)?;
        let Some(print) = &self.print else {
            return Ok(());
        };
        let job = &print.job;
        writeln!(f, "Estimate")?;
        writeln!(f, "  height        {:.3} mm", job.height_mm())?;
        writeln!(f, "  print time    {}", hours_minutes(print.print_time_s()))?;
        writeln!(
            f,
            "  resin         {:.1} ml, {:.1} g",
            job.volume_mm3 / 1000.0,
            print.weight_g()
        )?;
        if let Some(cost) = print.cost() {
            writeln!(
                f,
                "  cost          {cost:.2} {}",
                job.material.details.currency
            )?;
        }
        write_risks(f, &print.measured)?;
        writeln!(f)
    }
}

fn hours_minutes(seconds: u32) -> String {
    let minutes = seconds.div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, minutes) => format!("{hours} h {minutes:02} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_print_time_reads_in_hours_and_whole_minutes() {
        assert_eq!(hours_minutes(59), "1 min", "a started minute counts");
        assert_eq!(hours_minutes(3600), "1 h 00 min");
        assert_eq!(hours_minutes(5 * 3600 + 7 * 60), "5 h 07 min");
    }
}
