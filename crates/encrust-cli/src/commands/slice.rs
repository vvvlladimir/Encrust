use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Result, bail};
use core_engine::project::Project;

use crate::args::JobArgs;
use crate::batch::{ModelReport, PlateReport};
use crate::commands::GlobalArgs;
use crate::exit::{Exit, Stop};
use crate::pipeline::{Outcome, Watch, slice_staged};
use crate::stage::Staged;
use crate::{json, project, stage};

#[derive(clap::Args, Debug)]
pub struct SliceCommand {
    #[command(flatten)]
    pub plate: PlateArgs,

    /// Where the sliced output goes. The extension picks the container; a name without
    /// one is a directory for a PNG stack.
    #[arg(short, long, default_value = "out")]
    pub output: PathBuf,

    /// Slice every plate of a project holding a model, each into a file named after it
    /// beside the output, as the window's Slice all plates does.
    #[arg(long, conflicts_with = "plate")]
    pub all_plates: bool,

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

    /// Which plate of a project to slice, counted from one; the plate it was saved on
    /// without it.
    #[arg(long, value_name = "N")]
    pub plate: Option<u32>,
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
        matches!(self.inputs.as_slice(), [only] if !stage::describes_a_plate(only))
    }
}

impl SliceCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        self.plate.check()?;
        if self.all_plates {
            return self.run_every_plate(global, stop);
        }
        let (report, clean) = self.slice_inputs(global, stop)?;
        if let Some(report) = report {
            json::print(&report)?;
        }
        Ok(self.exit(clean))
    }

    /// Each plate holding a model into its own file, one JSON document over them all.
    fn run_every_plate(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        let path = match self.plate.inputs.as_slice() {
            [path] if stage::describes_a_project(path) => path,
            _ => bail!("--all-plates slices the plates of one project (.encrust)"),
        };
        stage::refuse_for_a_project(self.plate.arrange, &self.job)?;
        let project = project::open(path, &self.job)?;
        let plates = project::printable_plates(&project.manifest);
        if plates.is_empty() {
            bail!("{} holds nothing to print", path.display());
        }
        let mut reports = Vec::with_capacity(plates.len());
        let mut clean = true;
        for (plate, output) in core_engine::plate_outputs(&self.output, &plates) {
            stop.check()?;
            if global.talks() {
                println!("Plate {} into {}", plate + 1, output.display());
            }
            let (report, plate_clean) =
                self.slice_plate(path, &project, plate, &output, global, stop)?;
            reports.extend(report);
            clean &= plate_clean;
        }
        if global.json {
            json::print(&serde_json::json!({ "plates": reports }))?;
        }
        Ok(self.exit(clean))
    }

    /// Slices what the inputs stage, the `--plate` of a project if given, into the output.
    fn slice_inputs(
        &self,
        global: &GlobalArgs,
        stop: &Stop,
    ) -> Result<(Option<serde_json::Value>, bool)> {
        let started = Instant::now();
        let watch = global.watch(stop);
        let plate = &self.plate;
        let mut staged =
            stage::stage(&plate.inputs, plate.arrange, plate.plate, &self.job, &watch)?;
        self.finish(&mut staged, &self.output, global, &watch, started)
    }

    fn slice_plate(
        &self,
        path: &Path,
        project: &Project,
        plate: u32,
        output: &Path,
        global: &GlobalArgs,
        stop: &Stop,
    ) -> Result<(Option<serde_json::Value>, bool)> {
        let started = Instant::now();
        let watch = global.watch(stop);
        let mut staged = project::stage(path, project, Some(plate), &self.job)?;
        self.finish(&mut staged, output, global, &watch, started)
    }

    /// Cuts the staged plate into `output`; answers its report under `--json` and whether it
    /// came out clean.
    fn finish(
        &self,
        staged: &mut Staged,
        output: &Path,
        global: &GlobalArgs,
        watch: &Watch,
        started: Instant,
    ) -> Result<(Option<serde_json::Value>, bool)> {
        let (slice, raster) = slice_staged(staged, output, &self.job, watch)?;
        let outcome = Outcome {
            parts: std::mem::take(&mut staged.parts),
            output: output.to_owned(),
            slice,
            raster,
        };
        let clean = outcome.is_clean();
        if !global.json {
            return Ok((None, clean));
        }
        let report = if self.plate.is_one_model() {
            serde_json::to_value(ModelReport::sliced(&outcome, started.elapsed()))
        } else {
            serde_json::to_value(PlateReport::sliced(
                &self.plate.inputs,
                &outcome,
                started.elapsed(),
            ))
        };
        Ok((Some(report?), clean))
    }

    fn exit(&self, clean: bool) -> Exit {
        if self.job.strict && !clean {
            Exit::Unclean
        } else {
            Exit::Success
        }
    }
}
