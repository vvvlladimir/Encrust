use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, bail};

use crate::args::JobArgs;
use crate::batch::{ModelReport, PlateReport};
use crate::commands::GlobalArgs;
use crate::exit::{Exit, Stop};
use crate::pipeline::{Outcome, slice_staged};
use crate::{json, stage};

#[derive(clap::Args, Debug)]
pub struct SliceCommand {
    #[command(flatten)]
    pub plate: PlateArgs,

    /// Where the sliced output goes. The extension picks the container; a name without
    /// one is a directory for a PNG stack.
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,

    #[command(flatten)]
    pub job: JobArgs,
}

/// What goes on the plate: models, or one file that describes a whole plate.
#[derive(clap::Args, Debug)]
pub struct PlateArgs {
    /// Meshes to slice together (`.stl`, `.obj`, `.3mf`), or one plate file (`.toml`) or
    /// project (`.encrust`).
    #[arg(required = true, value_name = "INPUT")]
    pub inputs: Vec<PathBuf>,

    /// Spread the models over the plate, biggest first, rather than leave each where its
    /// file put it.
    #[arg(long)]
    pub arrange: bool,
}

impl PlateArgs {
    /// Refuses a directory, which is a batch's input and not a model.
    pub fn check(&self) -> Result<()> {
        if let Some(dir) = self.inputs.iter().find(|path| path.is_dir()) {
            bail!(
                "{} is a directory; `encrust batch` slices a folder of models",
                dir.display()
            );
        }
        Ok(())
    }

    /// Whether this is one model alone, whose report keeps the shape a batch gives each.
    pub fn is_one_model(&self) -> bool {
        matches!(self.inputs.as_slice(), [only]
        if !only.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("toml") || extension.eq_ignore_ascii_case("encrust")
        }))
    }
}

impl SliceCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        let started = Instant::now();
        let watch = global.watch(stop);
        let staged = stage::stage(&self.plate.inputs, self.plate.arrange, &self.job, &watch)?;
        let (slice, raster) = slice_staged(&staged, &self.output, &self.job, &watch)?;
        let outcome = Outcome {
            parts: staged.parts,
            output: self.output.clone(),
            slice,
            raster,
        };
        if global.json {
            if self.plate.is_one_model() {
                json::print(&ModelReport::sliced(&outcome, started.elapsed()))?;
            } else {
                json::print(&PlateReport::sliced(
                    &self.plate.inputs,
                    &outcome,
                    started.elapsed(),
                ))?;
            }
        }
        Ok(if self.job.strict && !outcome.is_clean() {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}
