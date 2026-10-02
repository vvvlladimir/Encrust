use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;

use crate::args::{ImportArgs, ProfileArgs};
use crate::batch::ModelReport;
use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::{json, pipeline, profiles};

#[derive(clap::Args, Debug)]
pub struct InspectCommand {
    /// Mesh to look at: `.stl`, `.obj` or `.3mf`.
    pub input: PathBuf,

    #[command(flatten)]
    pub profile: ProfileArgs,

    #[command(flatten)]
    pub import: ImportArgs,

    /// Exit with code 3 when the model has defects or does not fit.
    #[arg(long)]
    pub strict: bool,
}

impl InspectCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        let chosen = profiles::resolve(&self.profile.selection())?;
        let started = Instant::now();
        let report = pipeline::inspect(&self.input, &self.import, &chosen)?;
        if global.json {
            json::print(&ModelReport::inspected(&report, started.elapsed()))?;
        } else if !global.quiet {
            print!("{report}");
        }
        Ok(if self.strict && !report.is_clean() {
            Exit::Unclean
        } else {
            Exit::Success
        })
    }
}
