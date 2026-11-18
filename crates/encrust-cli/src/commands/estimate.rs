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
}

impl EstimateCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        if let Some(path) = &self.recipe_out {
            write_recipe(&self.job, path)?;
        }
        let started = Instant::now();
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
        let estimate = estimate::estimate(&mut staged, &self.job.raster, self.quick_check, &watch)?;
        let report = EstimateReport::of(
            &self.plate.inputs,
            &staged.parts,
            &estimate,
            started.elapsed(),
        );
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
