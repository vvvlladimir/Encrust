use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;

use crate::args::{ImportArgs, ProfileArgs};
use crate::batch::ModelReport;
use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::plate_objects::plate_objects;
use crate::{json, pipeline};

#[derive(clap::Args, Debug)]
pub struct InspectCommand {
    /// Mesh to look at: `.stl`, `.obj` or `.3mf`; a project (`.encrust`) lists its objects
    /// by the ids `--only` takes.
    pub input: PathBuf,

    #[command(flatten)]
    pub profile: ProfileArgs,

    #[command(flatten)]
    pub import: ImportArgs,

    /// Exit with code 3 when the model has defects or does not fit.
    #[arg(long)]
    pub strict: bool,

    /// Count the pairs of faces that cross each other, which only wrapping mends.
    #[arg(long)]
    pub intersections: bool,

    /// List the shells, numbered as --drop-shells takes them.
    #[arg(long)]
    pub shells: bool,
}

impl InspectCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        if crate::stage::describes_a_project(&self.input) {
            return self.list_objects(global);
        }
        let chosen = self.profile.resolve()?;
        let started = Instant::now();
        let asked = pipeline::Inspect {
            intersections: self.intersections,
            shells: self.shells,
        };
        let report = pipeline::inspect(&self.input, &self.import, (&self.profile, &chosen), asked)?;
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

    /// Lists the project's objects, one a line, or as `objects` in JSON.
    fn list_objects(&self, global: &GlobalArgs) -> Result<Exit> {
        let project = crate::project::open(&self.input)?;
        let objects = plate_objects(&project);
        if global.json {
            json::print(&serde_json::json!({
                "input": self.input.display().to_string(),
                "objects": objects,
            }))?;
        } else if !global.quiet {
            println!(
                "{}
  id   name                         where",
                self.input.display()
            );
            for object in &objects {
                print!("{object}");
            }
        }
        Ok(Exit::Success)
    }
}
