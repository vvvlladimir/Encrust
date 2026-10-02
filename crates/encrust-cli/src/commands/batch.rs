use std::path::PathBuf;

use anyhow::Result;

use crate::args::JobArgs;
use crate::batch::{self, Batch};
use crate::commands::GlobalArgs;
use crate::exit::{Exit, Stop};
use crate::{json, profiles};

#[derive(clap::Args, Debug)]
pub struct BatchCommand {
    /// Directory of models. Only `.stl`, `.obj` and `.3mf` files directly in it are read.
    pub input: PathBuf,

    /// Directory the files and reports go to, made if it is not there.
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,

    /// Models cut at once. Each one already uses every core, so more of them at once buys
    /// throughput on small models and costs peak memory.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub jobs: usize,

    #[command(flatten)]
    pub job: JobArgs,
}

impl BatchCommand {
    pub fn batch(&self) -> Batch<'_> {
        Batch {
            input: &self.input,
            output: &self.output,
            job: &self.job,
            jobs: self.jobs,
        }
    }

    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        let chosen = profiles::resolve(&self.job.profile.selection())?;
        let summary = batch::run(&self.batch(), &chosen, &global.watch(stop))?;
        if global.json {
            json::print(&summary)?;
        }
        Ok(if summary.failed > 0 {
            Exit::PartlyFailed
        } else if self.job.strict && summary.unclean > 0 {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}
