//! The `encrust` command line: global flags, one subcommand each, and the dispatch to them.

mod batch;
mod completions;
mod convert;
mod diagnose;
mod estimate;
mod info;
mod inspect;
mod measure;
pub(crate) mod printer;
mod profiles;
mod recipes;
mod render;
mod serve;
pub mod slice;
mod test_print;

use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};

use crate::exit::{Exit, Stop};
use crate::pipeline::Watch;
use crate::steps::Steps;

pub use batch::BatchCommand;
pub use completions::CompletionsCommand;
pub use convert::ConvertCommand;
pub use diagnose::DiagnoseCommand;
pub use estimate::EstimateCommand;
pub use info::InfoCommand;
pub use inspect::InspectCommand;
pub use measure::MeasureCommand;
pub use printer::PrinterCommand;
pub use profiles::ProfilesCommand;
pub use recipes::RecipesCommand;
pub use render::RenderCommand;
pub use serve::ServeCommand;
pub use slice::SliceCommand;
pub use test_print::TestPrintCommand;

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

    /// Flags written down in a TOML file, a table per subcommand; a flag typed here wins.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// What `encrust serve` keeps between the runs it is asked for; none on the command line.
    #[arg(skip)]
    pub steps: Option<Arc<Steps>>,
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
            writes: true,
            steps: self.steps.clone(),
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
    /// How far apart two points of a model are, straight and, when asked, over its surface.
    Measure(MeasureCommand),
    /// Draw the plate as `slice` would build it, from the sides asked for, as PNG pictures
    /// with points marked.
    Render(RenderCommand),
    /// Open a sliced file, print what it states and decode every layer, or take one out.
    Info(InfoCommand),
    /// Write a sliced file again in another container, for a machine with the same panel.
    Convert(ConvertCommand),
    /// The printers and resins in the catalogue.
    #[command(subcommand)]
    Profiles(ProfilesCommand),
    /// The recipes Prepare runs, the built-in ones and yours.
    #[command(subcommand)]
    Recipes(RecipesCommand),
    /// Write one of the project's test prints as an STL: detail, exposure, surface or fit.
    TestPrint(TestPrintCommand),
    /// What a failed print most likely came from: its symptoms read against the check of the
    /// file printed, each cause with the change to try.
    Diagnose(DiagnoseCommand),
    /// Find printers on the network, ask one what it is doing, send it a file.
    #[command(subcommand)]
    Printer(PrinterCommand),
    /// Print the completion script for a shell, to be sourced from its startup file.
    Completions(CompletionsCommand),
    /// Run many invocations in one process, each a line of JSON on stdin answered on stdout,
    /// reusing what one made for the next: how a chat client's server drives the engine.
    Serve(ServeCommand),
}

impl Command {
    /// Whether the run reads [`Stop`] between its steps. One that does not is left to the
    /// shell's own Ctrl-C, so the first press ends it.
    pub fn watches_stop(&self) -> bool {
        match self {
            Self::Slice(_)
            | Self::Estimate(_)
            | Self::Render(_)
            | Self::Batch(_)
            | Self::Convert(_) => true,
            Self::Printer(command) => matches!(command, PrinterCommand::Send(_)),
            Self::Inspect(_)
            | Self::Measure(_)
            | Self::Info(_)
            | Self::Profiles(_)
            | Self::Recipes(_)
            | Self::TestPrint(_)
            | Self::Diagnose(_)
            | Self::Completions(_)
            | Self::Serve(_) => false,
        }
    }
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
        Command::Measure(command) => command.run(global),
        Command::Render(command) => command.run(global, stop),
        Command::Info(command) => command.run(global),
        Command::Convert(command) => command.run(global, stop),
        Command::Profiles(command) => command.run(global),
        Command::Recipes(command) => command.run(global),
        Command::TestPrint(command) => command.run(global),
        Command::Diagnose(command) => command.run(global),
        Command::Printer(command) => command.run(global, stop),
        Command::Completions(command) => command.run(),
        Command::Serve(command) => command.run(),
    }
}
