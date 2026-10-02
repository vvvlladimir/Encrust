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

use crate::Args;
use crate::profiles::Chosen;
use crate::{pipeline, report::ImportReport};

pub use report::{ModelReport, Summary};

/// The mesh extensions a batch run picks up. A directory holds anything; only these are
/// models, and anything else in it is left alone rather than failed on.
const MODELS: [&str; 3] = ["stl", "obj", "3mf"];

/// Slices every model in `input` into `args.output`, which is made if it is not there.
///
/// Returns false when `--strict` was given and any model came out unclean, so a farm can
/// gate on the exit code.
pub fn run(input: &Path, args: &Args, chosen: &Chosen) -> Result<bool> {
    let models = models_in(input)?;
    if models.is_empty() {
        bail!("no models in {}", input.display());
    }
    let out_dir = &args.output;
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("cannot make the output directory {}", out_dir.display()))?;

    println!(
        "Batch: {} model(s) from {} into {}\n",
        models.len(),
        input.display(),
        out_dir.display()
    );

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs.max(1))
        .build()
        .context("cannot start the batch thread pool")?;
    let reports: Vec<ModelReport> = pool.install(|| {
        models
            .par_iter()
            .map(|model| one(model, args, chosen))
            .collect()
    });

    for report in &reports {
        println!("{}", report.line());
    }

    let summary = Summary::of(input, out_dir, reports);
    let path = out_dir.join("batch.json");
    report::write(&path, &summary)
        .with_context(|| format!("cannot write the batch report {}", path.display()))?;
    println!("\n{}", summary.line(&path));

    Ok(!args.strict || summary.failed == 0 && summary.unclean == 0)
}

/// One model, and its report written beside its output. A model that fails does not stop
/// the run: an overnight batch reports the bad one in the morning rather than at 2 a.m.
fn one(model: &Path, args: &Args, chosen: &Chosen) -> ModelReport {
    let output = output_for(model, args, chosen);
    let started = std::time::Instant::now();
    let outcome = if args.no_slice {
        pipeline::inspect(model, args, chosen).map(Err::<pipeline::Outcome, ImportReport>)
    } else {
        pipeline::slice_one(model, &output, args, chosen, false).map(Ok)
    };

    let report = match outcome {
        Ok(Ok(outcome)) => ModelReport::sliced(&outcome, started.elapsed()),
        Ok(Err(import)) => ModelReport::inspected(&import, &output, started.elapsed()),
        Err(error) => ModelReport::failed(model, &output, &error, started.elapsed()),
    };

    let beside = args.output.join(format!(
        "{}.json",
        model.file_stem().unwrap_or_default().to_string_lossy()
    ));
    if let Err(error) = report::write(&beside, &report) {
        tracing::warn!("cannot write {}: {error:#}", beside.display());
    }
    report
}

/// Where one model's output goes: its own name in the output directory, with the
/// extension the printer's firmware reads. A run with no printer writes a PNG stack, and
/// a stack is a directory rather than a file.
fn output_for(model: &Path, args: &Args, chosen: &Chosen) -> PathBuf {
    let stem = model.file_stem().unwrap_or_default();
    let path = args.output.join(stem);
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

        let args = <Args as clap::Parser>::parse_from(["slice", dir.to_str().expect("ascii path")]);
        let chosen = crate::profiles::Chosen {
            printer: None,
            material: printer_profiles::MaterialProfile::default(),
        };
        let error = run(&dir, &args, &chosen).expect_err("there is nothing to slice");
        assert!(error.to_string().contains("no models"));

        std::fs::remove_dir_all(&dir).expect("the directory was just made");
    }
}
