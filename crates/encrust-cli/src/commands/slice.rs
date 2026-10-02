use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, bail};

use crate::args::JobArgs;
use crate::batch::ModelReport;
use crate::commands::GlobalArgs;
use crate::exit::{Exit, Stop};
use crate::{json, pipeline, profiles};

#[derive(clap::Args, Debug)]
pub struct SliceCommand {
    /// Mesh to slice: `.stl`, `.obj` or `.3mf`.
    pub input: PathBuf,

    /// Where the sliced output goes. The extension picks the container; a name without
    /// one is a directory for a PNG stack.
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,

    #[command(flatten)]
    pub job: JobArgs,
}

impl SliceCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        if self.input.is_dir() {
            bail!(
                "{} is a directory; `encrust batch` slices a folder of models",
                self.input.display()
            );
        }
        let chosen = profiles::resolve(&self.job.profile.selection())?;
        let started = Instant::now();
        let outcome = pipeline::slice_one(
            &self.input,
            &self.output,
            &self.job,
            &chosen,
            &global.watch(stop),
        )?;
        if global.json {
            json::print(&ModelReport::sliced(&outcome, started.elapsed()))?;
        }
        Ok(if self.job.strict && !outcome.is_clean() {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}
