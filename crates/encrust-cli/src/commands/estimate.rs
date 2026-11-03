use std::path::PathBuf;
use std::time::Instant;

use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::args::JobArgs;
use crate::batch::EstimateReport;
use crate::commands::GlobalArgs;
use crate::commands::slice::PlateArgs;
use crate::exit::{Exit, Stop};
use crate::{estimate, json, stage};

#[derive(clap::Args, Debug)]
pub struct EstimateCommand {
    #[command(flatten)]
    pub plate: PlateArgs,

    #[command(flatten)]
    pub job: JobArgs,

    /// Set this run beside a report `estimate --json` kept, under NAME, as `agent-lab` judges
    /// one preparation against another; the report's `compared` says how it stands. Repeat.
    #[arg(long, value_name = "NAME=FILE", value_parser = crate::score::parse_against)]
    pub against: Vec<(String, PathBuf)>,
}

impl EstimateCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        let started = Instant::now();
        let watch = global.watch(stop);
        let mut staged = stage::stage(
            &self.plate.inputs,
            self.plate.arrange,
            self.plate.plate,
            &self.job,
            &watch,
        )?;
        let estimate = estimate::estimate(&mut staged, &self.job.raster, &watch)?;
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
