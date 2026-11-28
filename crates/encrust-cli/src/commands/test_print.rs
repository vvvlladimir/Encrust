use std::path::PathBuf;

use anyhow::{Context, Result};
use core_samples::TestPrint;

use crate::commands::GlobalArgs;
use crate::exit::Exit;
use crate::json;

#[derive(clap::Args, Debug)]
pub struct TestPrintCommand {
    /// Which test print, as the window's start page offers them.
    #[arg(value_enum)]
    pub print: Printed,
    /// Where the STL goes: a file, or a directory it takes its own name in. The current
    /// directory when absent.
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

/// The test prints by the names the command line takes.
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum Printed {
    /// Spikes, ledges and pillars: the finest detail that prints.
    Detail,
    /// Pins, fins, slots and holes from fine to coarse: under- or overexposed.
    Exposure,
    /// Faces at each angle: the finish a surface takes.
    Surface,
    /// A hole and pins at each clearance: what fits.
    Fit,
}

impl From<Printed> for TestPrint {
    fn from(printed: Printed) -> Self {
        match printed {
            Printed::Detail => Self::Detail,
            Printed::Exposure => Self::Exposure,
            Printed::Surface => Self::Surface,
            Printed::Fit => Self::Fit,
        }
    }
}

impl TestPrintCommand {
    pub fn run(&self, global: &GlobalArgs) -> Result<Exit> {
        let print = TestPrint::from(self.print);
        let path = match &self.output {
            Some(path) if path.is_dir() => path.join(print.file()),
            Some(path) => path.clone(),
            None => PathBuf::from(print.file()),
        };
        let bytes = core_samples::stl(&print.build());
        std::fs::write(&path, bytes).with_context(|| format!("cannot write {}", path.display()))?;
        if global.json {
            json::print(&serde_json::json!({ "print": print.name(), "path": path }))?;
        } else if !global.quiet {
            println!(
                "Wrote the {} test print to {}",
                print.name(),
                path.display()
            );
        }
        Ok(Exit::Success)
    }
}
