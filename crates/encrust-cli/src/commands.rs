//! The `encrust` command line: global flags, one subcommand each, and the dispatch to them.

mod batch;
mod convert;
mod estimate;
mod info;
mod inspect;
mod profiles;
pub mod slice;

use std::io::IsTerminal;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};

use crate::exit::{Exit, Stop};
use crate::pipeline::Watch;

pub use batch::BatchCommand;
pub use convert::ConvertCommand;
pub use estimate::EstimateCommand;
pub use info::InfoCommand;
pub use inspect::InspectCommand;
pub use profiles::ProfilesCommand;
pub use slice::SliceCommand;

/// Slice meshes for MSLA resin printers.
#[derive(Parser, Debug)]
#[command(name = "encrust", version, about)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// Flags every subcommand answers.
#[derive(Debug, clap::Args)]
pub struct GlobalArgs {
    /// Print one JSON document to stdout in place of the text report; logs and progress
    /// stay on stderr.
    #[arg(long, global = true)]
    pub json: bool,

    /// Print nothing but errors: no stage reports, no progress, no log below error.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Log more: -v for debug, -vv for trace. RUST_LOG wins over both.
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    /// Draw no progress bar, even on a terminal.
    #[arg(long, global = true)]
    pub no_progress: bool,
}

impl GlobalArgs {
    /// Whether stages print their reports to stdout as they finish.
    pub fn talks(&self) -> bool {
        !self.json && !self.quiet
    }

    /// A bar only goes to a terminal, so a log file or a pipe never fills with redraws.
    pub fn shows_progress(&self) -> bool {
        !self.json && !self.quiet && !self.no_progress && std::io::stderr().is_terminal()
    }

    pub fn watch<'a>(&self, stop: &'a Stop) -> Watch<'a> {
        Watch {
            talk: self.talks(),
            progress: self.shows_progress(),
            stop,
        }
    }

    fn log_level(&self) -> &'static str {
        match (self.quiet, self.verbose) {
            (true, _) => "error",
            (false, 0) => "info",
            (false, 1) => "debug",
            (false, _) => "trace",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Slice models, a plate file or a project into a printable file, or a PNG stack for a
    /// name with no extension.
    Slice(SliceCommand),
    /// What a plate would take to print — layers, time, resin, weight, price, the layers
    /// likely to fail — with nothing written.
    Estimate(EstimateCommand),
    /// Slice every model in a directory: one file and one JSON report each, and
    /// `batch.json` over the lot.
    Batch(BatchCommand),
    /// Load and repair a model and say what was found, without slicing it.
    Inspect(InspectCommand),
    /// Open a sliced file, print what it states and decode every layer, or take one out.
    Info(InfoCommand),
    /// Write a sliced file again in another container, for a machine with the same panel.
    Convert(ConvertCommand),
    /// The printers and resins in the catalogue.
    #[command(subcommand)]
    Profiles(ProfilesCommand),
}

/// Sends logs to stderr at the level the global flags ask for, unless `RUST_LOG` says
/// otherwise. Stdout is the report's alone, which is what keeps `--json` parseable.
pub fn init_tracing(cli: &Cli) {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(cli.global.log_level())),
        )
        .init();
}

/// Runs one invocation until it ends or `stop` is asked for. A test drives this without
/// spawning a process; `exit::exit_code` turns what it returns into the process's code.
pub fn run(cli: &Cli, stop: &Stop) -> Result<Exit> {
    let global = &cli.global;
    match &cli.command {
        Command::Slice(command) => command.run(global, stop),
        Command::Estimate(command) => command.run(global, stop),
        Command::Batch(command) => command.run(global, stop),
        Command::Inspect(command) => command.run(global),
        Command::Info(command) => command.run(global),
        Command::Convert(command) => command.run(global, stop),
        Command::Profiles(command) => command.run(global),
    }
}
