//! A directory of models, sliced against one profile, with a report per model.
//!
//! One model is one job: it is oriented, hollowed, supported and cut on its own, and
//! comes out as its own file and its own JSON beside it. Packing several models onto one
//! plate is what the window is for; a farm wants one file per part.

mod report;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_pipeline::SlicedFormat;
use rayon::prelude::*;

use crate::args::JobArgs;
use crate::exit::{Stop, is_cancelled};
use crate::pipeline::{self, Watch};
use crate::profiles::Chosen;
use crate::progress::bar;

pub use report::{EstimateReport, ModelReport, PlateReport, Summary};

/// The mesh extensions a batch run picks up. A directory holds anything; only these are
/// models, and anything else in it is left alone rather than failed on.
const MODELS: [&str; 3] = ["stl", "obj", "3mf"];

/// Where a batch reads its models from and writes their files to, and how many it cuts
/// at once.
pub struct Batch<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub job: &'a JobArgs,
    pub jobs: usize,
}

/// Slices every model in `batch.input` into `batch.output`, which is made if it is not
/// there, and writes `batch.json` over the lot. A batch Ctrl-C stopped writes no summary.
pub fn run(batch: &Batch, chosen: &Chosen, watch: &Watch) -> Result<Summary> {
    let models = models_in(batch.input)?;
    if models.is_empty() {
        bail!("no models in {}", batch.input.display());
    }
    let out_dir = batch.output;
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("cannot make the output directory {}", out_dir.display()))?;

    if watch.talk {
        println!(
            "Batch: {} model(s) from {} into {}\n",
            models.len(),
            batch.input.display(),
            out_dir.display()
        );
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(batch.jobs.max(1))
        .build()
        .context("cannot start the batch thread pool")?;
    let progress = watch.progress.then(|| bar("models"));
    if let Some(progress) = &progress {
        progress.set_length(models.len() as u64);
    }
    let reports: Vec<Option<ModelReport>> = pool.install(|| {
        models
            .par_iter()
            .map(|model| {
                let report = one(model, batch, chosen, watch.stop);
                if let Some(progress) = &progress {
                    progress.inc(1);
                }
                report
            })
            .collect()
    });
    if let Some(progress) = progress {
        progress.finish_and_clear();
    }
    watch.stop.check()?;
    let reports: Vec<ModelReport> = reports.into_iter().flatten().collect();

    if watch.talk {
        for report in &reports {
            println!("{}", report.line());
        }
    }

    let summary = Summary::of(batch.input, out_dir, reports);
    let path = out_dir.join("batch.json");
    report::write(&path, &summary)
        .with_context(|| format!("cannot write the batch report {}", path.display()))?;
    if watch.talk {
        println!("\n{}", summary.line(&path));
    }
    Ok(summary)
}

/// One model, and its report written beside its output. A model that fails does not stop
/// the run: an overnight batch reports the bad one in the morning rather than at 2 a.m.
/// `None` once Ctrl-C is pressed: a model not started is skipped, and one stopped is not a
/// failure to report.
fn one(model: &Path, batch: &Batch, chosen: &Chosen, stop: &Stop) -> Option<ModelReport> {
    if stop.requested() {
        return None;
    }
    let output = output_for(model, batch.output, chosen);
    let started = std::time::Instant::now();
    let quiet = Watch {
        talk: false,
        progress: false,
        stop,
    };
    let report = match pipeline::slice_one(model, &output, batch.job, chosen, &quiet) {
        Ok(outcome) => ModelReport::sliced(&outcome, started.elapsed()),
        Err(error) if is_cancelled(&error) => return None,
        Err(error) => ModelReport::failed(model, &output, &error, started.elapsed()),
    };

    let beside = batch.output.join(format!(
        "{}.json",
        model.file_stem().unwrap_or_default().to_string_lossy()
    ));
    if let Err(error) = report::write(&beside, &report) {
        tracing::warn!("cannot write {}: {error:#}", beside.display());
    }
    Some(report)
}

/// Where one model's output goes: its own name in the output directory, with the
/// extension the printer's firmware reads. A run with no printer writes a PNG stack, and
/// a stack is a directory rather than a file.
fn output_for(model: &Path, out_dir: &Path, chosen: &Chosen) -> PathBuf {
    let stem = model.file_stem().unwrap_or_default();
    let path = out_dir.join(stem);
    match chosen.printer.as_ref().map(|printer| printer.output) {
        Some(format) => path.with_extension(SlicedFormat::from(format).extension()),
        None => path,
    }
}

/// Every model in `input`, sorted by name so two runs over one directory report in the
/// same order. Subdirectories are not walked: a batch is a folder of parts.
fn models_in(input: &Path) -> Result<Vec<PathBuf>> {
    let mut models: Vec<PathBuf> = std::fs::read_dir(input)
        .with_context(|| format!("cannot read {}", input.display()))?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| MODELS.iter().any(|known| e.eq_ignore_ascii_case(known)))
        })
        .collect();
    models.sort();
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_models_are_picked_up_and_they_come_out_sorted() {
        let dir = std::env::temp_dir().join("encrust-batch-listing");
        std::fs::create_dir_all(&dir).expect("the temporary directory is writable");
        for name in [
            "b.stl",
            "a.STL",
            "c.obj",
            "d.3mf",
            "notes.txt",
            "batch.json",
        ] {
            std::fs::write(dir.join(name), b"").expect("the temporary directory is writable");
        }

        let models = models_in(&dir).expect("the directory reads");
        let names: Vec<&str> = models
            .iter()
            .filter_map(|path| path.file_name()?.to_str())
            .collect();
        assert_eq!(
            names,
            ["a.STL", "b.stl", "c.obj", "d.3mf"],
            "case is not what makes a model"
        );

        std::fs::remove_dir_all(&dir).expect("the directory was just made");
    }

    #[test]
    fn a_directory_with_nothing_in_it_is_an_error_rather_than_an_empty_run() {
        let dir = std::env::temp_dir().join("encrust-batch-empty");
        std::fs::create_dir_all(&dir).expect("the temporary directory is writable");

        let cli = <crate::Cli as clap::Parser>::parse_from([
            "encrust",
            "batch",
            dir.to_str().expect("ascii path"),
        ]);
        let crate::Command::Batch(command) = &cli.command else {
            unreachable!("the batch subcommand was parsed");
        };
        let chosen = crate::profiles::Chosen {
            printer: None,
            material: printer_profiles::MaterialProfile::default(),
        };
        let watch = Watch {
            talk: false,
            progress: false,
            stop: &Stop::default(),
        };
        let error = run(&command.batch(), &chosen, &watch).expect_err("there is nothing to slice");
        assert!(error.to_string().contains("no models"));

        std::fs::remove_dir_all(&dir).expect("the directory was just made");
    }

    #[test]
    fn a_batch_stopped_by_ctrl_c_starts_no_model_and_reports_none() {
        let dir = std::env::temp_dir().join("encrust-batch-stopped");
        let out = dir.join("out");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the temporary directory is writable");
        for name in ["a.stl", "b.stl"] {
            std::fs::write(dir.join(name), b"").expect("the temporary directory is writable");
        }

        let cli = <crate::Cli as clap::Parser>::parse_from([
            "encrust",
            "batch",
            dir.to_str().expect("ascii path"),
            "-o",
            out.to_str().expect("ascii path"),
        ]);
        let crate::Command::Batch(command) = &cli.command else {
            unreachable!("the batch subcommand was parsed");
        };
        let chosen = crate::profiles::Chosen {
            printer: None,
            material: printer_profiles::MaterialProfile::default(),
        };
        let stop = Stop::default();
        stop.request();
        let watch = Watch {
            talk: false,
            progress: false,
            stop: &stop,
        };
        let error = run(&command.batch(), &chosen, &watch).expect_err("the batch was stopped");
        assert!(is_cancelled(&error), "got {error:#}");
        assert!(
            !out.join("a.json").exists() && !out.join("b.json").exists(),
            "a model Ctrl-C kept from starting is not reported as failed"
        );

        std::fs::remove_dir_all(&dir).expect("the directory was just made");
    }
}
