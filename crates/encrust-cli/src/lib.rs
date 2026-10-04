//! The `encrust` command line: its subcommands, their flags, and the runs they drive.
//!
//! `main.rs` is argument parsing, Ctrl-C and an exit code; everything else is here so that a
//! test can drive a run without spawning a process. Worked examples: `docs/cli.md`.

mod args;
mod batch;
mod commands;
mod config;
mod estimate;
mod exit;
mod hollowing;
mod json;
mod plate_file;
mod png_stack;
mod progress;
mod project;
mod raster_report;
mod slice_report;
mod sliced_file;
mod sliced_read;
mod stack;
mod stage;
mod stats;
mod supports;

pub mod pipeline;
pub mod profiles;
pub mod report;

pub use commands::{Cli, Command, GlobalArgs, init_tracing, run};
pub use config::parse_from;
pub use exit::{Cancelled, Exit, Stop, exit_code};
pub use json::{SCHEMA, error as json_error};
