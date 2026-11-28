use std::path::{Path, PathBuf};
use std::time::Instant;

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::args::JobArgs;
use crate::batch::EstimateReport;
use crate::commands::GlobalArgs;
use crate::commands::slice::PlateArgs;
use crate::exit::{Exit, Stop};
use crate::pipeline::Watch;
use crate::{estimate, json, stage};

#[derive(clap::Args, Debug)]
pub struct EstimateCommand {
    #[command(flatten)]
    pub plate: PlateArgs,

    #[command(flatten)]
    pub job: JobArgs,

    /// Read the panel in squares of two by two pixels, as the window's background check does:
    /// quicker, but blind to islands near the smallest one named.
    #[arg(long)]
    pub quick_check: bool,
    /// Set this run beside a report `estimate --json` kept, under NAME, as `agent-lab` judges
    /// one preparation against another; the report's `compared` says how it stands. Repeat.
    #[arg(long, value_name = "NAME=FILE", value_parser = crate::score::parse_against)]
    pub against: Vec<(String, PathBuf)>,
    /// Write the recipe this run takes each model through to FILE, the flags typed written
    /// into its steps, for the window to run the same steps (`--prepare`, `--recipe` or none).
    #[arg(long, value_name = "FILE")]
    pub recipe_out: Option<PathBuf>,
    /// Lay the plate and write `--project-out`, but cut and measure nothing: the report lists
    /// the models alone, for a run whose plate the next run takes on.
    #[arg(long)]
    pub stage_only: bool,
}

impl EstimateCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        if let Some(path) = &self.recipe_out {
            write_recipe(&self.job, path)?;
        }
        let started = Instant::now();
        crate::spent::take();
        let watch = Watch {
            writes: false,
            ..global.watch(stop)
        };
        let mut staged = stage::stage(
            &self.plate.inputs,
            self.plate.arrange,
            self.plate.plate,
            &self.job,
            &watch,
        )?;
        if self.stage_only {
            return Self::staged_only(&staged, (started, self.job.strict), global);
        }
        let estimate = estimate::estimate(&mut staged, &self.job.raster, self.quick_check, &watch)?;
        let mut report = EstimateReport::of(
            &self.plate.inputs,
            &staged.parts,
            &estimate,
            started.elapsed(),
        );
        report.stages_s = crate::spent::take();
        let compared = if self.against.is_empty() {
            BTreeMap::new()
        } else {
            let mut value = serde_json::to_value(&report)?;
            value["schema"] = json::SCHEMA.into();
            crate::score::compared(&value, &self.against)
        };
        if global.json {
            json::print(&Compared {
                report: &report,
                compared,
            })?;
        } else if global.talks() {
            print!("{estimate}");
            for (name, judged) in &compared {
                println!("Against {name}: {judged}");
            }
        }
        let clean = staged.parts.iter().all(crate::stage::Part::is_clean) && estimate.is_clean();
        Ok(if self.job.strict && !clean {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}

impl EstimateCommand {
    /// Answers a run asked to lay the plate alone with its models, as their parts found them.
    fn staged_only(
        staged: &stage::Staged,
        (started, strict): (Instant, bool),
        global: &GlobalArgs,
    ) -> Result<Exit> {
        let clean = staged.parts.iter().all(crate::stage::Part::is_clean);
        let report = StagedReport {
            status: crate::batch::status_of(clean),
            seconds: started.elapsed().as_secs_f64(),
            stages_s: crate::spent::take(),
            models: staged
                .parts
                .iter()
                .map(crate::batch::PartReport::of)
                .collect(),
        };
        if global.json {
            json::print(&report)?;
        }
        Ok(if strict && !clean {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}

/// What `--stage-only` answers: the models laid, and what laying them took.
#[derive(Serialize)]
struct StagedReport {
    status: crate::batch::Status,
    seconds: f64,
    stages_s: BTreeMap<String, f64>,
    models: Vec<crate::batch::PartReport>,
}

/// The report, and how it stands against each kept one `--against` named.
#[derive(Serialize)]
struct Compared<'a> {
    #[serde(flatten)]
    report: &'a EstimateReport,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    compared: BTreeMap<String, Value>,
}

/// The recipe `job` takes a model through, its flags folded in, written to `path`.
fn write_recipe(job: &JobArgs, path: &Path) -> Result<()> {
    let asked = crate::recipe::Asked::of(job)?;
    let shaping = stage::Shaping::of(job)?;
    let recipe = crate::recipe::folded(asked.as_ref().map(|asked| &asked.plan), &shaping);
    std::fs::write(path, recipe.to_toml())
        .with_context(|| format!("cannot write the recipe to {}", path.display()))
}
