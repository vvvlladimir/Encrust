use std::time::Instant;

use anyhow::Result;

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
}

impl EstimateCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        let started = Instant::now();
        let watch = global.watch(stop);
        let mut staged = stage::stage(&self.plate.inputs, self.plate.arrange, &self.job, &watch)?;
        let estimate = estimate::estimate(&mut staged, &self.job.raster, &watch)?;
        if global.json {
            json::print(&EstimateReport::of(
                &self.plate.inputs,
                &staged.parts,
                &estimate,
                started.elapsed(),
            ))?;
        } else if global.talks() {
            print!("{estimate}");
        }
        let clean = staged.parts.iter().all(crate::stage::Part::is_clean) && estimate.is_clean();
        Ok(if self.job.strict && !clean {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}
