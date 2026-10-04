use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use core_pipeline::{Converting, Observer, convert, open_file};
use indicatif::ProgressBar;
use serde::Serialize;

use crate::args::ProfileArgs;
use crate::commands::GlobalArgs;
use crate::exit::{Cancelled, Exit, Stop};
use crate::progress::bar;
use crate::sliced_file::{CtbRevision, format_of, now_unix_s};
use crate::{json, profiles};

#[derive(clap::Args, Debug)]
pub struct ConvertCommand {
    /// Sliced file in any container this reads.
    pub input: PathBuf,

    /// The file to write; its extension picks the container.
    #[arg(short, long)]
    pub output: PathBuf,

    /// The printer must be given, and its panel must be the one the masks were drawn for.
    /// The resin gives the lifts, waits and price; layer heights and exposures are the
    /// file's own.
    #[command(flatten)]
    pub profile: ProfileArgs,

    /// Which `.ctb` revision to write, when the output is one.
    #[arg(long, value_name = "N", default_value = "4")]
    pub ctb_version: CtbRevision,
}

#[derive(Serialize)]
struct Report {
    input: String,
    output: String,
    layers: usize,
    resin_mm3: f32,
}

impl ConvertCommand {
    pub fn run(&self, global: &GlobalArgs, stop: &Stop) -> Result<Exit> {
        let chosen = profiles::resolve(&self.profile.selection())?;
        let Some(printer) = chosen.printer else {
            bail!("convert needs --printer or --profile: the machine the new file is for");
        };
        let Some(format) = format_of(&self.output, self.ctb_version) else {
            bail!("{} names no sliced-file format", self.output.display());
        };
        let mut source = open_file(&self.input)?;
        let name = self
            .output
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        let mut watching = Watching {
            bar: global.shows_progress().then(|| bar("layers")),
            stop,
        };
        let converted = convert(
            &mut source,
            &Converting {
                format: format.at_revision_of(printer.output),
                name: &name,
                printer: &printer,
                material: &chosen.material,
                created_unix_s: now_unix_s(),
            },
            &self.output,
            &mut watching,
        )
        .with_context(|| format!("cannot convert {}", self.input.display()))?;
        drop(watching);
        let converted = converted.ok_or(Cancelled)?;

        if global.json {
            json::print(&Report {
                input: self.input.display().to_string(),
                output: self.output.display().to_string(),
                layers: converted.layers,
                resin_mm3: converted.volume_mm3,
            })?;
        } else if global.talks() {
            println!(
                "Converted {} layers into {}, {:.1} ml of resin",
                converted.layers,
                self.output.display(),
                converted.volume_mm3 / 1000.0
            );
        }
        Ok(Exit::Success)
    }
}

/// The bar over the layers, and Ctrl-C.
struct Watching<'a> {
    bar: Option<ProgressBar>,
    stop: &'a Stop,
}

impl Observer for Watching<'_> {
    fn layers(&mut self, done: usize, total: usize) {
        if let Some(bar) = &self.bar {
            bar.set_length(total as u64);
            bar.set_position(done as u64);
        }
    }

    fn cancelled(&self) -> bool {
        self.stop.requested()
    }
}

impl Drop for Watching<'_> {
    fn drop(&mut self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}
