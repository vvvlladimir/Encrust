//! A directory of models, sliced against one profile, one job, file and JSON report per model.

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
    /// What each file is named after; see `core_engine::file_stem`.
    pub name: &'a str,
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
    let outputs = outputs_for(&models, batch, chosen)?;
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
            .zip(&outputs)
            .map(|(model, output)| {
                let report = one(model, output, batch, chosen, watch.stop);
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

/// One model, and its report written beside its output; a failing model does not stop the run.
/// `None` once Ctrl-C is pressed: a model not started is skipped, one stopped is not a failure.
fn one(
    model: &Path,
    output: &Path,
    batch: &Batch,
    chosen: &Chosen,
    stop: &Stop,
) -> Option<ModelReport> {
    if stop.requested() {
        return None;
    }
    let started = std::time::Instant::now();
    let quiet = Watch {
        talk: false,
        progress: false,
        stop,
    };
    let report = match pipeline::slice_one(model, output, batch.job, chosen, &quiet) {
        Ok(outcome) => ModelReport::sliced(&outcome, started.elapsed()),
        Err(error) if is_cancelled(&error) => return None,
        Err(error) => ModelReport::failed(model, output, &error, started.elapsed()),
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

/// Where each model's output goes: `batch.name` made of it in the output directory, with the
/// printer's extension, refused when two models would land on one file.
fn outputs_for(models: &[PathBuf], batch: &Batch, chosen: &Chosen) -> Result<Vec<PathBuf>> {
    for tag in core_engine::unknown_tags(batch.name) {
        tracing::warn!("--name: {{{tag}}} stands for nothing and is kept as written");
    }
    let when = jiff::Zoned::now().datetime();
    let outputs: Vec<PathBuf> = models
        .iter()
        .map(|model| output_for(model, batch, chosen, when))
        .collect();
    let mut seen = std::collections::HashSet::with_capacity(outputs.len());
    if let Some(twice) = outputs.iter().find(|output| !seen.insert(*output)) {
        bail!(
            "--name {} gives two models the one file {}; put {{model}} in it",
            batch.name,
            twice.display()
        );
    }
    Ok(outputs)
}

/// A run with no printer writes a PNG stack, which is a directory, so it gets no extension.
fn output_for(
    model: &Path,
    batch: &Batch,
    chosen: &Chosen,
    when: jiff::civil::DateTime,
) -> PathBuf {
    let printer = chosen.printer.as_ref();
    let fields = core_engine::NameFields {
        model: &model.file_stem().unwrap_or_default().to_string_lossy(),
        model_ext: &model.extension().unwrap_or_default().to_string_lossy(),
        printer: printer.map_or("", |printer| &printer.name),
        resin: &chosen.material.name,
        layer_height_mm: batch
            .job
            .slicing
            .layer_height
            .unwrap_or(chosen.material.layer_height_mm),
        plate: 1,
        when,
    };
    let stem = core_engine::file_stem(batch.name, &fields);
    // Not `with_extension`, which would cut a stem holding a dot at it.
    batch
        .output
        .join(match printer.map(|printer| printer.output) {
            Some(format) => format!("{stem}.{}", SlicedFormat::from(format).extension()),
            None => stem,
        })
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
mod tests;
